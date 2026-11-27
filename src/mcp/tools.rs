//! The in-game MCP server's tools: what the battle looks like, and the changes an agent
//! may make to it. Each runs on the game's own thread, in [`super::serve`].
use super::{Content, Reply};
use crate::rewind::{Live, MarkKind, Timeline};
use crate::state::{Campaign, Encounter, Mode, PlayerHelm, Screen};
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use serde_json::{Value, json};
use shipvivor_campaign::Card;
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{EnemyClass, Raider, Weight};
use std::fmt::Write as _;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// What the server tells an agent when it connects.
pub(super) const INSTRUCTIONS: &str = "Shipvivor, running. The player's ship is Breakwater; \
the enemy ships are raiders, each known by its id (#123). Positions are metres on the chart: \
x east, y south, so north is -y. Headings and bearings are compass degrees, clockwise from \
north. A position can be given as x and y, or as a range in metres and a bearing from \
Breakwater (dead ahead when the bearing is left out). Start with `situation`. Pause the \
battle to work on it, `step` it a few seconds to watch, and `rewind` along its timeline to \
try a moment again. Every change made here is written into the run's log as a dev line.";

const KNOTS_PER_MPS: f32 = 1.943_84;
/// The most seconds of battle one `step` sails.
const MOST_STEP: f32 = 30.;
/// The width a screenshot is cut down to, at most.
const SHOT_WIDTH: u32 = 1280;
const SHOT_FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/work/mcp");

const CLASSES: [EnemyClass; 10] = [
    EnemyClass::Freighter,
    EnemyClass::Coaster,
    EnemyClass::Patrol,
    EnemyClass::TorpedoBoat,
    EnemyClass::Destroyer,
    EnemyClass::Cruiser,
    EnemyClass::Battleship,
    EnemyClass::Commodore,
    EnemyClass::Parliament,
    EnemyClass::Regent,
];
const WEIGHTS: [Weight; 4] = [
    Weight::Light,
    Weight::Standard,
    Weight::Heavy,
    Weight::UltraHeavy,
];
const CARDS: [Card; 24] = [
    Card::Loader,
    Card::Shells,
    Card::Turbines,
    Card::Caps,
    Card::Velocity,
    Card::TripleTurrets,
    Card::QuadTurrets,
    Card::MoreSecondaries,
    Card::HeavySecondaries,
    Card::QuickSecondaries,
    Card::SecondaryRange,
    Card::TwinSecondaries,
    Card::TrainingGear,
    Card::Armor,
    Card::Hardening,
    Card::MoreAntiAir,
    Card::HeavyAntiAir,
    Card::QuickAntiAir,
    Card::Pumps,
    Card::MoreLaunchers,
    Card::MoreMissiles,
    Card::MorePlanes,
    Card::MoreBombs,
    Card::SecondaryTurrets,
];

/// Every tool, with what it takes, for `tools/list`.
pub(super) fn list() -> Value {
    let classes: Vec<String> = CLASSES.iter().map(|c| format!("{c:?}")).collect();
    let weights: Vec<String> = WEIGHTS.iter().map(|w| format!("{w:?}")).collect();
    let cards: Vec<String> = CARDS.iter().map(|c| format!("{c:?}")).collect();
    let id = json!({ "type": "integer", "description": "The ship's id, as `situation` lists it." });
    let class = json!({ "type": "string", "enum": classes });
    let weight = json!({
        "type": "string",
        "enum": weights,
        "description": "Barrels in every main turret: 1, 2, 3 or 4. Only destroyers, cruisers and battleships come in weights.",
    });
    let place = |extra: Value| {
        let mut props = json!({
            "x": { "type": "number", "description": "Metres east." },
            "y": { "type": "number", "description": "Metres south (north is -y)." },
            "range": { "type": "number", "description": "Metres from Breakwater, instead of x and y." },
            "bearing": { "type": "number", "description": "Compass degrees from Breakwater, with range. Dead ahead without one." },
            "heading": { "type": "number", "description": "Compass degrees." },
        });
        if let (Value::Object(props), Value::Object(extra)) = (&mut props, extra) {
            props.extend(extra);
        }
        props
    };
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| {
        json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": properties, "required": required },
        })
    };
    json!([
        tool(
            "situation",
            "The battle as it stands: the screen, the leg and its clock, Breakwater, every enemy ship afloat nearest first, the bases, and the timeline.",
            json!({ "ships": { "type": "integer", "description": "The most ships listed (default 30)." } }),
            &[]
        ),
        tool(
            "ship",
            "One enemy ship in full: class, weight, guns, hull, motion, orders.",
            json!({ "id": id }),
            &["id"]
        ),
        tool(
            "pause",
            "Pause the battle, as the Esc menu does.",
            json!({}),
            &[]
        ),
        tool("resume", "Resume a paused battle.", json!({}), &[]),
        tool(
            "step",
            "Sail a paused battle on for some seconds, then pause it again, and say what changed.",
            json!({ "seconds": { "type": "number", "description": "Seconds of battle, up to 30." } }),
            &["seconds"]
        ),
        tool(
            "spawn",
            "Place a new enemy ship. She sails on her own, in no squadron. Facing Breakwater without a heading; gunned as the sector would gun her without a weight.",
            place(json!({ "class": class, "weight": weight })),
            &["class"]
        ),
        tool(
            "move",
            "Move an enemy ship, with no way on. A ship at anchor weighs and sails.",
            place(json!({ "id": id })),
            &["id"]
        ),
        tool(
            "remove",
            "Take an enemy ship out of the battle. She isn't sunk: no wreck, no credit.",
            json!({ "id": id }),
            &["id"]
        ),
        tool(
            "replace",
            "Rebuild an enemy ship as a fresh ship of a class where she lies, keeping her id and squadron. The same class mends her.",
            json!({ "id": id, "class": class, "weight": weight }),
            &["id", "class"]
        ),
        tool(
            "reweigh",
            "Regun an enemy ship in another weight: up or down a number of barrels a turret.",
            json!({ "id": id, "weight": weight }),
            &["id", "weight"]
        ),
        tool(
            "damage",
            "Strike an enemy ship's hull for some points, spread along her length. A ship holed below water sinks on the next step and counts as sunk.",
            json!({ "id": id, "points": { "type": "number" } }),
            &["id", "points"]
        ),
        tool(
            "sink",
            "Sink an enemy ship on the next step of the battle, with her wreck and credit.",
            json!({ "id": id }),
            &["id"]
        ),
        tool(
            "player",
            "Change Breakwater: move her (x/y or range/bearing, heading), mend her hull, or fit a card at once.",
            place(
                json!({ "mend": { "type": "boolean" }, "fit": { "type": "string", "enum": cards } })
            ),
            &[]
        ),
        tool(
            "timeline",
            "The battle's timeline: the moments kept and every ship sunk on it.",
            json!({}),
            &[]
        ),
        tool(
            "rewind",
            "Take the battle back along its timeline and leave it paused there. Give `to`, seconds into the sector, or `back`, seconds before now.",
            json!({ "to": { "type": "number" }, "back": { "type": "number" } }),
            &[]
        ),
        tool(
            "screenshot",
            "The game window as it is now, as a PNG. Also saved under work/mcp.",
            json!({}),
            &[]
        ),
    ])
}

pub(super) fn known(name: &str) -> bool {
    list()
        .as_array()
        .is_some_and(|tools| tools.iter().any(|t| t["name"] == name))
}

/// How long the server waits for the game to answer a call.
pub(super) fn patience(name: &str, args: &Value) -> Duration {
    let base = Duration::from_secs(15);
    match name {
        // A step sails in real time, at whatever pace the window keeps.
        "step" => base + Duration::from_secs_f32(seconds(args).unwrap_or(0.) * 2.),
        "screenshot" => Duration::from_secs(30),
        _ => base,
    }
}

/// A `step` under way: the seconds of battle left to sail, and who is waiting.
#[derive(Resource, Default)]
pub(super) struct Stepping(Option<(f32, f32, Sender<Reply>)>);

/// Answer a call now, or `None` when the reply will come later.
pub(super) fn run(
    world: &mut World,
    name: &str,
    args: &Value,
    reply: Sender<Reply>,
) -> Option<Reply> {
    Some(match name {
        "situation" => {
            let ships = args.get("ships").and_then(Value::as_u64).unwrap_or(30) as usize;
            Ok(text(situation(world, ships)))
        }
        "ship" => ship(world, args),
        "pause" => pause(world),
        "resume" => resume(world),
        "step" => return step(world, args, reply),
        "spawn" => spawn(world, args),
        "move" => move_ship(world, args),
        "remove" => remove(world, args),
        "replace" => replace(world, args),
        "reweigh" => reweigh(world, args),
        "damage" => damage(world, args, None),
        "sink" => damage(world, args, Some(1e6)),
        "player" => player(world, args),
        "timeline" => Ok(text(timeline(world))),
        "rewind" => rewind(world, args),
        "screenshot" => return screenshot(world, reply),
        _ => Err(format!("there is no tool {name:?}")),
    })
}

fn text(s: impl Into<String>) -> Vec<Content> {
    vec![Content::Text(s.into())]
}
fn degrees(radians: f32) -> i32 {
    (radians.to_degrees().round() as i32).rem_euclid(360)
}
fn clock(secs: f32) -> String {
    let whole = secs.abs().round() as u32;
    let sign = if secs < 0. { "-" } else { "" };
    format!("{sign}{}:{:02}", whole / 60, whole % 60)
}
fn number(args: &Value, key: &str) -> Option<f32> {
    args.get(key).and_then(Value::as_f64).map(|n| n as f32)
}
fn seconds(args: &Value) -> Option<f32> {
    number(args, "seconds")
}
fn id(args: &Value) -> Result<u64, String> {
    args.get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "give the ship's id".into())
}
/// A name with its case, spaces and underscores ignored.
fn fold(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn class(args: &Value) -> Result<EnemyClass, String> {
    let name = args.get("class").and_then(Value::as_str).unwrap_or("");
    let wanted = fold(name);
    CLASSES
        .into_iter()
        .find(|c| {
            fold(&format!("{c:?}")) == wanted || fold(crate::mission_hud::ship_class(*c)) == wanted
        })
        .ok_or_else(|| format!("no class {name:?}: one of {CLASSES:?}"))
}
fn weight(args: &Value) -> Result<Option<Weight>, String> {
    let Some(name) = args.get("weight").and_then(Value::as_str) else {
        return Ok(None);
    };
    let wanted = match fold(name).as_str() {
        "ultra" => "ultraheavy".to_string(),
        other => other.to_string(),
    };
    WEIGHTS
        .into_iter()
        .find(|w| fold(&format!("{w:?}")) == wanted)
        .map(Some)
        .ok_or_else(|| format!("no weight {name:?}: one of {WEIGHTS:?}"))
}
/// The place the arguments name, from x and y or a range and bearing from her.
fn place(args: &Value, helm: &PlayerHelm) -> Result<Option<Vec2>, String> {
    match (number(args, "x"), number(args, "y"), number(args, "range")) {
        (Some(x), Some(y), _) => Ok(Some(Vec2::new(x, y))),
        (None, None, Some(range)) => {
            let bearing = number(args, "bearing").map_or(helm.heading, f32::to_radians);
            Ok(Some(helm.position + direction(bearing) * range))
        }
        (None, None, None) => Ok(None),
        _ => Err("give both x and y, or a range and a bearing".into()),
    }
}
fn heading_arg(args: &Value) -> Option<f32> {
    number(args, "heading").map(f32::to_radians)
}

/// A ship's name as the HUD gives it, with her weight.
fn name(r: &Raider) -> String {
    let class = crate::mission_hud::ship_class(r.class);
    if Weight::comes_in(r.class) {
        r.weight().name(class)
    } else {
        class.to_owned()
    }
}
fn ship_line(r: &Raider, from: Vec2) -> String {
    let off = r.body.pos - from;
    let mut line = format!(
        "#{} {}  at ({:.0}, {:.0})  {:.2} km bearing {:03}°  heading {:03}°  {:.0} kn  hull {:.0}%",
        r.body.id,
        name(r),
        r.body.pos.x,
        r.body.pos.y,
        off.length() / 1000.,
        degrees(heading(off)),
        degrees(r.body.heading),
        r.motion().length() * KNOTS_PER_MPS,
        r.vessel().integrity() * 100.
    );
    if r.has_contact {
        line.push_str("  has her in sight");
    }
    if let Some(base) = r.moored_at() {
        let _ = write!(line, "  at anchor off base {base}");
    }
    line
}

/// Edits are made to a battle on screen, not to one the timeline is showing.
fn editable(world: &World) -> Result<(), String> {
    let run = world.resource::<Campaign>();
    match run.screen() {
        Screen::Rewind => Err(
            "the timeline is open on screen: close it (Esc) or use the rewind tool first".into(),
        ),
        Screen::Sailing | Screen::Paused | Screen::Trial | Screen::Refit => Ok(()),
        screen => Err(format!("no battle is under way on the {screen:?} screen")),
    }
}
fn note(world: &mut World, line: String) {
    if let Some(mut log) = world.get_resource_mut::<crate::telemetry::Logbook>() {
        log.note_dev(&line);
    }
}

fn situation(world: &World, ships: usize) -> String {
    let run = world.resource::<Campaign>();
    let battle = world.resource::<Encounter>();
    let helm = world.resource::<PlayerHelm>();
    let timeline = world.resource::<Timeline>();
    let mut s = String::new();
    let mission = run.stage.mission();
    let _ = writeln!(
        s,
        "Screen {:?} (mode {:?}). Round {} of {}: {}{}, {} into the sector, {} on the clock.",
        run.screen(),
        run.mode,
        run.stage.round(),
        shipvivor_campaign::ROUNDS,
        mission.name,
        if run.stage.is_opening() {
            String::new()
        } else {
            format!(" ({})", run.grade.name().to_lowercase())
        },
        clock(battle.at_sea()),
        clock(run.remaining()),
    );
    let o = run.objectives;
    let _ = writeln!(
        s,
        "Stars {}: sunk {} of {}, stores {} of {}, drydock {}. Run: hits {}, sunk {}.",
        run.stars(),
        o.kills,
        mission.kills,
        o.crates,
        mission.crates,
        if o.destroyed_drydock {
            "destroyed"
        } else {
            "standing"
        },
        run.hits,
        run.sunk
    );
    let _ = writeln!(
        s,
        "\nBreakwater at ({:.0}, {:.0}), heading {:03}°, {:.1} kn (ordered {:.1} kn), hull {}%, level {} ({} XP to the next).",
        helm.position.x,
        helm.position.y,
        degrees(helm.heading),
        helm.speed * KNOTS_PER_MPS,
        helm.ordered_speed() * KNOTS_PER_MPS,
        run.hull_percent(),
        run.level,
        run.next_level_xp().saturating_sub(run.experience),
    );
    let _ = writeln!(s, "Fittings: {:?}", run.fittings);
    let mut afloat: Vec<&Raider> = battle.raiders().iter().filter(|r| r.afloat()).collect();
    afloat.sort_by(|a, b| {
        let d = |r: &&Raider| r.body.pos.distance_squared(helm.position);
        d(a).total_cmp(&d(b))
    });
    let _ = writeln!(s, "\nEnemy ships afloat: {}, nearest first.", afloat.len());
    for r in afloat.iter().take(ships) {
        let _ = writeln!(s, "  {}", ship_line(r, helm.position));
    }
    if afloat.len() > ships {
        let _ = writeln!(s, "  … and {} more", afloat.len() - ships);
    }
    let _ = writeln!(s, "\nBases: {}.", battle.ports().len());
    for (i, port) in battle.ports().iter().enumerate() {
        let off = port.body.pos - helm.position;
        let guns = port.turrets().iter().filter(|g| g.live()).count();
        let _ = writeln!(
            s,
            "  base {i} at ({:.0}, {:.0}) {:.2} km bearing {:03}°: {}{}, {} of {} docks, {} factories, {} guns",
            port.body.pos.x,
            port.body.pos.y,
            off.length() / 1000.,
            degrees(heading(off)),
            if port.wrecked() {
                "wrecked"
            } else {
                "standing"
            },
            if port.roused() { ", roused" } else { "" },
            port.live_docks(),
            port.docks().len(),
            port.live_factories(),
            guns,
        );
    }
    s.push('\n');
    s.push_str(&timeline_text(timeline));
    s
}

fn timeline_text(timeline: &Timeline) -> String {
    let Some((first, last)) = timeline.span() else {
        return "Timeline: nothing of this leg is kept yet.".into();
    };
    let mut s = format!(
        "Timeline: {} moments from {} to {} ({:.1} to {:.1} s into the sector).",
        timeline.moments(),
        clock(first),
        clock(last),
        first,
        last
    );
    if let Some(at) = timeline.scrubbing() {
        let _ = write!(s, " Open on screen at {}.", clock(at));
    }
    for mark in timeline.marks() {
        let what = match mark.kind {
            MarkKind::Sunk(class) => format!("{} sunk", crate::mission_hud::ship_class(class)),
            MarkKind::Lost => "Breakwater lost".into(),
        };
        let _ = write!(
            s,
            "\n  {} ({:.1} s): {what} at ({:.0}, {:.0})",
            clock(mark.at),
            mark.at,
            mark.pos.x,
            mark.pos.y
        );
    }
    s
}

fn timeline(world: &World) -> String {
    timeline_text(world.resource::<Timeline>())
}

fn ship(world: &World, args: &Value) -> Reply {
    let id = id(args)?;
    let battle = world.resource::<Encounter>();
    let helm = world.resource::<PlayerHelm>();
    let r = battle
        .ship(id)
        .ok_or_else(|| format!("no ship afloat has id {id}"))?;
    let mut s = ship_line(r, helm.position);
    let _ = write!(
        s,
        "\nClass {:?}, weight {:?}, {} barrels a turret, {} guns. Hull {:.0} points ({:.0}%).",
        r.class,
        r.weight(),
        r.barrels(),
        r.guns().len(),
        r.hull(),
        r.vessel().integrity() * 100.
    );
    if let Some(port) = r.destination_port() {
        let _ = write!(s, "\nBound for base {port}.");
    }
    if r.cargo() > 0. {
        let _ = write!(s, " Cargo {:.2} dock points.", r.cargo());
    }
    if r.director_lost {
        s.push_str("\nHer director is out.");
    }
    Ok(text(s))
}

fn pause(world: &mut World) -> Reply {
    let mut run = world.resource_mut::<Campaign>();
    match run.screen() {
        Screen::Sailing => {
            run.mode = Mode::Paused;
            Ok(text("Paused."))
        }
        Screen::Paused => Ok(text("Already paused.")),
        screen => Err(format!("nothing to pause on the {screen:?} screen")),
    }
}

fn resume(world: &mut World) -> Reply {
    let mut run = world.resource_mut::<Campaign>();
    match run.screen() {
        Screen::Paused => {
            run.mode = Mode::Patrol;
            Ok(text("Resumed."))
        }
        Screen::Sailing => Ok(text("Already under way.")),
        screen => Err(format!("nothing to resume on the {screen:?} screen")),
    }
}

fn step(world: &mut World, args: &Value, reply: Sender<Reply>) -> Option<Reply> {
    let secs = seconds(args).unwrap_or(0.);
    if !(secs > 0. && secs <= MOST_STEP) {
        return Some(Err(format!("step between 0 and {MOST_STEP} seconds")));
    }
    if world.resource::<Stepping>().0.is_some() {
        return Some(Err("a step is already under way".into()));
    }
    let mut run = world.resource_mut::<Campaign>();
    match run.screen() {
        Screen::Paused | Screen::Sailing => run.mode = Mode::Patrol,
        screen => return Some(Err(format!("nothing to step on the {screen:?} screen"))),
    }
    let from = world.resource::<Encounter>().at_sea();
    world.resource_mut::<Stepping>().0 = Some((secs, from, reply));
    None
}

/// Count a `step` down on the battle's own clock, and pause when it is sailed.
pub(super) fn count_steps(
    fixed: Res<Time<Fixed>>,
    mut stepping: ResMut<Stepping>,
    mut run: ResMut<Campaign>,
    battle: Res<Encounter>,
    timeline: Res<Timeline>,
) {
    let Some((left, from, _)) = &mut stepping.0 else {
        return;
    };
    let from = *from;
    let screen = run.screen();
    let ended = if screen == Screen::Sailing {
        *left -= fixed.delta_secs();
        if *left > 1e-4 {
            return;
        }
        run.mode = Mode::Paused;
        String::from("paused again")
    } else {
        format!("stopped early on the {screen:?} screen")
    };
    let Some((_, _, reply)) = stepping.0.take() else {
        return;
    };
    let now = battle.at_sea();
    let sunk: Vec<String> = (timeline.marks().iter())
        .filter(|m| m.at > from + 1e-3)
        .map(|m| match m.kind {
            MarkKind::Sunk(class) => format!("{} sunk", crate::mission_hud::ship_class(class)),
            MarkKind::Lost => "Breakwater lost".into(),
        })
        .collect();
    let mut s = format!(
        "Sailed {:.1} s, from {} to {}, and {ended}. Hull {}%, {} enemy ships afloat.",
        now - from,
        clock(from),
        clock(now),
        run.hull_percent(),
        battle.raiders().iter().filter(|r| r.afloat()).count()
    );
    if !sunk.is_empty() {
        let _ = write!(s, " Meanwhile: {}.", sunk.join(", "));
    }
    let _ = reply.send(Ok(text(s)));
}

fn spawn(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let class = class(args)?;
    let weight = weight(args)?;
    let helm = world.resource::<PlayerHelm>();
    let at = place(args, helm)?.ok_or("give x and y, or a range and a bearing")?;
    let facing = heading_arg(args).unwrap_or_else(|| heading(helm.position - at));
    let from = helm.position;
    let mut battle = world.resource_mut::<Encounter>();
    let id = battle.dev_spawn(class, at, facing, weight)?;
    let line = ship_line(battle.ship(id).expect("just placed"), from);
    note(world, format!("placed {line}"));
    Ok(text(format!("Placed {line}")))
}

fn move_ship(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let id = id(args)?;
    let helm = world.resource::<PlayerHelm>();
    let from = helm.position;
    let at = place(args, helm)?;
    let mut battle = world.resource_mut::<Encounter>();
    let r = battle
        .ship(id)
        .ok_or_else(|| format!("no ship afloat has id {id}"))?;
    let at = at.unwrap_or(r.body.pos);
    battle.dev_move(id, at, heading_arg(args))?;
    let line = ship_line(battle.ship(id).expect("just moved"), from);
    note(world, format!("moved {line}"));
    Ok(text(format!("Moved {line}")))
}

fn remove(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let id = id(args)?;
    let class = world.resource_mut::<Encounter>().dev_remove(id)?;
    note(world, format!("removed #{id} {class:?}"));
    Ok(text(format!("Removed #{id}, a {class:?}.")))
}

fn replace(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let (id, class, weight) = (id(args)?, class(args)?, weight(args)?);
    let from = world.resource::<PlayerHelm>().position;
    let mut battle = world.resource_mut::<Encounter>();
    battle.dev_replace(id, class, weight)?;
    let line = ship_line(battle.ship(id).expect("just rebuilt"), from);
    note(world, format!("rebuilt {line}"));
    Ok(text(format!("Rebuilt as {line}")))
}

fn reweigh(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let id = id(args)?;
    let weight = weight(args)?.ok_or("give a weight")?;
    let from = world.resource::<PlayerHelm>().position;
    let mut battle = world.resource_mut::<Encounter>();
    battle.dev_reweigh(id, weight)?;
    let line = ship_line(battle.ship(id).expect("just regunned"), from);
    note(world, format!("regunned {line}"));
    Ok(text(format!("Regunned: {line}")))
}

fn damage(world: &mut World, args: &Value, points: Option<f32>) -> Reply {
    editable(world)?;
    let id = id(args)?;
    let points = points
        .or_else(|| number(args, "points"))
        .filter(|p| *p > 0.)
        .ok_or("give the points of damage")?;
    let left = world.resource_mut::<Encounter>().dev_damage(id, points)?;
    let sinking = left <= 0.
        || !world
            .resource::<Encounter>()
            .ship(id)
            .is_some_and(Raider::afloat);
    note(
        world,
        format!("struck #{id} for {points:.0}, hull {:.0}%", left * 100.),
    );
    Ok(text(if sinking {
        format!("#{id} is going down: she sinks on the next step of the battle.")
    } else {
        format!(
            "#{id} struck for {points:.0}: hull {:.0}% left.",
            left * 100.
        )
    }))
}

fn player(world: &mut World, args: &Value) -> Reply {
    editable(world)?;
    let mut done = Vec::new();
    let helm = world.resource::<PlayerHelm>();
    if let Some(at) = place(args, helm)? {
        let radius = world.resource::<Campaign>().hull_shape().radius;
        world.resource::<Encounter>().dev_check_water(radius, at)?;
        let mut helm = world.resource_mut::<PlayerHelm>();
        helm.position = at;
        if let Some(heading) = heading_arg(args) {
            helm.heading = heading;
        }
        world.resource_mut::<crate::seascape::Sea>().calm();
        done.push(format!("moved to ({:.0}, {:.0})", at.x, at.y));
    } else if let Some(heading) = heading_arg(args) {
        world.resource_mut::<PlayerHelm>().heading = heading;
        done.push(format!("turned to {:03}°", degrees(heading)));
    }
    if args.get("mend").and_then(Value::as_bool) == Some(true) {
        world.resource_mut::<Campaign>().dev_mend();
        done.push("mended".into());
    }
    if let Some(card) = args.get("fit").and_then(Value::as_str) {
        let wanted = fold(card);
        let card = (CARDS.into_iter())
            .find(|c| fold(&format!("{c:?}")) == wanted)
            .ok_or_else(|| format!("no card {card:?}"))?;
        let before = world.resource::<Campaign>().fittings;
        world.resource_mut::<Campaign>().dev_fit(card);
        if world.resource::<Campaign>().fittings == before {
            done.push(format!("{card:?} adds nothing more to her"));
        } else {
            done.push(format!("fitted {card:?}"));
        }
    }
    if done.is_empty() {
        return Err("nothing to change: give a place, a heading, mend or fit".into());
    }
    world.resource_scope(|world, mut battle: Mut<Encounter>| {
        battle.fit_player(world.resource::<Campaign>());
    });
    let summary = done.join(", ");
    note(world, format!("Breakwater {summary}"));
    let hull = world.resource::<Campaign>().hull_percent();
    Ok(text(format!("Breakwater {summary}. Hull {hull}%.")))
}

fn rewind(world: &mut World, args: &Value) -> Reply {
    let now = world.resource::<Encounter>().at_sea();
    let to = match (number(args, "to"), number(args, "back")) {
        (Some(to), None) => to,
        (None, Some(back)) => now - back,
        _ => {
            return Err("give `to` (seconds into the sector) or `back` (seconds before now)".into());
        }
    };
    let mut state: SystemState<(ResMut<Timeline>, Live)> = SystemState::new(world);
    let done = match state.get_mut(world) {
        Ok((mut timeline, mut live)) => timeline.rewind_to(to, &mut live),
        Err(err) => Err(format!("the battle isn't ready: {err}")),
    };
    state.apply(world);
    let (from, to) = done?;
    Ok(text(format!(
        "Rewound {:.1} s, from {} to {}, and paused there. {}",
        from - to,
        clock(from),
        clock(to),
        situation(world, 12)
    )))
}

fn screenshot(world: &mut World, reply: Sender<Reply>) -> Option<Reply> {
    if let Err(err) = std::fs::create_dir_all(SHOT_FOLDER) {
        return Some(Err(format!("couldn't make {SHOT_FOLDER}: {err}")));
    }
    let (stamp, _) = crate::snapshot::timestamp(std::time::SystemTime::now());
    let path = std::path::Path::new(SHOT_FOLDER).join(format!("{stamp}.png"));
    world
        .spawn(Screenshot::primary_window())
        .observe(move |shot: On<ScreenshotCaptured>| {
            let image = shot.image.clone();
            let (path, reply) = (path.clone(), reply.clone());
            std::thread::spawn(move || {
                let width = image.width();
                let scale = width.div_ceil(SHOT_WIDTH).max(1);
                let saved = crate::snapshot::save_png(image, scale, &path)
                    .and_then(|()| std::fs::read(&path).map_err(|e| e.to_string()));
                let _ = reply.send(saved.map(|png| {
                    vec![
                        Content::Image(png),
                        Content::Text(format!("Saved {}", path.display())),
                    ]
                }));
            });
        });
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_read_whatever_their_case_and_spacing() {
        assert_eq!(
            class(&json!({ "class": "torpedo boat" })),
            Ok(EnemyClass::TorpedoBoat)
        );
        assert_eq!(
            class(&json!({ "class": "Iron Regent" })),
            Ok(EnemyClass::Regent)
        );
        assert!(class(&json!({ "class": "dinghy" })).is_err());
        assert_eq!(
            weight(&json!({ "weight": "ultra heavy" })),
            Ok(Some(Weight::UltraHeavy))
        );
        assert_eq!(
            weight(&json!({ "weight": "ultra" })),
            Ok(Some(Weight::UltraHeavy))
        );
        assert_eq!(weight(&json!({})), Ok(None));
    }

    #[test]
    fn a_range_and_bearing_are_measured_from_her() {
        let mut helm = PlayerHelm::default();
        helm.position = Vec2::new(100., 100.);
        helm.heading = 0.;
        let east = place(&json!({ "range": 500., "bearing": 90. }), &helm)
            .unwrap()
            .unwrap();
        assert!(east.abs_diff_eq(Vec2::new(600., 100.), 0.01), "{east}");
        let ahead = place(&json!({ "range": 200. }), &helm).unwrap().unwrap();
        assert!(ahead.abs_diff_eq(Vec2::new(100., -100.), 0.01), "{ahead}");
        assert_eq!(place(&json!({}), &helm), Ok(None));
        assert!(place(&json!({ "x": 1. }), &helm).is_err());
    }

    /// Stops compiling when a class or a card is added to the game: list it above too.
    #[allow(dead_code)]
    fn listed(class: EnemyClass, card: Card) {
        use EnemyClass::*;
        match class {
            Freighter | Coaster | Patrol | TorpedoBoat | Destroyer | Cruiser | Battleship
            | Commodore | Parliament | Regent => {}
        }
        use Card::*;
        match card {
            Loader | Shells | Turbines | Caps | Velocity | TripleTurrets | QuadTurrets
            | MoreSecondaries | HeavySecondaries | QuickSecondaries | SecondaryRange
            | TwinSecondaries | TrainingGear | Armor | Hardening | MoreAntiAir | HeavyAntiAir
            | QuickAntiAir | Pumps | MoreLaunchers | MoreMissiles | MorePlanes | MoreBombs
            | SecondaryTurrets => {}
        }
    }
}
