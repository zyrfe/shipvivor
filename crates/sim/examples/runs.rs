//! Sail whole runs with captains of several skills and report how hard a run is.
//!
//! `cargo run --release -p shipvivor-sim --example runs -- [options]`
//!
//! - `--seeds N` runs per captain (default 24), from `--first S` (default 1)
//! - `--skills a,b,..` captains' skills, 0 a novice to 1 the bot at her best
//!   (default 0.2,0.4,0.6,0.8,1)
//! - `--pressures p,q,..` how hard enemy blows land, 1 the game as it stands (default 1)
//! - `--shape a,b,..` a further share of that for each round from the first, to try a
//!   new `PRESSURE` table in crates/sim/src/sector.rs before building it in (default 1s)
//! - `--lanes tough|moderate` which passages she takes (default tough)
//! - `--temperament bold|steady|cautious` (default steady)
//! - `--bell MEAN,SPREAD` the players' skills as a bell curve (default 0.6,0.15)
//!
//! For each pressure it prints the share of runs won at each skill, the round each run
//! ended in, the share of captains lost in each round they sailed and their median
//! lowest hull there, a fitted curve of the chance to win against skill, and what a
//! field of players whose skills fall on the bell curve would make of it. Each run's log
//! goes to `work/sims/runs/`.
use shipvivor_campaign::ROUNDS;
use shipvivor_sim::bot::{Skill, Temperament};
use shipvivor_sim::leg::clock;
use shipvivor_sim::run::{Ending, Lanes, RunReport, RunSetup, sail_run};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

struct Options {
    seeds: u64,
    first: u64,
    skills: Vec<f32>,
    pressures: Vec<f32>,
    shape: [f32; ROUNDS],
    lanes: Lanes,
    temperament: Temperament,
    bell: (f32, f32),
}

fn list(text: &str) -> Vec<f32> {
    text.split(',')
        .filter_map(|v| v.trim().parse().ok())
        .collect()
}

fn options() -> Options {
    let mut o = Options {
        seeds: 24,
        first: 1,
        skills: vec![0.2, 0.4, 0.6, 0.8, 1.],
        pressures: vec![1.],
        shape: [1.; ROUNDS],
        lanes: Lanes::Tough,
        temperament: Temperament::Steady,
        bell: (0.6, 0.15),
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.chunks(2) {
        let (flag, value) = (pair[0].as_str(), pair.get(1).map_or("", String::as_str));
        match flag {
            "--seeds" => o.seeds = value.parse().unwrap_or(o.seeds),
            "--first" => o.first = value.parse().unwrap_or(o.first),
            "--skills" => o.skills = list(value),
            "--pressures" => o.pressures = list(value),
            "--shape" => {
                for (slot, v) in o.shape.iter_mut().zip(list(value)) {
                    *slot = v;
                }
            }
            "--lanes" => {
                o.lanes = if value == "moderate" {
                    Lanes::Moderate
                } else {
                    Lanes::Tough
                }
            }
            "--temperament" => {
                if let Some(t) = Temperament::ALL.into_iter().find(|t| t.name() == value) {
                    o.temperament = t;
                }
            }
            "--bell" => {
                if let [mean, spread] = list(value)[..] {
                    o.bell = (mean, spread);
                }
            }
            other => {
                eprintln!("unknown option {other}");
                std::process::exit(2);
            }
        }
    }
    o
}

fn median(mut values: Vec<f32>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f32::total_cmp);
    Some(values[values.len() / 2])
}

/// The chance a captain gets through every round, estimated round by round as a
/// survival study does (Kaplan and Meier): the product, over the rounds, of the share of
/// those who sailed each round who were not lost in it. A run stuck in a round, the bot
/// unable to win it or lose it, counts among those who sailed that round and is then left
/// out, rather than being counted a win or a loss.
fn through(runs: &[&RunReport]) -> f32 {
    (1..=ROUNDS)
        .map(|round| {
            let sailed = (runs.iter())
                .filter(|r| r.legs.iter().any(|(at, ..)| *at == round))
                .count();
            let lost = (runs.iter())
                .filter(|r| r.ending == Ending::Lost(round))
                .count();
            if sailed == 0 {
                1.
            } else {
                1. - lost as f32 / sailed as f32
            }
        })
        .product()
}

/// A logistic curve through each skill's chance of getting through, weighted by its runs,
/// fitted over a grid: the skill at which half get through, and how steeply the chance
/// rises there, per unit of skill. `None` when every skill's chance is nought, or sure.
fn fit(points: &[(f32, f32, usize)]) -> Option<(f32, f32)> {
    if points.iter().all(|p| p.1 < 0.02) || points.iter().all(|p| p.1 > 0.98) {
        return None;
    }
    let mut best = (f32::INFINITY, 0., 0.);
    for i in 0..=300 {
        let middle = -1. + i as f32 * 0.01;
        for j in 1..=80 {
            let slope = j as f32 * 0.5;
            let error: f32 = (points.iter())
                .map(|&(skill, chance, n)| {
                    n as f32 * (logistic(skill, (middle, slope)) - chance).powi(2)
                })
                .sum();
            if error < best.0 {
                best = (error, middle, slope);
            }
        }
    }
    Some((best.1, best.2))
}

fn logistic(skill: f32, (middle, slope): (f32, f32)) -> f32 {
    1. / (1. + (-slope * (skill - middle)).exp())
}

fn normal_pdf(x: f32, mean: f32, spread: f32) -> f32 {
    (-0.5 * ((x - mean) / spread).powi(2)).exp()
}

/// The standard normal's cumulative share below `z`, for reading a skill as a percentile.
fn normal_cdf(z: f32) -> f32 {
    // Abramowitz and Stegun 7.1.26, good to about 1e-7.
    let t = 1. / (1. + 0.327_591_1 * z.abs() / std::f32::consts::SQRT_2);
    let poly = t
        * (0.254_829_6
            + t * (-0.284_496_7 + t * (1.421_413_7 + t * (-1.453_152 + t * 1.061_405_4))));
    let erf = 1. - poly * (-(z * z) / 2.).exp();
    if z >= 0. {
        0.5 * (1. + erf)
    } else {
        0.5 * (1. - erf)
    }
}

fn bar(share: f32) -> String {
    "#".repeat((share * 40.).round() as usize)
}

fn log(r: &RunReport) -> String {
    let s = &r.setup;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# run seed {} skill {:.2} {} {} lanes, pressure {:.2}: {:?}; level {}, {:.0} m, guns {:?}",
        s.seed,
        s.skill.0,
        s.temperament.name(),
        s.lanes.name(),
        s.pressure,
        r.ending,
        r.level,
        r.length,
        r.guns
    );
    for (i, (round, grade, leg)) in r.legs.iter().enumerate() {
        let _ = writeln!(
            out,
            "round {round} {} ({}): {} after {}, {} stars, {}/{} bases, lowest hull {}%, took {} damage, {} sunk, main {}/{} hit, level {}",
            leg.setup.stage.mission().name,
            grade.name().to_lowercase(),
            leg.outcome.name(),
            clock(leg.sailed),
            leg.stars,
            leg.yards,
            leg.bases,
            leg.lowest_hull,
            leg.damage,
            leg.kills,
            leg.hits,
            leg.fired,
            leg.level
        );
        // A leg she could neither win nor lose, pulse by pulse, to see why.
        if matches!(r.ending, Ending::Stuck(_)) && i + 1 == r.legs.len() {
            out.push_str(&leg.log());
        }
        if let Some(refit) = r.refits.get(i) {
            let _ = writeln!(
                out,
                "  yard: spent {}, kept {}: {:?}",
                refit.spent, refit.left, refit.jobs
            );
        }
    }
    out
}

fn main() {
    let o = options();
    let mut jobs = Vec::new();
    for &pressure in &o.pressures {
        for &skill in &o.skills {
            for seed in o.first..o.first + o.seeds {
                jobs.push(RunSetup {
                    seed,
                    temperament: o.temperament,
                    skill: Skill(skill),
                    lanes: o.lanes,
                    pressure,
                    shape: o.shape,
                });
            }
        }
    }
    let total = jobs.len();
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
                    let report = sail_run(setup);
                    reports.lock().unwrap().push(report);
                }
            });
        }
    });
    let reports = reports.into_inner().unwrap();
    let folder = PathBuf::from("work/sims/runs");
    let _ = std::fs::create_dir_all(&folder);
    for r in &reports {
        let s = &r.setup;
        let name = format!(
            "p{:.2}-s{:.2}-{}-{}.log",
            s.pressure,
            s.skill.0,
            s.lanes.name(),
            s.seed
        );
        let _ = std::fs::write(folder.join(name), log(r));
    }
    println!(
        "{total} runs of {} {} captains on {} lanes, round shape {:?}, in {:.0} s on {threads} threads. Logs in {}",
        o.seeds,
        o.temperament.name(),
        o.lanes.name(),
        o.shape,
        started.elapsed().as_secs_f32(),
        folder.display()
    );

    for &pressure in &o.pressures {
        let at = |skill: f32| -> Vec<&RunReport> {
            reports
                .iter()
                .filter(|r| r.setup.pressure == pressure && r.setup.skill.0 == skill)
                .collect()
        };
        println!("\n== pressure {pressure:.2}");
        // Where the runs ended.
        let mut header = String::from("skill   won  through  ended in round:");
        for round in 1..=ROUNDS {
            let _ = write!(header, " {round:>4}");
        }
        header.push_str("  stuck  median round reached");
        println!("{header}");
        let mut ended: Vec<(f32, Vec<f32>)> = Vec::new();
        for &skill in &o.skills {
            let runs = at(skill);
            let n = runs.len().max(1) as f32;
            let won = runs.iter().filter(|r| r.ending == Ending::Won).count() as f32 / n;
            let mut line = format!(
                "{skill:>5.2}  {:>3.0}%     {:>3.0}%                ",
                won * 100.,
                through(&runs) * 100.
            );
            let mut shares = Vec::new();
            for round in 1..=ROUNDS {
                let lost = runs
                    .iter()
                    .filter(|r| r.ending == Ending::Lost(round))
                    .count() as f32
                    / n;
                shares.push(lost);
                let _ = write!(line, " {:>3.0}%", lost * 100.);
            }
            let stuck = runs
                .iter()
                .filter(|r| matches!(r.ending, Ending::Stuck(_)))
                .count() as f32
                / n;
            let _ = write!(line, "  {:>4.0}%", stuck * 100.);
            let reached = median(
                runs.iter()
                    .map(|r| r.ending.round().min(ROUNDS + 1) as f32)
                    .collect(),
            )
            .unwrap_or(0.);
            let _ = write!(
                line,
                "  {}",
                if reached > ROUNDS as f32 {
                    "won".to_string()
                } else {
                    format!("{reached:.0}")
                }
            );
            shares.push(won);
            ended.push((skill, shares));
            println!("{line}");
        }
        // Round by round: the share lost of those who sailed it, and how low they got.
        println!("\nlost in each round, of those who sailed it / median lowest hull there");
        let mut header = String::from("skill");
        for round in 1..=ROUNDS {
            let _ = write!(header, " {:>10}", format!("round {round}"));
        }
        println!("{header}");
        for &skill in &o.skills {
            let runs = at(skill);
            let mut line = format!("{skill:>5.2}");
            for round in 1..=ROUNDS {
                let sailed: Vec<&RunReport> = runs
                    .iter()
                    .copied()
                    .filter(|r| r.legs.iter().any(|(at, ..)| *at == round))
                    .collect();
                if sailed.is_empty() {
                    let _ = write!(line, " {:>10}", "-");
                    continue;
                }
                let lost = sailed
                    .iter()
                    .filter(|r| r.ending == Ending::Lost(round))
                    .count() as f32
                    / sailed.len() as f32;
                let low = median(
                    sailed
                        .iter()
                        .flat_map(|r| r.legs.iter().filter(|(at, ..)| *at == round))
                        .map(|(.., leg)| f32::from(leg.lowest_hull))
                        .collect(),
                )
                .unwrap_or(0.);
                let _ = write!(line, " {:>10}", format!("{:.0}%/{:.0}%", lost * 100., low));
            }
            println!("{line}");
        }
        // The curve, through each skill's chance of getting through.
        let points: Vec<(f32, f32, usize)> = (o.skills.iter())
            .map(|&skill| {
                let runs = at(skill);
                (skill, through(&runs), runs.len())
            })
            .collect();
        let (mean, spread) = o.bell;
        match fit(&points) {
            Some(curve) => {
                let percentile = normal_cdf((curve.0 - mean) / spread) * 100.;
                println!(
                    "\nhalf get through at skill {:.2}, the {:.0}th percentile of the bell curve; the chance rises {:.0} points per tenth of skill there",
                    curve.0,
                    percentile,
                    curve.1 * 0.1 * 25.
                );
                // A field of players on the bell curve.
                let steps = 200;
                let (mut won, mut weight) = (0., 0.);
                for i in 0..=steps {
                    let skill = mean - 3. * spread + 6. * spread * i as f32 / steps as f32;
                    let w = normal_pdf(skill, mean, spread);
                    won += w * logistic(skill.clamp(0., 1.), curve);
                    weight += w;
                }
                println!(
                    "players with skill ~ N({mean:.2}, {spread:.2}): {:.0}% would get through a run",
                    won / weight * 100.
                );
            }
            None => println!(
                "\nat every skill sailed, the chance of getting through is nought or sure: no curve to fit"
            ),
        }
        // Where a field of players on the bell curve would end, from the skills sailed.
        let weights: Vec<f32> = ended
            .iter()
            .map(|(skill, _)| normal_pdf(*skill, mean, spread))
            .collect();
        let total: f32 = weights.iter().sum();
        if total > 0. {
            println!("where that field's runs end:");
            for round in 0..=ROUNDS {
                let share: f32 = ended
                    .iter()
                    .zip(&weights)
                    .map(|((_, shares), w)| shares[round] * w)
                    .sum::<f32>()
                    / total;
                let name = if round == ROUNDS {
                    "won".to_string()
                } else {
                    format!("lost in round {}", round + 1)
                };
                println!("  {name:>16} {:>4.0}% {}", share * 100., bar(share));
            }
        }
    }
}
