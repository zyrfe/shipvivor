//! Draw generated sector charts and report how the survey judged them.
//!
//! `cargo run -p shipvivor-encounter --example charts -- [waters] [first seed] [count]`
//!
//! `waters` is narrows, open, sound, cut or all. Each writes a contact sheet to
//! `work/charts/<waters>.svg` and prints each chart's score and measures, then the share
//! of raw sketches the survey passed and what failed the rest. Set `CHARTS_WHY` to list
//! the defects of the first few sketches that failed, and `CHARTS_TILE` to draw each
//! chart larger than 520 pixels a side. Streamlines trace the
//! solved current, shaded by speed, and the ice is drawn where it lies on arrival.
use shipvivor_encounter::{
    Battle, Chart, Course, ISLAND_RADIUS, LandmarkKind, SHORE_RANGE, Survey, Waters,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

/// Side of one chart's tile, unless `CHARTS_TILE` gives another.
fn tile_size() -> f32 {
    std::env::var("CHARTS_TILE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(520.)
}
const COLUMNS: usize = 4;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pick = args.first().map_or("all", String::as_str);
    let first: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let count: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(12);
    let all = [
        ("narrows", Waters::NARROWS),
        ("open", Waters::OPEN),
        ("sound", Waters::SOUND),
        ("cut", Waters::CUT),
    ];
    std::fs::create_dir_all("work/charts").expect("create work/charts");
    for (name, waters) in all.iter().filter(|(n, _)| pick == "all" || pick == *n) {
        let mut tiles = String::new();
        println!(
            "\n{name}: seed  score  bases(m) stages factories(m) turn(°) detour shore cover lanes islands  ms  set(m/s) peak  ice  sea(ms) glacier(m) coast"
        );
        for (i, seed) in (first..first + count).enumerate() {
            let clock = Instant::now();
            let generated = Chart::generate(seed, waters);
            let ms = clock.elapsed().as_secs_f32() * 1000.;
            let Some((chart, survey)) = generated else {
                println!("  {seed:>6}  no chart passed  {ms:.0}");
                continue;
            };
            let m = &survey.metrics;
            let clock = Instant::now();
            let battle = Battle::for_chart(9999., &chart).with_flagship();
            let sea_ms = clock.elapsed().as_secs_f32() * 1000.;
            println!(
                "  {seed:>6}  {:.2}  {:>6.0}  {:.2}  {:>6.0}  {:>4.0}  {:.2}  {:.2}  {:>2}  {:>2}  {:>2}  {ms:>5.0}  {:.2}  {:.2}  {:>2}  {sea_ms:>5.0}  {:>4.0}  {:>3}",
                survey.score,
                m.base_spacing,
                m.stage_balance,
                m.factory_spacing,
                m.sharpest_turn,
                m.worst_detour,
                m.shore_exposure,
                m.cover,
                m.convoy_lanes,
                m.islands,
                chart.set.flow.length(),
                battle.currents().peak(),
                chart.ice.len(),
                chart
                    .islets
                    .iter()
                    .chain(&chart.coast)
                    .filter(|i| i.kind == LandmarkKind::Glacier)
                    .map(|i| i.radius)
                    .fold(0., f32::max),
                chart.coast.len(),
            );
            let tile_side = tile_size();
            let (x, y) = (
                (i % COLUMNS) as f32 * tile_side,
                (i / COLUMNS) as f32 * tile_side,
            );
            tiles += &tile(&chart, &survey, &battle, x, y);
        }
        let rows = (count as usize).div_ceil(COLUMNS);
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Helvetica" font-size="11"><rect width="100%" height="100%" fill="#1d3b52"/>{tiles}</svg>"##,
            w = tile_size() * COLUMNS.min(count as usize) as f32,
            h = tile_size() * rows as f32
        );
        let path = format!("work/charts/{name}.svg");
        std::fs::write(&path, svg).expect("write chart sheet");
        println!("  wrote {path}");

        // How often a raw sketch passes, and why the rest fail.
        let mut failures: BTreeMap<String, usize> = BTreeMap::new();
        let sketches = 400;
        let mut passed = 0;
        let why = std::env::var_os("CHARTS_WHY").is_some();
        for seed in 0..sketches {
            let survey = Survey::of(&Chart::sketch_one(seed * 7919 + 13, waters));
            if survey.passed() {
                passed += 1;
            } else if why && seed < 12 {
                let defects: Vec<String> = survey.defects.iter().map(|d| d.to_string()).collect();
                println!("    sketch {seed}: {}", defects.join("; "));
            }
            for defect in &survey.defects {
                let kind = format!("{defect:?}");
                let kind = kind.split([' ', '{']).next().unwrap_or("").to_owned();
                *failures.entry(kind).or_default() += 1;
            }
        }
        println!("  {passed} of {sketches} raw sketches pass. Defects in the rest:");
        let mut sorted: Vec<_> = failures.into_iter().collect();
        sorted.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (kind, n) in sorted {
            println!("    {n:>4}  {kind}");
        }
    }
}

fn tile(chart: &Chart, survey: &Survey, battle: &Battle, ox: f32, oy: f32) -> String {
    #[allow(non_snake_case)]
    let TILE = tile_size();
    let mut points = vec![glam::Vec2::ZERO];
    points.extend(chart.gates.iter().copied());
    points.extend(chart.harbours.iter().map(|h| h.pos));
    points.extend(chart.islets.iter().map(|i| i.pos));
    let low = points
        .iter()
        .copied()
        .fold(glam::Vec2::MAX, glam::Vec2::min)
        - 160.;
    let high = points
        .iter()
        .copied()
        .fold(glam::Vec2::MIN, glam::Vec2::max)
        + 160.;
    let scale = (TILE - 30.) / (high - low).max_element();
    let middle = (high - low) * scale * 0.5;
    let at = |p: glam::Vec2| {
        let q = (p - low) * scale - middle;
        (ox + TILE * 0.5 + q.x, oy + TILE * 0.5 + 6. + q.y)
    };
    let mut s = String::new();
    let clip = format!("tile{}x{}", ox as u32, oy as u32);
    let _ = write!(
        s,
        r##"<clipPath id="{clip}"><rect x="{ox}" y="{oy}" width="{TILE}" height="{TILE}"/></clipPath><g clip-path="url(#{clip})">"##
    );
    s += &streamlines(battle, low, high, &at);
    for port in battle.ports() {
        for gun in port.turrets() {
            let (x, y) = at(gun.position(&port.body));
            let _ = write!(
                s,
                r##"<circle cx="{x}" cy="{y}" r="{}" fill="#c0392b" fill-opacity="0.08" stroke="#c0392b" stroke-opacity="0.4" stroke-dasharray="4 3"/>"##,
                SHORE_RANGE * scale
            );
        }
    }
    let mut path = String::new();
    let lap = (chart.course == Course::Circuit).then_some(chart.gates[0]);
    for (i, p) in std::iter::once(glam::Vec2::ZERO)
        .chain(chart.gates.iter().copied())
        .chain(lap)
        .enumerate()
    {
        let (x, y) = at(p);
        let _ = write!(path, "{}{x:.1},{y:.1} ", if i == 0 { "M" } else { "L" });
    }
    let _ = write!(
        s,
        r##"<path d="{path}" fill="none" stroke="#f1c40f" stroke-opacity="0.7" stroke-width="1.5" stroke-dasharray="6 4"/>"##
    );
    // The mainland first, so glaciers and crags show over it.
    let mut land: Vec<_> = battle.landmarks().iter().collect();
    land.sort_by_key(|island| island.kind != LandmarkKind::Mainland);
    for island in land {
        let (x, y) = at(island.body.pos);
        let fill = match island.kind {
            LandmarkKind::Beacon => "#c8b68a",
            LandmarkKind::Stacks => "#8d8a83",
            LandmarkKind::Pines => "#4f7a4a",
            LandmarkKind::Glacier => "#e4eef3",
            LandmarkKind::Mainland => "#a3aaa2",
        };
        let _ = write!(
            s,
            r##"<circle cx="{x}" cy="{y}" r="{}" fill="{fill}"/>"##,
            island.radius * scale
        );
    }
    for (i, port) in battle.ports().iter().enumerate() {
        let (x, y) = at(port.body.pos);
        let _ = write!(
            s,
            r##"<circle cx="{x}" cy="{y}" r="{}" fill="#b5651d"/>"##,
            ISLAND_RADIUS * scale
        );
        let (bx, by) = at(port.berth());
        let _ = write!(
            s,
            r##"<line x1="{x}" y1="{y}" x2="{bx}" y2="{by}" stroke="#e6edf2" stroke-width="2"/>"##
        );
        for f in 0..port.factories().len() {
            let (fx, fy) = at(port.factory_position(f));
            let _ = write!(
                s,
                r##"<rect x="{}" y="{}" width="4" height="4" fill="#2c2c2c"/>"##,
                fx - 2.,
                fy - 2.
            );
        }
        // Each base is lettered, with the stage it rouses in.
        let _ = write!(
            s,
            r##"<text x="{}" y="{}" fill="#ff8f70" font-weight="bold" font-size="14">{}<tspan font-size="9" font-weight="normal"> {}</tspan></text>"##,
            x + 10.,
            y - 10.,
            char::from(b'A' + i as u8),
            port.stage() + 1
        );
    }
    let mut previous = glam::Vec2::ZERO;
    for (i, g) in chart.gates.iter().enumerate() {
        let across = (*g - previous).normalize_or_zero().perp() * 14.;
        previous = *g;
        let ((x1, y1), (x2, y2)) = (at(*g - across), at(*g + across));
        let (x, y) = at(*g);
        let _ = write!(
            s,
            r##"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="#2ecc71" stroke-width="3"/><text x="{}" y="{}" fill="#2ecc71">{i}</text>"##,
            x + 6.,
            y + 4.
        );
    }
    for raider in battle.raiders() {
        let (x, y) = at(raider.body.pos);
        let _ = write!(
            s,
            r##"<polygon points="{},{} {},{} {},{}" fill="#e74c3c"/>"##,
            x,
            y - 5.,
            x - 4.,
            y + 4.,
            x + 4.,
            y + 4.
        );
    }
    for berg in battle.icebergs() {
        let (x, y) = at(berg.body.pos);
        let _ = write!(
            s,
            r##"<circle cx="{x}" cy="{y}" r="{}" fill="#bfe9f5" stroke="#ffffff" stroke-width="0.6"/>"##,
            (berg.radius * scale).max(1.2)
        );
    }
    for package in battle.salvage() {
        let (x, y) = at(package.body.pos);
        let _ = write!(
            s,
            r##"<rect x="{}" y="{}" width="4" height="4" fill="#f5b041"/>"##,
            x - 2.,
            y - 2.
        );
    }
    let (x, y) = at(glam::Vec2::ZERO);
    let _ = write!(
        s,
        r##"<circle cx="{x}" cy="{y}" r="5" fill="white"/><text x="{}" y="{}" fill="white">start</text>"##,
        x + 7.,
        y + 4.
    );
    let _ = write!(
        s,
        r##"<text x="{}" y="{}" fill="#e6edf2" stroke="#0e2333" stroke-width="3" paint-order="stroke">seed {} · score {:.2} · bases {:.0} m · set {:.2} m/s · peak {:.2}</text>"##,
        ox + 10.,
        oy + 15.,
        chart.seed.unwrap_or(0),
        survey.score,
        survey.metrics.base_spacing,
        chart.set.flow.length(),
        battle.currents().peak()
    );
    let _ = write!(
        s,
        r##"</g><rect x="{ox}" y="{oy}" width="{TILE}" height="{TILE}" fill="none" stroke="#0e2333" stroke-width="4"/>"##
    );
    s
}

/// Streamlines of the current from a lattice of seeds, each traced both ways and shaded
/// from slack (dark) to the fastest water on the chart (pale).
fn streamlines(
    battle: &Battle,
    low: glam::Vec2,
    high: glam::Vec2,
    at: &impl Fn(glam::Vec2) -> (f32, f32),
) -> String {
    let currents = battle.currents();
    let peak = currents.peak().max(0.05);
    let mut s = String::new();
    let spacing = (high - low).max_element() / 22.;
    let mut y = low.y + spacing * 0.5;
    while y < high.y {
        let mut x = low.x + spacing * 0.5;
        while x < high.x {
            for sign in [1., -1.] {
                let mut p = glam::Vec2::new(x, y);
                let mut path = String::new();
                let (px, py) = at(p);
                let _ = write!(path, "M{px:.1},{py:.1} ");
                let mut speed = 0.;
                for _ in 0..28 {
                    let v = currents.at(p);
                    if v.length() < 1e-3 {
                        break;
                    }
                    let mid = p + v.normalize() * 3.;
                    let step = currents.at(mid).normalize_or_zero() * sign * 6.;
                    speed += v.length();
                    p += step;
                    let (px, py) = at(p);
                    let _ = write!(path, "L{px:.1},{py:.1} ");
                }
                let shade = (speed / 28. / peak).clamp(0., 1.);
                let light = (60. + 170. * shade) as u8;
                let _ = write!(
                    s,
                    r##"<path d="{path}" fill="none" stroke="rgb({},{},{})" stroke-opacity="0.55" stroke-width="1"/>"##,
                    light / 2,
                    light,
                    light.saturating_add(20)
                );
            }
            x += spacing;
        }
        y += spacing;
    }
    s
}
