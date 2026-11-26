//! Sail simulated legs and report whether they can be won and how they were spent.
//!
//! `cargo run --release -p shipvivor-sim --example sim -- [stage] [first seed] [count] [temperaments]`
//!
//! `stage` is the first word of a sea's name: lantern, narrows (the default), blackcap,
//! outer, greywater, pilots, eider, walrus and so on through every round's pool, or
//! crown. A passage is sailed moderate unless `-tough` follows the name, as in
//! `pilots-tough`. Each seed is sailed once by each temperament (bold, steady and cautious, or a
//! comma-separated choice of them) on a fresh, stock ship, using the run seed the way
//! the game does, so `SHIPVIVOR_SEED=<seed> cargo run` sails the same waters. Each leg's
//! log goes to `work/sims/<stage>/<seed>-<temperament>.log`, and the summary is printed.
use shipvivor_campaign::{Grade, Stage};
use shipvivor_sim::bot::Temperament;
use shipvivor_sim::leg::{Activity, Outcome, Report, Setup, clock, sail};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// A sea by the first word of its name, lower case and without "The" or apostrophes:
/// lantern, iron or narrows, blackcap, outer, greywater, pilots, sealers, ... crown.
fn stage(name: &str) -> Option<Stage> {
    if name == "narrows" {
        return Some(Stage::IronNarrows);
    }
    shipvivor_campaign::POOLS
        .iter()
        .flatten()
        .copied()
        .chain([Stage::CrownDeep])
        .find(|sea| short_name(*sea) == name)
}

fn short_name(sea: Stage) -> String {
    let name = sea.mission().name.to_lowercase().replace('\'', "");
    let name = name.trim_start_matches("the ");
    name.split(' ').next().unwrap_or_default().to_string()
}

fn temperaments(list: &str) -> Vec<Temperament> {
    Temperament::ALL
        .into_iter()
        .filter(|t| list.split(',').any(|name| name == t.name()))
        .collect()
}

fn median(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.;
    }
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

fn mean(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, n) = values.fold((0., 0), |(s, n), v| (s + v, n + 1));
    if n == 0 { 0. } else { sum / n as f32 }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stage_name = args.first().map_or("narrows", String::as_str);
    let (sea_name, grade) = match stage_name.strip_suffix("-tough") {
        Some(name) => (name, Grade::Tough),
        None => (stage_name, Grade::Moderate),
    };
    let Some(stage) = stage(sea_name) else {
        eprintln!("unknown stage {stage_name}");
        std::process::exit(2);
    };
    let first: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let count: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(12);
    let chosen = args
        .get(3)
        .map_or(Temperament::ALL.to_vec(), |list| temperaments(list));
    let jobs: Vec<Setup> = (first..first + count)
        .flat_map(|seed| {
            chosen.iter().map(move |&temperament| Setup {
                stage,
                grade,
                seed,
                temperament,
            })
        })
        .collect();

    let started = std::time::Instant::now();
    let queue = Mutex::new(jobs.into_iter());
    let reports = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let Some(setup) = queue.lock().unwrap().next() else {
                        break;
                    };
                    let report = sail(setup);
                    reports.lock().unwrap().push(report);
                }
            });
        }
    });
    let mut reports = reports.into_inner().unwrap();
    reports.sort_by_key(|r| (r.setup.temperament, r.setup.seed));

    let folder = PathBuf::from("work/sims").join(stage_name);
    let _ = std::fs::create_dir_all(&folder);
    for r in &reports {
        let path = folder.join(format!(
            "{}-{}.log",
            r.setup.seed,
            r.setup.temperament.name()
        ));
        let _ = std::fs::write(path, r.log());
    }

    println!(
        "{}: {} legs of {} min in {:.1} s ({:.2} s a leg on {threads} threads). Logs in {}",
        stage.mission().name,
        reports.len(),
        stage.mission().minutes,
        started.elapsed().as_secs_f32(),
        mean(reports.iter().map(|r| r.wall)),
        folder.display()
    );
    println!();
    println!(
        "{:<6} {:<8} {:<10} {:>6} {:>5} {:>4} {:>5} {:>6} {:>5}  profile",
        "seed", "captain", "outcome", "time", "stars", "sunk", "yards", "low", "dmg/m"
    );
    for r in &reports {
        println!(
            "{:<6} {:<8} {:<10} {:>6} {:>5} {:>4} {:>2}/{:<2} {:>5}% {:>5.0}  {}",
            r.setup.seed,
            r.setup.temperament.name(),
            r.outcome.name(),
            clock(r.sailed),
            r.stars,
            r.kills,
            r.yards,
            r.bases,
            r.lowest_hull,
            r.damage as f32 / (r.sailed / 60.).max(1e-3),
            r.profile_line()
        );
    }

    println!();
    let mut by: BTreeMap<Temperament, Vec<&Report>> = BTreeMap::new();
    for r in &reports {
        by.entry(r.setup.temperament).or_default().push(r);
    }
    for (temperament, legs) in by {
        let outcomes = [
            Outcome::Cleared,
            Outcome::Survived,
            Outcome::RegentSunk,
            Outcome::Sunk,
            Outcome::Stalled,
        ]
        .into_iter()
        .filter_map(|o| {
            let n = legs.iter().filter(|r| r.outcome == o).count();
            (n > 0).then(|| format!("{} {}", o.name(), n))
        })
        .collect::<Vec<_>>()
        .join(", ");
        let shares = Activity::ALL
            .iter()
            .map(|a| {
                format!(
                    "{} {:.0}%",
                    a.name(),
                    mean(legs.iter().map(|r| r.share(*a))) * 100.
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let dead_air = legs.iter().filter(|r| r.longest_quiet > 60.).count();
        let swamped_lost = legs
            .iter()
            .filter(|r| {
                r.outcome == Outcome::Sunk
                    && r.seconds.get(&Activity::Swamped).copied().unwrap_or(0.) > 30.
            })
            .count();
        let fled_lost = legs
            .iter()
            .filter(|r| {
                r.outcome == Outcome::Sunk
                    && r.seconds.get(&Activity::Fleeing).copied().unwrap_or(0.) > 20.
            })
            .count();
        println!("{} ({} legs): {outcomes}", temperament.name(), legs.len());
        println!(
            "  median time {}, stars {:.1}, median lowest hull {:.0}%, damage {:.0}/min, hit rate {:.0}%, crawling {:.0}%",
            clock(median(legs.iter().map(|r| r.sailed).collect())),
            mean(legs.iter().map(|r| f32::from(r.stars))),
            median(legs.iter().map(|r| f32::from(r.lowest_hull)).collect()),
            mean(
                legs.iter()
                    .map(|r| r.damage as f32 / (r.sailed / 60.).max(1e-3))
            ),
            mean(
                legs.iter()
                    .map(|r| r.hits as f32 / (r.fired as f32).max(1.))
            ) * 100.,
            mean(legs.iter().map(|r| r.crawling / r.sailed.max(1e-3))) * 100.
        );
        println!("  {shares}");
        println!(
            "  warships under way {:.1}, within {} m {:.1}, sunk {:.1}/min, planes downed {:.1}/leg, torpedoed {:.2}/leg",
            mean(legs.iter().map(|r| r.mean_afloat)),
            shipvivor_sim::bot::THREAT_RANGE,
            mean(legs.iter().map(|r| r.mean_near)),
            mean(
                legs.iter()
                    .map(|r| r.kills as f32 / (r.sailed / 60.).max(1e-3))
            ),
            mean(legs.iter().map(|r| r.planes as f32)),
            mean(legs.iter().map(|r| r.torpedoed as f32))
        );
        println!(
            "  first contact {} (median), longest quiet {} (median); dead air in {dead_air}, swamped then sunk in {swamped_lost}, ran and still sunk in {fled_lost}",
            clock(median(
                legs.iter().filter_map(|r| r.first_contact).collect()
            )),
            clock(median(legs.iter().map(|r| r.longest_quiet).collect()))
        );
    }
}
