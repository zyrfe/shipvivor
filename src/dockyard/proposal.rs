//! The preview column reports figures from the same proposed design the sea receives.
//! Each figure is a pictogram with the value now and the value after, coloured by
//! whether the change helps, so the column reads without labels.
use super::*;
use cards::Change;
use shipvivor_shipyard::GunStation;

const KNOTS: f32 = 1.94384;

/// What a figure is about: a pictogram, or a word where no pictogram fits.
enum Mark {
    Icon(Handle<Image>),
    Word(&'static str),
}

fn figure(
    cmd: &mut Commands,
    parent: Entity,
    art: &Insignia,
    mark: Mark,
    before: String,
    after: String,
    color: Color,
) {
    let row = node(
        cmd,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(7),
            ..default()
        },
        Color::NONE,
    );
    match mark {
        Mark::Icon(image) => icon(cmd, row, image, muted(), 16.),
        Mark::Word(word) => text(cmd, row, word, 12., muted()),
    }
    if !before.is_empty() && before != after {
        text(cmd, row, before, 14., muted());
        icon(cmd, row, art.arrow.clone(), muted(), 10.);
    }
    text(cmd, row, after, 15., color);
}

/// A figure whose colour follows from comparing two numbers. `higher` says whether
/// more is better; `None` leaves it neutral.
#[allow(clippy::too_many_arguments)]
fn delta(
    cmd: &mut Commands,
    parent: Entity,
    art: &Insignia,
    image: Handle<Image>,
    (before, after): (f32, f32),
    unit: &str,
    precision: usize,
    higher: Option<bool>,
) {
    let color = match higher {
        _ if (after - before).abs() < 1e-3 => paper(),
        Some(true) if after > before => Change::Gain.color(),
        Some(false) if after < before => Change::Gain.color(),
        Some(_) => Change::Cost.color(),
        None => paper(),
    };
    figure(
        cmd,
        parent,
        art,
        Mark::Icon(image),
        format!("{before:.precision$} {unit}")
            .trim_end()
            .to_string(),
        format!("{after:.precision$} {unit}").trim_end().to_string(),
        color,
    );
}

/// Why a refit cannot be made, in a few words beside a padlock.
fn note(cmd: &mut Commands, parent: Entity, art: &Insignia, words: impl Into<String>) {
    let row = node(
        cmd,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(7),
            ..default()
        },
        Color::NONE,
    );
    icon(cmd, row, art.lock.clone(), brass(), 16.);
    text(cmd, row, words, 13., brass());
}

fn hint(cmd: &mut Commands, parent: Entity, words: impl Into<String>) {
    text(cmd, parent, words, 12., cyan());
}

/// Main turrets aboard as a row of pips, out of the most the ship can carry.
pub(super) fn pips(cmd: &mut Commands, parent: Entity, filled: &[(usize, Color)]) {
    let row = node(
        cmd,
        parent,
        Node {
            column_gap: px(3),
            align_items: AlignItems::Center,
            ..default()
        },
        Color::NONE,
    );
    let mut colours = filled
        .iter()
        .flat_map(|&(count, color)| std::iter::repeat_n(color, count));
    for _ in 0..shipvivor_campaign::MAX_GUN_STATIONS {
        let color = colours.next();
        cmd.spawn((
            Node {
                width: px(9),
                height: px(9),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(color.unwrap_or(Color::NONE)),
            BorderColor::all(color.unwrap_or(rule())),
            ChildOf(row),
        ));
    }
}

fn turret_change(cmd: &mut Commands, parent: Entity, art: &Insignia, before: usize, after: usize) {
    let row = node(
        cmd,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(7),
            ..default()
        },
        Color::NONE,
    );
    icon(cmd, row, art.turret.clone(), muted(), 16.);
    pips(cmd, row, &[(before, muted())]);
    icon(cmd, row, art.arrow.clone(), muted(), 10.);
    let color = if after > before {
        Change::Gain.color()
    } else {
        Change::Cost.color()
    };
    pips(cmd, row, &[(after, color)]);
}

/// The pictogram for one of a fitting card's figures, where there is one.
fn stat_art(label: &str, card: Card, art: &Insignia) -> Option<Handle<Image>> {
    let secondary = matches!(
        card,
        Card::MoreSecondaries
            | Card::HeavySecondaries
            | Card::QuickSecondaries
            | Card::SecondaryRange
            | Card::TwinSecondaries
    );
    Some(match label {
        "Reload" if secondary => art.fitting(Card::QuickSecondaries),
        "Reload" => art.fitting(Card::Loader),
        "AP opens" | "AP at range" => art.fitting(Card::ArmorPiercing),
        "Speed" | "Top speed" => art.fitting(Card::Velocity),
        "Barrels" if secondary => art.fitting(Card::TwinSecondaries),
        "Barrels" => art.fitting(Card::TripleTurrets),
        "Guns a side" => art.fitting(Card::MoreSecondaries),
        "Reach" => art.fitting(Card::SecondaryRange),
        "Damage" => art.fitting(Card::HeavySecondaries),
        "Calibre" => art.fitting(Card::Shells),
        _ => return None,
    })
}

pub(super) fn details(
    cmd: &mut Commands,
    spec: Entity,
    run: &Campaign,
    art: &Insignia,
    job: DockJob,
) {
    let design = run.ship.design();
    let current = run.fittings;
    let fits = job.fits(run);
    let speed = |top: f32| top * KNOTS * current.speed_factor();
    match job {
        DockJob::WidenBeam => {
            let Some(next) = run.next_beam() else {
                note(cmd, spec, art, "Widest beam fitted");
                return;
            };
            delta(
                cmd,
                spec,
                art,
                art.beam.clone(),
                (design.beam(), next.metres()),
                "m",
                1,
                Some(true),
            );
            for (image, paired) in [
                (art.turret.clone(), false),
                (art.wing_turrets.clone(), true),
            ] {
                let before = current.capacity_for(run.beam_class(), paired);
                let after = current.capacity_for(next, paired);
                figure(
                    cmd,
                    spec,
                    art,
                    Mark::Icon(image),
                    if before == 0 {
                        "-".into()
                    } else {
                        format!("{before} in")
                    },
                    format!("{after} in"),
                    Change::Gain.color(),
                );
            }
            if let Some(after) = run.projected_design(job) {
                delta(
                    cmd,
                    spec,
                    art,
                    art.fitting(Card::Velocity),
                    (speed(design.top_speed()), speed(after.top_speed())),
                    "kn",
                    1,
                    Some(true),
                );
            }
            if run.beam_class().next() != Some(next) {
                hint(cmd, spec, "Two stars unlock the capital beam");
            }
        }
        DockJob::Calibre(inches) => {
            let old = current.round().calibre_inches() as u16;
            let stations = design.gun_stations();
            let paired_only = stations.iter().all(GunStation::paired);
            if old == inches {
                hint(cmd, spec, "Fitted");
                return;
            }
            if !fits {
                let twin = shipvivor_campaign::Fittings {
                    turrets: shipvivor_encounter::TurretLayout::Twin,
                    ..current
                };
                let why = if let Some(b) = catalog::required_beam(current, inches, paired_only) {
                    format!("Needs {} beam, {:.1} m", b.name(), b.metres())
                } else if let Some(b) = catalog::required_beam(twin, inches, paired_only) {
                    format!("Needs twin turrets, {} beam", b.name())
                } else if let Some(b) = catalog::required_beam(current, inches, false) {
                    format!("Needs a centre turret, {} beam", b.name())
                } else {
                    "Needs twin centre turrets, Capital beam".into()
                };
                note(cmd, spec, art, why);
            }
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Shells),
                (f32::from(old), f32::from(inches)),
                "in",
                0,
                Some(true),
            );
            if fits {
                let planned = run.projected_fittings(job);
                delta(
                    cmd,
                    spec,
                    art,
                    art.fitting(Card::Loader),
                    (current.reload(), planned.reload()),
                    "s",
                    1,
                    Some(false),
                );
                if let Some(wing) = stations.iter().find(|s| s.paired()) {
                    figure(
                        cmd,
                        spec,
                        art,
                        Mark::Icon(art.wing_turrets.clone()),
                        String::new(),
                        format!(
                            "{:.0} in",
                            planned.for_station(wing).round().calibre_inches()
                        ),
                        paper(),
                    );
                }
            }
        }
        DockJob::Move { from, .. } | DockJob::Replace { at: from, .. } => {
            // A move is always between adjacent frames, so the old and new station correspond.
            let to = match job {
                DockJob::Move { to, .. } if fits => to,
                _ => from,
            };
            module_details(cmd, spec, run, art, job, from, to);
        }
        DockJob::Section(kind) if fits => {
            let before = design.sheet();
            let after = run.projected_sheet(kind);
            delta(
                cmd,
                spec,
                art,
                art.length.clone(),
                (before.length, after.length),
                "m",
                0,
                None,
            );
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Velocity),
                (speed(before.top_speed), speed(after.top_speed)),
                "kn",
                1,
                Some(true),
            );
            delta(
                cmd,
                spec,
                art,
                art.turn.clone(),
                (
                    before.turn_radius * current.speed_factor(),
                    after.turn_radius * current.speed_factor(),
                ),
                "m",
                0,
                Some(false),
            );
            // On the last order the dock's own warning says the bay will sail empty.
            if kind == DockModule::Reserve && !run.closes_dock(job) {
                hint(cmd, spec, "Then select the new bay for a turret");
            }
        }
        DockJob::Section(kind) => {
            let count = run.dock_modules().filter(|(_, m)| *m == kind).count();
            note(
                cmd,
                spec,
                art,
                format!(
                    "{count} of {} fitted",
                    shipvivor_campaign::MAX_SECTIONS_PER_KIND
                ),
            );
        }
        DockJob::Fitting(Card::TripleTurrets | Card::QuadTurrets) | DockJob::TwinTurrets => {
            if !fits {
                note(cmd, spec, art, "Too heavy for this beam and bore");
                return;
            }
            let after = run.projected_fittings(job);
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::TripleTurrets),
                (current.turrets.count() as f32, after.turrets.count() as f32),
                "",
                0,
                Some(true),
            );
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Shells),
                (
                    f32::from(current.capacity_for(run.beam_class(), false)),
                    f32::from(after.capacity_for(run.beam_class(), false)),
                ),
                "in",
                0,
                Some(true),
            );
        }
        DockJob::Fitting(_) if !fits => note(cmd, spec, art, "Fully fitted"),
        DockJob::Fitting(Card::Turbines) => {
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Velocity),
                (
                    speed(design.top_speed()),
                    design.top_speed() * KNOTS * current.with(Card::Turbines).speed_factor(),
                ),
                "kn",
                1,
                Some(true),
            );
        }
        DockJob::Fitting(card) => {
            for stat in cards::face(card, current).stats {
                let mark =
                    stat_art(stat.label, card, art).map_or(Mark::Word(stat.label), Mark::Icon);
                let color = if stat.before.is_empty() {
                    Change::New.color()
                } else {
                    stat.change.color()
                };
                figure(cmd, spec, art, mark, stat.before, stat.after, color);
            }
        }
    }
}

fn module_details(
    cmd: &mut Commands,
    spec: Entity,
    run: &Campaign,
    art: &Insignia,
    job: DockJob,
    from: usize,
    to: usize,
) {
    let design = run.ship.design();
    let Some(proposed) = run.projected_design(job) else {
        let guns = design.gun_stations();
        let remaining_guns = guns.iter().filter(|g| g.berth.module != from).count();
        let why = match job {
            DockJob::Move { .. } => "Bow and stern are fixed",
            _ if run.module_locked(from) => "Machinery stays aboard",
            DockJob::Replace {
                with: DeckModule::Reserve,
                ..
            } if remaining_guns == 0 => "Keep one main turret",
            DockJob::Replace { with, .. } if with.name() == run.module_name(from) => {
                "Already fitted"
            }
            DockJob::Replace {
                with: DeckModule::Reserve,
                ..
            } if run
                .dock_modules()
                .filter(|(_, kind)| *kind == DockModule::Reserve)
                .count()
                >= shipvivor_campaign::MAX_SECTIONS_PER_KIND =>
            {
                "Two reserve bays at most"
            }
            _ => "Four main turrets at most",
        };
        note(cmd, spec, art, why);
        return;
    };
    let original = design.gun_stations();
    let proposed_guns = proposed.gun_stations();
    if proposed_guns.len() != original.len() {
        turret_change(cmd, spec, art, original.len(), proposed_guns.len());
    }
    let local: Vec<_> = proposed_guns
        .iter()
        .filter(|s| s.berth.module == to)
        .collect();
    let fittings = run.projected_fittings(job);
    let travel = |gun: &GunStation| 2. * (gun.half_arc - gun.inner_mask).to_degrees();
    if let Some(first) = local.first() {
        // The guns on this bay: their mount, their bore and their barrels.
        let battery = fittings.for_station(first);
        let row = node(
            cmd,
            spec,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(7),
                ..default()
            },
            Color::NONE,
        );
        let mount = if local.len() == 2 {
            art.wing_turrets.clone()
        } else {
            art.turret.clone()
        };
        icon(cmd, row, mount, muted(), 16.);
        text(
            cmd,
            row,
            format!("{:.0} in", battery.round().calibre_inches()),
            15.,
            paper(),
        );
        let barrels = match battery.turrets.count() {
            2 => art.twin.clone(),
            3 => art.fitting(Card::TripleTurrets),
            _ => art.fitting(Card::QuadTurrets),
        };
        icon(cmd, row, barrels, paper(), 16.);
        let old = original.iter().find(|s| s.berth.module == from).map(travel);
        arcs(cmd, spec, art, &local, old, travel(first));
    } else {
        let barrels = |guns: &[GunStation], fittings: shipvivor_campaign::Fittings| {
            guns.iter()
                .map(|g| fittings.for_station(g).turrets.count())
                .sum::<usize>() as f32
        };
        delta(
            cmd,
            spec,
            art,
            art.fitting(Card::TripleTurrets),
            (
                barrels(&original, run.fittings),
                barrels(&proposed_guns, fittings),
            ),
            "",
            0,
            Some(true),
        );
        let coverage = |guns: &[GunStation]| {
            (0..360)
                .filter(|d| guns.iter().any(|g| g.bears((*d as f32).to_radians())))
                .count() as f32
        };
        delta(
            cmd,
            spec,
            art,
            art.fitting(Card::SecondaryRange),
            (coverage(&original), coverage(&proposed_guns)),
            "deg",
            0,
            Some(true),
        );
    }
    // Centre and wing bores only need showing once a wider bore separates them.
    let bore = |gun: &GunStation| fittings.for_station(gun).round().calibre_inches();
    if let (Some(centre), Some(wing)) = (
        proposed_guns.iter().find(|g| !g.paired()),
        proposed_guns.iter().find(|g| g.paired()),
    ) && bore(centre) != bore(wing)
    {
        let row = node(
            cmd,
            spec,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(7),
                ..default()
            },
            Color::NONE,
        );
        for (image, gun) in [
            (art.turret.clone(), centre),
            (art.wing_turrets.clone(), wing),
        ] {
            icon(cmd, row, image, muted(), 16.);
            text(cmd, row, format!("{:.0} in  ", bore(gun)), 15., paper());
        }
    }
}

// The plot is drawn as the ship lies in the dock: bow to the left, starboard up. Each
// ring is the train of one turret, so a wing pair's two sides stay distinguishable.
fn arcs(
    cmd: &mut Commands,
    parent: Entity,
    art: &Insignia,
    stations: &[&GunStation],
    before: Option<f32>,
    after: f32,
) {
    let row = node(
        cmd,
        parent,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(10),
            ..default()
        },
        Color::NONE,
    );
    let plot = node(
        cmd,
        row,
        Node {
            width: px(80),
            height: px(64),
            ..default()
        },
        Color::NONE,
    );
    let dot = |cmd: &mut Commands, x: f32, y: f32, w: f32, h: f32, c: Color| {
        node(
            cmd,
            plot,
            Node {
                position_type: PositionType::Absolute,
                left: px(x - w * 0.5),
                top: px(y - h * 0.5),
                width: px(w),
                height: px(h),
                ..default()
            },
            c,
        );
    };
    dot(cmd, 40., 32., 22., 8., muted());
    dot(cmd, 28., 32., 4., 4., paper());
    for (i, station) in stations.iter().enumerate() {
        let radius = 28. - 4.5 * i as f32;
        for n in 0..48 {
            let angle = n as f32 * std::f32::consts::TAU / 48.;
            let color = if !station.bears(angle) {
                rule()
            } else if i == 0 {
                cyan()
            } else {
                brass()
            };
            dot(
                cmd,
                40. - radius * angle.cos(),
                32. - radius * angle.sin(),
                2.8,
                2.8,
                color,
            );
        }
    }
    let sweep = node(
        cmd,
        row,
        Node {
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        },
        Color::NONE,
    );
    if let Some(before) = before.filter(|b| (b - after).abs() > 0.5) {
        text(cmd, sweep, format!("{before:.0}"), 14., muted());
        icon(cmd, sweep, art.arrow.clone(), muted(), 10.);
        let color = if after > before {
            Change::Gain.color()
        } else {
            Change::Cost.color()
        };
        text(cmd, sweep, format!("{after:.0} deg"), 15., color);
    } else {
        text(cmd, sweep, format!("{after:.0} deg"), 15., paper());
    }
}
