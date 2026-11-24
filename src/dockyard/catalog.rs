//! The refits listed in a section's callout: pictogram tiles with a one-word caption,
//! one row per kind of work.
use super::*;

pub(super) fn required_beam(
    fit: shipvivor_campaign::Fittings,
    inches: u16,
    paired: bool,
) -> Option<ShipBeam> {
    ShipBeam::ALL
        .into_iter()
        .find(|&beam| fit.capacity_for(beam, paired) >= inches)
}

pub(super) fn slot_name(design: &Design, at: usize) -> &'static str {
    use shipvivor_shipyard::Component::*;
    let parts = &design.modules()[at].fit;
    if parts.contains(&Engine) {
        if parts.contains(&TorpedoRack) {
            "Engine"
        } else {
            "Aux engine"
        }
    } else if parts.contains(&Director) {
        "Bridge"
    } else {
        match parts.iter().filter(|&&part| part == MainGun).count() {
            0 => DeckModule::Reserve.name(),
            1 => DeckModule::CentreBattery.name(),
            _ => DeckModule::WingBattery.name(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    /// What stands on the bay, and where the bay sits.
    Bay,
    Bore,
    Guns,
    Secondaries,
    Machinery,
    Hull,
}
fn row_of(job: DockJob) -> Row {
    use Card::*;
    match job {
        DockJob::Replace { .. } | DockJob::Move { .. } => Row::Bay,
        DockJob::Calibre(_) => Row::Bore,
        DockJob::TwinTurrets
        | DockJob::Fitting(
            TripleTurrets | QuadTurrets | ArmorPiercing | Caps | Velocity | Loader | Shells,
        ) => Row::Guns,
        DockJob::Fitting(Turbines) => Row::Machinery,
        DockJob::Fitting(_) => Row::Secondaries,
        DockJob::Section(_) | DockJob::WidenBeam => Row::Hull,
    }
}
/// Bore, barrels and shells are the same in every turret.
pub(super) fn ship_wide(job: DockJob) -> bool {
    matches!(row_of(job), Row::Bore | Row::Guns)
}

/// The pictogram on a refit's tile.
fn tile_art(job: DockJob, run: &Campaign, art: &Insignia) -> Handle<Image> {
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
            DeckModule::Reserve => art.remove.clone(),
        },
        DockJob::Move { from, to } if to < from => art.fore.clone(),
        DockJob::Move { .. } => art.aft.clone(),
        DockJob::Section(DockModule::Reserve) => art.add_bay.clone(),
        DockJob::Section(DockModule::EngineRoom) => art.fitting(Card::Turbines),
        DockJob::WidenBeam => art.beam.clone(),
        DockJob::TwinTurrets => art.twin.clone(),
        DockJob::Fitting(card) => art.fitting(card),
        DockJob::Calibre(_) => art.fitting(Card::Shells),
    }
}

/// One word under a tile. The full name heads the preview once the tile is chosen.
fn caption(job: DockJob, run: &Campaign) -> &'static str {
    use Card::*;
    match job {
        DockJob::Replace { at, with } => match exchange_name(run, at, with) {
            "Add centre turret" => "Turret",
            "Add wing turrets" => "Wings",
            "Split into wing turrets" => "Split",
            "Merge into centre turret" => "Merge",
            _ => "Remove",
        },
        DockJob::Move { from, to } if to < from => "Fwd",
        DockJob::Move { .. } => "Aft",
        DockJob::Section(DockModule::Reserve) => "Bay",
        DockJob::Section(DockModule::EngineRoom) => "Engine",
        DockJob::WidenBeam => "Widen",
        DockJob::TwinTurrets => "Twin",
        DockJob::Calibre(_) => "",
        DockJob::Fitting(card) => match card {
            TripleTurrets => "Triple",
            QuadTurrets => "Quad",
            ArmorPiercing => "AP",
            Caps => "Caps",
            Velocity => "Velocity",
            Loader | QuickSecondaries => "Reload",
            MoreSecondaries => "More",
            TwinSecondaries => "Twin",
            SecondaryRange => "Range",
            HeavySecondaries => "Heavy",
            Turbines => "Turbines",
            Shells => "Bore",
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
    let mounted = run.fittings.round().calibre_inches() as u16;
    let mut rows: Vec<(Row, Entity)> = Vec::new();
    let mut tagged = false;
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
        let selected = run.dock_option == Some(i);
        let tint = if selected {
            brass()
        } else if job == DockJob::Calibre(mounted) {
            cyan()
        } else if job.fits(run) {
            paper()
        } else {
            rule()
        };
        if let DockJob::Calibre(inches) = job {
            let chip = button(
                cmd,
                row,
                Some(Action::Preview(i)),
                selected,
                false,
                Node {
                    width: px(46),
                    height: px(46),
                    border: UiRect::all(px(1)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
            );
            text(cmd, chip, format!("{inches}\""), 16., tint);
            continue;
        }
        let tile = button(
            cmd,
            row,
            Some(Action::Preview(i)),
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
    }
}
