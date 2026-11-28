//! The refits listed in a section's callout: pictogram tiles with a one-word caption,
//! one row per kind of work. A click orders one at once, booking its price from the
//! points, and work the yard cannot make or the points cannot cover takes no click.
use super::*;
use shipvivor_encounter::TurretLayout;

pub(super) fn slot_name(design: &Design, at: usize) -> &'static str {
    use shipvivor_shipyard::Component::*;
    let parts = &design.modules()[at].fit;
    if parts.contains(&Engine) {
        if design.machinery() == Some(at) {
            "Engine"
        } else {
            "Aux engine"
        }
    } else if parts.contains(&Director) {
        "Bridge"
    } else if parts.contains(&Catapult) {
        "Floatplane"
    } else if parts.contains(&MissileRack) {
        DockModule::Missiles.name()
    } else if design.modules()[at].frame == shipvivor_shipyard::Frame::DoubleBattery {
        DeckModule::DoubleBattery.name()
    } else {
        match parts.iter().filter(|&&part| part == MainGun).count() {
            0 => DeckModule::Reserve.name(),
            1 => DeckModule::CentreBattery.name(),
            _ => DeckModule::WingBattery.name(),
        }
    }
}

/// The pictogram for a section in the yard's stock.
pub(super) fn stock_art(module: DockModule, art: &Insignia) -> Handle<Image> {
    match module {
        DockModule::Turret => art.centre_turret.clone(),
        DockModule::EngineRoom => art.fitting(Card::Turbines),
        DockModule::Floatplane => art.floatplane.clone(),
        DockModule::Missiles => art.missile.clone(),
        DockModule::Reserve => art.add_bay.clone(),
    }
}

/// One word under a stock tile.
pub(super) fn stock_caption(module: DockModule) -> &'static str {
    match module {
        DockModule::Turret => "Turret",
        DockModule::EngineRoom => "Engine",
        DockModule::Floatplane => "Plane",
        DockModule::Missiles => "Missiles",
        DockModule::Reserve => "Bay",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    /// What stands on the bay, and where the bay sits.
    Bay,
    /// The bore of this bay's turrets.
    Bore,
    /// This bay's turrets: their barrels and how high they stand.
    Turret,
    /// Shells and gear, the same in every turret.
    Guns,
    Secondaries,
    Machinery,
    Hull,
}
fn row_of(job: DockJob) -> Row {
    use Card::*;
    match job {
        DockJob::Replace { .. } | DockJob::Move { .. } | DockJob::Tubes { .. } => Row::Bay,
        DockJob::Calibre { .. } => Row::Bore,
        DockJob::Barrels { .. } | DockJob::Barbette { .. } => Row::Turret,
        DockJob::Fitting(
            TripleTurrets | QuadTurrets | Caps | Velocity | Loader | Shells,
        ) => Row::Guns,
        DockJob::Fitting(TrainingGear) => Row::Guns,
        DockJob::Fitting(Turbines | Hardening | Pumps) => Row::Machinery,
        DockJob::Fitting(Armor) => Row::Hull,
        DockJob::Fitting(_) => Row::Secondaries,
        DockJob::Insert { .. } | DockJob::WidenBeam => Row::Hull,
    }
}
/// Shells and gear are the same in every turret. Bore and barrels are each turret's own.
pub(super) fn ship_wide(job: DockJob) -> bool {
    matches!(row_of(job), Row::Guns)
}

/// The pictogram for a turret with this many barrels.
fn barrels_art(barrels: TurretLayout, art: &Insignia) -> Handle<Image> {
    match barrels {
        TurretLayout::Single => art.turret.clone(),
        TurretLayout::Twin => art.twin.clone(),
        TurretLayout::Triple => art.fitting(Card::TripleTurrets),
        TurretLayout::Quad => art.fitting(Card::QuadTurrets),
    }
}

/// The pictogram on a refit's tile.
pub(super) fn tile_art(job: DockJob, run: &Campaign, art: &Insignia) -> Handle<Image> {
    let empty = |at: usize| {
        run.ship
            .design()
            .modules()
            .get(at)
            .is_some_and(|m| m.fit.is_empty())
    };
    match job {
        DockJob::Replace { at, with } => match with {
            DeckModule::CentreBattery if empty(at) => art.centre_turret.clone(),
            DeckModule::CentreBattery => art.turret.clone(),
            DeckModule::WingBattery => art.wing_turrets.clone(),
            DeckModule::DoubleBattery => art.double_turrets.clone(),
            DeckModule::Reserve => art.remove.clone(),
        },
        DockJob::Move { from, to } if to < from => art.fore.clone(),
        DockJob::Move { .. } => art.aft.clone(),
        DockJob::Insert { module, .. } => stock_art(module, art),
        DockJob::WidenBeam => art.beam.clone(),
        DockJob::Barrels { to, .. } => barrels_art(to, art),
        DockJob::Barbette { from, to, .. } if to > from => art.raise.clone(),
        DockJob::Barbette { .. } => art.turret.clone(),
        DockJob::Fitting(card) => art.fitting(card),
        DockJob::Calibre { .. } => art.fitting(Card::Shells),
        DockJob::Tubes { .. } => art.tubes.clone(),
    }
}

/// A word or two for an order on the yard's list. Unlike a tile's caption it describes
/// the order itself, not the bay as it stands after it.
pub(super) fn order_caption(job: DockJob, run: &Campaign) -> String {
    match job {
        DockJob::Calibre { to, .. } => format!("{to}\""),
        DockJob::Replace { with, .. } => match with {
            DeckModule::CentreBattery => "Turret",
            DeckModule::WingBattery => "Wings",
            DeckModule::DoubleBattery => "Double",
            DeckModule::Reserve => "Remove",
        }
        .into(),
        DockJob::Barrels { to, .. } => match to {
            TurretLayout::Single => "Single",
            TurretLayout::Twin => "Twin",
            TurretLayout::Triple => "Triple",
            TurretLayout::Quad => "Quad",
        }
        .into(),
        _ => caption(job, run).into(),
    }
}

/// One word under a tile. The full name heads the preview once the tile is chosen.
pub(super) fn caption(job: DockJob, run: &Campaign) -> &'static str {
    use Card::*;
    match job {
        DockJob::Replace { at, with } => match exchange_name(run, at, with) {
            "Add centre turret" => "Turret",
            "Add wing turrets" | "Wing turrets" => "Wings",
            "Split into wing turrets" => "Split",
            "Merge into centre turret" => "Merge",
            _ if with == DeckModule::DoubleBattery => "Double",
            _ => "Remove",
        },
        DockJob::Move { from, to } if to < from => "Fwd",
        DockJob::Move { .. } => "Aft",
        DockJob::Insert { module, .. } => stock_caption(module),
        DockJob::WidenBeam => match run.beam_class().next() {
            Some(next) => next.short_name(),
            None => "Heaviest",
        },
        DockJob::Barrels { from, to, .. } if to < from => "Fewer",
        DockJob::Barrels { to, .. } => match to {
            TurretLayout::Twin => "Twin",
            TurretLayout::Triple => "Triple",
            _ => "Quad",
        },
        DockJob::Barbette { from, to, .. } if to > from => "Raise",
        DockJob::Barbette { .. } => "Lower",
        DockJob::Calibre { .. } => "",
        DockJob::Tubes { .. } => "Tubes",
        DockJob::Fitting(card) => match card {
            TripleTurrets => "Triple",
            QuadTurrets => "Quad",
            Caps => "Caps",
            Velocity => "Velocity",
            Loader | QuickSecondaries => "Reload",
            MoreSecondaries => "More",
            // The mounting the next barrel makes, or the last once every one is fitted.
            TwinSecondaries => match run.fittings.secondaries().barrels {
                ..=1 => "Twin",
                2 => "Triple",
                _ => "Quad",
            },
            SecondaryRange => "Range",
            HeavySecondaries => "Heavy",
            Turbines => "Turbines",
            Shells => "Bore",
            TrainingGear => "Train",
            Armor => "Armor",
            Hardening => "Harden",
            MoreAntiAir => "More AA",
            HeavyAntiAir => "Flak",
            QuickAntiAir => "Reload",
            Pumps => "Pumps",
            MoreLaunchers => "Launch",
            MoreMissiles => "Salvo",
            MorePlanes => "Planes",
            MoreBombs => "Bombs",
            // The size of gunhouse the next rank builds, or the last once it is fitted.
            SecondaryTurrets => match run.fittings.next_secondary_bore() {
                Some(..=4) => "4-inch",
                _ => "6-inch",
            },
        },
    }
}

pub(super) fn options(
    cmd: &mut Commands,
    parent: Entity,
    run: &Campaign,
    art: &Insignia,
    jobs: &[DockJob],
) {
    let mut rows: Vec<(Row, Entity)> = Vec::new();
    let mut tagged = false;
    let beckoned = super::need(run);
    for (i, &job) in jobs.iter().enumerate() {
        let kind = row_of(job);
        let row = match rows.iter().find(|(r, _)| *r == kind) {
            Some(&(_, row)) => {
                // Moves follow the bay's exchanges after a little space.
                if matches!(job, DockJob::Move { .. })
                    && !matches!(jobs.get(i.wrapping_sub(1)), Some(DockJob::Move { .. }))
                {
                    node(
                        cmd,
                        row,
                        Node {
                            width: px(10),
                            ..default()
                        },
                        Color::NONE,
                    );
                }
                row
            }
            None => {
                if ship_wide(job) && !tagged {
                    tagged = true;
                    text(cmd, parent, "ALL TURRETS", 10., muted());
                }
                let row = node(
                    cmd,
                    parent,
                    Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(4),
                        row_gap: px(4),
                        ..default()
                    },
                    Color::NONE,
                );
                rows.push((kind, row));
                row
            }
        };
        // Picked out with the arrow keys.
        let selected = run.dock_option == Some(i);
        let quote = run.quote(job);
        // Cyan is what the ship carries now: the bore mounted, or a fitting at its last rank.
        let tint = if selected {
            brass()
        } else if matches!(job, DockJob::Calibre { from, to, .. } if from == to)
            || fully_fitted(job, run)
        {
            cyan()
        } else if quote.is_some() {
            paper()
        } else {
            rule()
        };
        // A click orders the refit at once. One the yard cannot make takes no click.
        let action = quote.map(|_| Action::Order(i));
        if let DockJob::Calibre { to: inches, .. } = job {
            let chip = button(
                cmd,
                row,
                action,
                selected,
                false,
                // The bore sits low, clear of the price in the top corner.
                Node {
                    width: px(46),
                    height: px(46),
                    border: UiRect::all(px(1)),
                    padding: UiRect::bottom(px(6)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexEnd,
                    ..default()
                },
            );
            text(cmd, chip, format!("{inches}\""), 16., tint);
            price(cmd, chip, run, art, job, quote);
            if beckoned == Some(Need::Job(job)) {
                cmd.entity(chip).insert(Beckon);
            }
            continue;
        }
        let tile = button(
            cmd,
            row,
            action,
            selected,
            false,
            Node {
                width: px(56),
                height: px(46),
                border: UiRect::all(px(1)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(3),
                ..default()
            },
        );
        icon(cmd, tile, tile_art(job, run, art), tint, 22.);
        text(cmd, tile, caption(job, run), 10., tint);
        price(cmd, tile, run, art, job, quote);
        if beckoned == Some(Need::Job(job)) {
            cmd.entity(tile).insert(Beckon);
        }
        if let DockJob::Fitting(card) = job {
            rank_pips(cmd, tile, run, card, selected);
        }
    }
}

/// A tile's price, as the hull and stock tiles show theirs: in brass, what a click books
/// from the points, and in red, work the ship could take but the points left cannot
/// cover. Work that books nothing, or gives points back, shows none.
fn price(
    cmd: &mut Commands,
    tile: Entity,
    run: &Campaign,
    art: &Insignia,
    job: DockJob,
    quote: Option<i32>,
) {
    match quote {
        Some(spent) => {
            if let Ok(spent @ 1..) = u16::try_from(spent) {
                price_tag(cmd, tile, art, spent, brass());
            }
        }
        None if job.fits(run) && run.points_after(job).is_none() => {
            price_tag(cmd, tile, art, job.cost(), cards::Change::Cost.color());
        }
        None => {}
    }
}

/// A fitting fitted to its last rank: done, rather than refused.
/// A deck full of secondaries that a longer hull would make room on is locked, not done.
pub(super) fn fully_fitted(job: DockJob, run: &Campaign) -> bool {
    match job {
        DockJob::Fitting(card) => {
            run.ranks(card).is_some_and(|(fitted, most)| fitted >= most)
                && run.refusal(job).is_none()
        }
        _ => false,
    }
}

/// More ranks than this are shown on a meter, as pips would run into the price.
const MOST_PIPS: u16 = 6;

/// The ranks of a stacking fitting along the top left of its tile, clear of the price:
/// cyan for each one fitted, brass for the one being previewed. A fitting with more
/// ranks than pips fit, such as the secondaries a long hull has room for, shows them
/// as a segmented meter along the foot of the tile instead.
fn rank_pips(cmd: &mut Commands, tile: Entity, run: &Campaign, card: Card, previewed: bool) {
    let Some((fitted, most)) = run.ranks(card) else {
        return;
    };
    let ink = |rank: u16| {
        if rank < fitted {
            cyan()
        } else if previewed && rank == fitted {
            brass()
        } else {
            rule()
        }
    };
    if most > MOST_PIPS {
        let meter = node(
            cmd,
            tile,
            Node {
                position_type: PositionType::Absolute,
                left: px(4),
                right: px(4),
                bottom: px(2),
                height: px(2),
                column_gap: px(1),
                ..default()
            },
            Color::NONE,
        );
        for rank in 0..most {
            node(
                cmd,
                meter,
                Node {
                    flex_grow: 1.,
                    ..default()
                },
                ink(rank),
            );
        }
        return;
    }
    let row = node(
        cmd,
        tile,
        Node {
            position_type: PositionType::Absolute,
            top: px(3),
            left: px(3),
            column_gap: px(2),
            ..default()
        },
        Color::NONE,
    );
    for rank in 0..most {
        let color = if rank < fitted {
            Some(cyan())
        } else if previewed && rank == fitted {
            Some(brass())
        } else {
            None
        };
        cmd.spawn((
            Node {
                width: px(4),
                height: px(4),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(color.unwrap_or(Color::NONE)),
            BorderColor::all(color.unwrap_or(rule())),
            ChildOf(row),
        ));
    }
}
