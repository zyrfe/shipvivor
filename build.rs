//! Numbers each release build, so a round can be pinned to the binary that sailed it.
//!
//! The count lives in `work/build-number`: out of git, out of `target` so `cargo clean` keeps
//! it, and outside the watched paths so writing it doesn't trigger another run. Only release
//! builds count, since `cargo run --release` is how the game is played. Other profiles leave
//! `SHIPVIVOR_BUILD` unset, and the game calls itself a dev build.

use std::{env, fs, path::PathBuf};

fn main() {
    // Run again only when the workspace's code or manifests change. Watching nothing would
    // fall back to every file in the package, `work` included.
    for watched in ["build.rs", "Cargo.toml", "Cargo.lock", "src", "crates"] {
        println!("cargo::rerun-if-changed={watched}");
    }
    if env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let file = root.join("work").join("build-number");
    let last: u32 = fs::read_to_string(&file)
        .ok()
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(0);
    let number = last + 1;
    // Write beside it and rename, so an interrupted build can't leave the count empty.
    let staged = file.with_extension("tmp");
    let saved = fs::create_dir_all(root.join("work"))
        .and_then(|()| fs::write(&staged, format!("{number}\n")))
        .and_then(|()| fs::rename(&staged, &file));
    if let Err(error) = saved {
        println!(
            "cargo::warning=build {number} could not be saved to {}: {error}",
            file.display()
        );
    }
    println!("cargo::rustc-env=SHIPVIVOR_BUILD={number}");
}
