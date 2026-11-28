//! The preview column reports figures from the same proposed design the sea receives.
//! Each figure is a pictogram with the value now and the value after, coloured by
//! whether the change helps, so the column reads without labels.
use super::*;
use cards::Change;
use shipvivor_encounter::TurretLayout;
use shipvivor_shipyard::GunStation;

const KNOTS: f32 = 1.94384;

/// What a figure is about: a pictogram, or a word where no pictogram fits.
enum Mark {
    Icon(Handle<Image>),
    Word(std::borrow::Cow<'static, str>),
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
        Mark::Word(word) => text(cmd, row, word.into_owned(), 12., muted()),
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

/// Main turrets aboard as a row of pips, out of the `most` the ship can carry.
pub(super) fn pips(cmd: &mut Commands, parent: Entity, filled: &[(usize, Color)], most: usize) {
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
    for _ in 0..most {
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

fn turret_change(
    cmd: &mut Commands,
    parent: Entity,
    art: &Insignia,
    (before, after): (usize, usize),
    most: usize,
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
    icon(cmd, row, art.turret.clone(), muted(), 16.);
    pips(cmd, row, &[(before, muted())], most);
    icon(cmd, row, art.arrow.clone(), muted(), 10.);
    let color = if after > before {
        Change::Gain.color()
    } else {
        Change::Cost.color()
    };
    pips(cmd, row, &[(after, color)], most);
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
    let flak = matches!(
        card,
        Card::MoreAntiAir | Card::HeavyAntiAir | Card::QuickAntiAir
    );
    Some(match label {
        "Reload" if flak => art.fitting(Card::QuickAntiAir),
        "Damage" if flak => art.fitting(Card::HeavyAntiAir),
        "Guns" | "Reach" if flak => art.fitting(Card::MoreAntiAir),
        "Traverse" => art.fitting(Card::TrainingGear),
        "Plate" => art.fitting(Card::Armor),
        // The largest round the plate turns aside.
        "Stops" => art.fitting(Card::Shells),
        "Engine" => art.fitting(Card::Turbines),
        "Pumping" => art.fitting(Card::Pumps),
        "Holds dry to" => art.hull.clone(),
        "Turret" => art.turret.clone(),
        "Range" => art.fitting(Card::SecondaryRange),
        "Reload" if secondary => art.fitting(Card::QuickSecondaries),
        "Reload" => art.fitting(Card::Loader),
        "Pierces" => art.fitting(Card::Caps),
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
            let Some(next) = run.beam_class().next() else {
                note(cmd, spec, art, "Heaviest hull built");
                return;
            };
            if let Some(why) = run.widen_refusal() {
                refusal(cmd, spec, art, why);
            }
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
        }
        DockJob::Calibre { at, from, to } => {
            let Some(station) = station_on(design, at) else {
                return;
            };
            if from == to {
                hint(cmd, spec, "Fitted");
                return;
            }
            let barrels = station.gunhouse.barrels;
            if !fits {
                let at_beam = |beam: ShipBeam, barrels| {
                    shipvivor_campaign::Fittings {
                        turrets: barrels,
                        ..current
                    }
                    .capacity_for(beam, station.paired())
                        >= to
                };
                let beam = ShipBeam::ALL.into_iter().find(|&b| at_beam(b, barrels));
                let why = match (beam, station.paired()) {
                    (Some(b), _) => format!("Needs {} hull", b.name().to_lowercase()),
                    _ if ShipBeam::ALL
                        .into_iter()
                        .any(|b| at_beam(b, TurretLayout::Single)) =>
                    {
                        "Needs fewer barrels".into()
                    }
                    (None, true) => "Too big for a wing gallery".into(),
                    (None, false) => "Too big for this cradle".into(),
                };
                note(cmd, spec, art, why);
            }
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Shells),
                (f32::from(from), f32::from(to)),
                "in",
                0,
                Some(true),
            );
            if let Some(after) = run.projected_design(job).and_then(|d| station_on(&d, at)) {
                let (now, then) = (current.for_station(&station), current.for_station(&after));
                delta(
                    cmd,
                    spec,
                    art,
                    art.fitting(Card::Loader),
                    (now.reload(), then.reload()),
                    "s",
                    1,
                    Some(false),
                );
                // A heavier gunhouse trains more slowly.
                delta(
                    cmd,
                    spec,
                    art,
                    art.fitting(Card::TrainingGear),
                    (now.traverse(), then.traverse()),
                    "deg/s",
                    0,
                    Some(true),
                );
            }
        }
        DockJob::Barrels { at, from, to } => {
            let Some(station) = station_on(design, at) else {
                return;
            };
            if !fits {
                note(cmd, spec, art, "Too heavy for this beam and bore");
            }
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::TripleTurrets),
                (from.count() as f32, to.count() as f32),
                "",
                0,
                Some(true),
            );
            // The heaviest bore the cradle carries with these barrels.
            let cradle = |barrels: TurretLayout| {
                f32::from(
                    shipvivor_campaign::Fittings {
                        turrets: barrels,
                        ..current
                    }
                    .capacity_for(run.beam_class(), station.paired()),
                )
            };
            delta(
                cmd,
                spec,
                art,
                art.fitting(Card::Shells),
                (cradle(from), cradle(to)),
                "in",
                0,
                Some(true),
            );
        }
        DockJob::Barbette { at, .. } => {
            let Some(before) = station_on(design, at) else {
                return;
            };
            if !fits {
                note(cmd, spec, art, "Nothing beside it to fire over");
                return;
            }
            let Some(after) = run.projected_design(job).and_then(|d| station_on(&d, at)) else {
                return;
            };
            let travel = |gun: &GunStation| 2. * (gun.half_arc - gun.inner_mask).to_degrees();
            delta(
                cmd,
                spec,
                art,
                art.raise.clone(),
                (before.lift(), after.lift()),
                "m",
                1,
                None,
            );
            arcs(
                cmd,
                spec,
                art,
                &[&after],
                Some(travel(&before)),
                travel(&after),
            );
        }
        DockJob::Move { from, .. } | DockJob::Replace { at: from, .. } => {
            // A move is always between adjacent frames, so the old and new station correspond.
            let to = match job {
                DockJob::Move { to, .. } if fits => to,
                _ => from,
            };
            module_details(cmd, spec, run, art, job, from, to);
        }
        DockJob::Insert { module, .. } => {
            if let Some(why) = run.refusal(job) {
                refusal(cmd, spec, art, why);
                return;
            }
            let Some(after) = run.projected_design(job) else {
                return;
            };
            stock_details(cmd, spec, art, module);
            let before_guns = design.gun_stations().len();
            let after_guns = after.gun_stations().len();
            if after_guns != before_guns {
                let most = run.beam_class().most_turrets();
                turret_change(cmd, spec, art, (before_guns, after_guns), most);
            }
            let (before, after) = (design.sheet(), after.sheet());
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
            // Short of a turret's points, the dock's own warning says the bay will sail empty.
            if module == DockModule::Reserve && !run.leaves_bay_empty(job) {
                hint(cmd, spec, "Then select the new bay for a turret");
            }
        }
        // Thicker plate waits for a broader beam.
        DockJob::Fitting(_) if !fits => match run.refusal(job) {
            Some(why) => refusal(cmd, spec, art, why),
            None => note(cmd, spec, art, "Fully fitted"),
        },
        DockJob::Tubes { at } => {
            let last = design.modules().len() - 1;
            note(
                cmd,
                spec,
                art,
                match at {
                    0 => "A spread of three, fired dead ahead",
                    _ if at == last => "A spread of three, fired dead astern",
                    _ => "A spread of three, trained to either beam",
                },
            );
            figure(
                cmd,
                spec,
                art,
                Mark::Icon(art.tubes.clone()),
                design.tube_mounts().len().to_string(),
                (design.tube_mounts().len() + 1).to_string(),
                Change::New.color(),
            );
        }
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
            for stat in cards::face(card, current, design).stats {
                let mark = stat_art(&stat.label, card, art)
                    .map_or_else(|| Mark::Word(stat.label.clone()), Mark::Icon);
                // A figure with nothing before it is new, unless it is the gain itself,
                // such as the share a loader takes off every reload.
                let color = if stat.before.is_empty() && !matches!(stat.change, Change::Gain) {
                    Change::New.color()
                } else {
                    stat.change.color()
                };
                figure(cmd, spec, art, mark, stat.before, stat.after, color);
            }
        }
    }
}

/// The first main turret standing on module `at`.
fn station_on(design: &shipvivor_shipyard::Design, at: usize) -> Option<GunStation> {
    design
        .gun_stations()
        .into_iter()
        .find(|g| g.berth.module == at)
}

/// What a section from the yard's stock brings aboard besides its length.
fn stock_details(cmd: &mut Commands, spec: Entity, art: &Insignia, module: DockModule) {
    use shipvivor_shipyard as yard;
    let new = |cmd: &mut Commands, image: Handle<Image>, words: String| {
        figure(
            cmd,
            spec,
            art,
            Mark::Icon(image),
            String::new(),
            words,
            Change::New.color(),
        );
    };
    match module {
        DockModule::Floatplane => {
            new(
                cmd,
                art.floatplane.clone(),
                format!("{} m", yard::FLOATPLANE_REACH),
            );
            new(
                cmd,
                art.fitting(Card::HeavySecondaries),
                format!("{} / {:.0} s", yard::BOMB_DAMAGE, yard::SORTIE_SECONDS),
            );
        }
        DockModule::Missiles => {
            new(
                cmd,
                art.missile.clone(),
                format!("{} m", yard::MISSILE_RANGE),
            );
            new(
                cmd,
                art.fitting(Card::HeavySecondaries),
                format!(
                    "{} x {} / {:.0} s",
                    yard::MISSILE_SALVO,
                    yard::MISSILE_DAMAGE,
                    yard::MISSILE_RELOAD
                ),
            );
        }
        DockModule::Turret | DockModule::EngineRoom | DockModule::Reserve => {}
    }
}

/// Why the yard turns a job down, as a padlock and the figure that would open it.
pub(super) fn refusal(cmd: &mut Commands, spec: Entity, art: &Insignia, why: Refusal) {
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
    icon(cmd, row, art.lock.clone(), brass(), 16.);
    let (image, words) = match why {
        Refusal::Full(1) => (None, "One at most".to_string()),
        Refusal::Full(most) => (None, format!("{most} at most")),
        Refusal::Beam(beam) => (
            Some(art.beam.clone()),
            format!("Needs {} hull", beam.name().to_lowercase()),
        ),
        Refusal::Length(metres) => (Some(art.length.clone()), format!("Needs {metres} m")),
        Refusal::Longest => (
            Some(art.length.clone()),
            format!("{:.0} m at most", shipvivor_campaign::MAX_LENGTH),
        ),
        Refusal::Widest => (None, "Heaviest hull built".to_string()),
        Refusal::Section(module) => (
            Some(super::catalog::stock_art(module, art)),
            "Needs another".to_string(),
        ),
    };
    if let Some(image) = image {
        icon(cmd, row, image, brass(), 14.);
    }
    text(cmd, row, words, 13., brass());
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
        let most = run.beam_class().most_turrets();
        turret_change(cmd, spec, art, (original.len(), proposed_guns.len()), most);
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
            1 => art.turret.clone(),
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
