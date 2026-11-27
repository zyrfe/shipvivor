//! With the `mcp` feature (on by default): a Model Context Protocol server inside the
//! game, so an agent can read the battle and change it while it runs, and above all while
//! it is paused. It speaks MCP's Streamable HTTP transport, answering each POST with plain
//! JSON (it offers tools only, so it never streams), at `http://127.0.0.1:7531/mcp`, or
//! the port in `SHIPVIVOR_MCP_PORT`. Register it with
//! `claude mcp add --transport http shipvivor http://127.0.0.1:7531/mcp`.
//!
//! It asks for no credentials. It listens on the loopback address only, and refuses what a
//! web page could send it: a `Host` that isn't this machine (DNS rebinding), an `Origin`
//! that isn't this machine, and any body not sent as `application/json`, which a page can
//! only send after a CORS preflight that is never granted.
//!
//! The server runs on threads of its own and never touches the game. Each tool call is
//! handed over a channel to [`serve`], an exclusive system that runs every frame, paused
//! or not, and answers it there. See `tools` for what each tool does.
mod tools;

use bevy::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// The port when `SHIPVIVOR_MCP_PORT` names none.
pub const PORT: u16 = 7531;
/// The MCP revisions understood, the newest first.
const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// The largest request body taken, in bytes.
const MOST_BODY: usize = 1 << 20;

pub struct McpPlugin;
impl Plugin for McpPlugin {
    fn build(&self, app: &mut App) {
        let port = std::env::var("SHIPVIVOR_MCP_PORT")
            .ok()
            .and_then(|p| p.trim().parse().ok())
            .unwrap_or(PORT);
        let (calls, inbox) = channel();
        let started = std::thread::Builder::new()
            .name("mcp".into())
            .spawn(move || listen(port, calls));
        if let Err(err) = started {
            warn!("Couldn't start the MCP server: {err}");
        }
        app.insert_resource(Inbox(Mutex::new(inbox)))
            .init_resource::<tools::Stepping>()
            .add_systems(Update, serve)
            .add_systems(FixedUpdate, tools::count_steps.after(crate::advance_ship));
    }
}

/// What a tool hands back.
pub(crate) enum Content {
    Text(String),
    /// A PNG.
    Image(Vec<u8>),
}
/// A tool's answer, or why it refused.
pub(crate) type Reply = Result<Vec<Content>, String>;

/// A tool call waiting for the game.
struct Call {
    tool: String,
    args: Value,
    reply: Sender<Reply>,
}

#[derive(Resource)]
struct Inbox(Mutex<Receiver<Call>>);

/// Answer the tool calls that came in since the last frame. A call that waits on the
/// game (a step, a screenshot) keeps its reply to answer later.
fn serve(world: &mut World) {
    let calls: Vec<Call> = match world.resource::<Inbox>().0.lock() {
        Ok(inbox) => inbox.try_iter().collect(),
        Err(_) => return,
    };
    for call in calls {
        if let Some(answer) = tools::run(world, &call.tool, &call.args, call.reply.clone()) {
            let _ = call.reply.send(answer);
        }
    }
}

fn listen(port: u16, calls: Sender<Call>) {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(err) => {
            warn!("The MCP server couldn't take 127.0.0.1:{port}: {err}");
            return;
        }
    };
    info!("MCP server at http://127.0.0.1:{port}/mcp");
    for stream in listener.incoming().flatten() {
        let calls = calls.clone();
        std::thread::spawn(move || {
            if let Err(err) = answer(stream, port, &calls) {
                debug!("MCP connection: {err}");
            }
        });
    }
}

/// One HTTP request and its response. The connection closes after it.
fn answer(stream: TcpStream, port: u16, calls: &Sender<Call>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut out = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let path = words.next().unwrap_or_default().to_owned();
    let mut headers = HashMap::new();
    let mut size = 0;
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header)?;
        size += read;
        if read == 0 || header.trim().is_empty() {
            break;
        }
        if size > 64 * 1024 {
            return respond(&mut out, 431, "Request Header Fields Too Large", None);
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    if let Err(why) = vet(&method, &path, &headers, port) {
        return respond(&mut out, why.0, why.1, None);
    }
    let length: usize = (headers.get("content-length"))
        .and_then(|l| l.parse().ok())
        .unwrap_or(0);
    if length > MOST_BODY {
        return respond(&mut out, 413, "Content Too Large", None);
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let response = match serde_json::from_slice::<Value>(&body) {
        Ok(message) => rpc(message, calls),
        Err(err) => Some(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32700, "message": format!("not JSON: {err}") },
        })),
    };
    match response {
        Some(response) => respond(&mut out, 200, "OK", Some(&response.to_string())),
        None => respond(&mut out, 202, "Accepted", None),
    }
}

/// Refuse anything but a POST to `/mcp` from this machine, sent as JSON.
fn vet(
    method: &str,
    path: &str,
    headers: &HashMap<String, String>,
    port: u16,
) -> Result<(), (u16, &'static str)> {
    if !headers
        .get("host")
        .is_some_and(|host| local_host(host, port))
    {
        return Err((403, "Forbidden"));
    }
    if headers
        .get("origin")
        .is_some_and(|origin| !local_origin(origin))
    {
        return Err((403, "Forbidden"));
    }
    if path.split('?').next() != Some("/mcp") {
        return Err((404, "Not Found"));
    }
    if method != "POST" {
        return Err((405, "Method Not Allowed"));
    }
    let json = (headers.get("content-type"))
        .is_some_and(|t| t.to_ascii_lowercase().starts_with("application/json"));
    if !json {
        return Err((415, "Unsupported Media Type"));
    }
    Ok(())
}
fn local_host(host: &str, port: u16) -> bool {
    let host = host.to_ascii_lowercase();
    ["127.0.0.1", "localhost"]
        .iter()
        .any(|name| host == *name || host == format!("{name}:{port}"))
}
fn local_origin(origin: &str) -> bool {
    let origin = origin.to_ascii_lowercase();
    ["http://127.0.0.1", "http://localhost"].iter().any(|base| {
        origin
            .strip_prefix(base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(':'))
    })
}

fn respond(
    out: &mut TcpStream,
    code: u16,
    reason: &str,
    json: Option<&str>,
) -> std::io::Result<()> {
    let body = json.unwrap_or_default();
    let kind = if json.is_some() {
        "Content-Type: application/json\r\n"
    } else {
        ""
    };
    let allow = if code == 405 { "Allow: POST\r\n" } else { "" };
    write!(
        out,
        "HTTP/1.1 {code} {reason}\r\n{kind}{allow}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    out.flush()
}

/// Answer a JSON-RPC message or batch. Notifications get no answer.
fn rpc(message: Value, calls: &Sender<Call>) -> Option<Value> {
    match message {
        Value::Array(batch) => {
            let answers: Vec<Value> = batch.into_iter().filter_map(|m| one(m, calls)).collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
        message => one(message, calls),
    }
}

fn one(message: Value, calls: &Sender<Call>) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::list() })),
        "tools/call" => Ok(call(&params, calls)),
        _ => Err(format!("there is no method {method:?}")),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": message },
        }),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|v| PROTOCOLS.contains(v))
        .unwrap_or(PROTOCOLS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "shipvivor", "version": env!("CARGO_PKG_VERSION") },
        "instructions": tools::INSTRUCTIONS,
    })
}

/// Hand a tool call to the game and wait for its answer.
fn call(params: &Value, calls: &Sender<Call>) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let answer = if !tools::known(name) {
        Err(format!("there is no tool {name:?}"))
    } else {
        let (reply, answer) = channel();
        let patience = tools::patience(name, &args);
        let sent = calls.send(Call {
            tool: name.to_owned(),
            args,
            reply,
        });
        match sent {
            Err(_) => Err("the game has shut down".into()),
            Ok(()) => answer.recv_timeout(patience).unwrap_or_else(|_| {
                Err("the game didn't answer in time: is its window stalled or minimised?".into())
            }),
        }
    };
    match answer {
        Ok(contents) => json!({
            "content": contents.into_iter().map(content).collect::<Vec<_>>(),
            "isError": false,
        }),
        Err(why) => json!({
            "content": [{ "type": "text", "text": why }],
            "isError": true,
        }),
    }
}

fn content(c: Content) -> Value {
    match c {
        Content::Text(text) => json!({ "type": "text", "text": text }),
        Content::Image(png) => json!({
            "type": "image",
            "data": base64(&png),
            "mimeType": "image/png",
        }),
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn only_json_posts_from_this_machine_are_taken() {
        let good = [
            ("host", "127.0.0.1:7531"),
            ("content-type", "application/json"),
        ];
        assert_eq!(vet("POST", "/mcp", &headers(&good), 7531), Ok(()));
        let local = [
            ("host", "localhost:7531"),
            ("content-type", "application/json; charset=utf-8"),
            ("origin", "http://localhost:6274"),
        ];
        assert_eq!(vet("POST", "/mcp", &headers(&local), 7531), Ok(()));
        // DNS rebinding: a page on another name that resolves here.
        let rebound = [
            ("host", "evil.example:7531"),
            ("content-type", "application/json"),
        ];
        assert_eq!(
            vet("POST", "/mcp", &headers(&rebound), 7531),
            Err((403, "Forbidden"))
        );
        // A page elsewhere posting across origins.
        let page = [
            ("host", "127.0.0.1:7531"),
            ("content-type", "application/json"),
            ("origin", "https://evil.example"),
        ];
        assert_eq!(
            vet("POST", "/mcp", &headers(&page), 7531),
            Err((403, "Forbidden"))
        );
        let sly = [
            ("host", "127.0.0.1:7531"),
            ("content-type", "application/json"),
            ("origin", "http://localhost.evil.example"),
        ];
        assert_eq!(
            vet("POST", "/mcp", &headers(&sly), 7531),
            Err((403, "Forbidden"))
        );
        // A simple form post needs no preflight, so it is refused for its type.
        let form = [("host", "127.0.0.1:7531"), ("content-type", "text/plain")];
        assert_eq!(
            vet("POST", "/mcp", &headers(&form), 7531),
            Err((415, "Unsupported Media Type"))
        );
        assert_eq!(
            vet("GET", "/mcp", &headers(&good), 7531),
            Err((405, "Method Not Allowed"))
        );
        assert_eq!(
            vet("POST", "/", &headers(&good), 7531),
            Err((404, "Not Found"))
        );
        assert_eq!(
            vet(
                "POST",
                "/mcp",
                &headers(&[("content-type", "application/json")]),
                7531
            ),
            Err((403, "Forbidden"))
        );
    }

    #[test]
    fn notifications_get_no_answer_and_the_handshake_names_the_tools() {
        let (calls, _inbox) = channel();
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(rpc(note, &calls), None);
        let hello = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } });
        let answer = rpc(hello, &calls).unwrap();
        assert_eq!(answer["result"]["protocolVersion"], "2025-03-26");
        let list = rpc(
            json!({ "jsonrpc": "2.0", "id": "a", "method": "tools/list" }),
            &calls,
        )
        .unwrap();
        let names: Vec<&str> = (list["result"]["tools"].as_array().unwrap().iter())
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for name in [
            "situation",
            "spawn",
            "move",
            "replace",
            "reweigh",
            "rewind",
            "step",
        ] {
            assert!(names.contains(&name), "{name} missing from {names:?}");
        }
        let unknown = rpc(
            json!({ "jsonrpc": "2.0", "id": 2, "method": "nope" }),
            &calls,
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn base64_pads_as_the_standard_says() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
