//! A badge over each enemy drydock: the emblem of the hull on its slip, ringed by a
//! countdown that a sweep eats clockwise until she launches. While a dock lies wrecked
//! and its base's factories raise it again, the badge shows a spanner and counts down
//! the rebuild instead. A dock with nothing left to raise it shows nothing.
use crate::{
    camera::ShipView,
    mission_hud::charted,
    state::{Campaign, Encounter, PlayerHelm, Screen},
    threat_ring::{BadgeArt, Emblem},
};
use bevy::prelude::*;
use shipvivor_encounter::{Dock, Port};
use std::f32::consts::TAU;

/// The badge's side, and the width of its countdown ring.
pub const BADGE: f32 = 34.;
const RING: f32 = 4.;
/// Air between a badge and the top of its gantry, and between two badges of a base.
const GAP: f32 = 4.;
/// Badges kept: two slips at every base a sector can hold.
const POOL: usize = crate::mission_hud::MAX_BASES * 2;
/// Under this share of the count left, the ring turns coral.
const URGENT: f32 = 0.25;

#[derive(Component)]
pub struct SlipBadge {
    ring: Entity,
    emblem: Entity,
}

fn chalk() -> Color {
    Color::srgb_u8(239, 240, 220)
}
fn coral() -> Color {
    Color::srgb_u8(255, 111, 93)
}
fn brass() -> Color {
    Color::srgb_u8(255, 201, 101)
}
fn track() -> Color {
    Color::srgba_u8(151, 179, 179, 82)
}
/// Opaque, so the ring's colour never shows through the disc it frames.
fn plate() -> Color {
    Color::srgb_u8(12, 31, 40)
}

/// What a dock's badge shows.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Slip {
    /// A hull on the ways, with the share of her countdown still to run.
    Building(Emblem, f32),
    /// Funded and fitted out, waiting only for clear water off the slip.
    Ready(Emblem),
    /// Wrecked, with the share of the factories' rebuild still to run.
    Raising(f32),
}

fn slip(port: &Port, dock: &Dock) -> Option<Slip> {
    if let Some(rebuilt) = dock.rebuilding() {
        return (port.live_factories() > 0).then_some(Slip::Raising(1. - rebuilt));
    }
    let emblem = Emblem::of(dock.order().0);
    Some(if dock.ready() {
        Slip::Ready(emblem)
    } else {
        Slip::Building(emblem, 1. - dock.progress())
    })
}

/// How far the base's tag must rise to clear the badges on its slips.
pub fn lift(port: &Port) -> f32 {
    let shown = port.docks().iter().any(|dock| slip(port, dock).is_some());
    if shown { BADGE + GAP } else { 0. }
}

pub fn spawn(cmd: &mut Commands, root: Entity, spanner: Handle<Image>) {
    for _ in 0..POOL {
        let emblem = cmd
            .spawn((
                Node {
                    width: px(BADGE - 2. * RING),
                    height: px(BADGE - 2. * RING),
                    ..default()
                },
                ImageNode::new(spanner.clone()),
            ))
            .id();
        let disc = cmd
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(RING),
                    top: px(RING),
                    width: px(BADGE - 2. * RING),
                    height: px(BADGE - 2. * RING),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(plate()),
            ))
            .add_child(emblem)
            .id();
        let ring = cmd
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundGradient::default(),
            ))
            .id();
        cmd.spawn((
            SlipBadge { ring, emblem },
            Node {
                position_type: PositionType::Absolute,
                width: px(BADGE),
                height: px(BADGE),
                display: Display::None,
                ..default()
            },
            UiTransform::default(),
            ChildOf(root),
        ))
        .add_children(&[ring, disc]);
    }
}

/// The ring for a count with `left` of it still to run: the part already swept is the
/// dim track, and the rest is `color`, so the sweep eats it clockwise from the top.
fn ring(left: f32, color: Color) -> BackgroundGradient {
    let swept = (1. - left.clamp(0., 1.)) * TAU;
    BackgroundGradient::from(ConicGradient::new(
        UiPosition::CENTER,
        vec![
            AngularColorStop::new(track(), 0.),
            AngularColorStop::new(track(), swept),
            AngularColorStop::new(color, swept),
            AngularColorStop::new(color, TAU),
        ],
    ))
}

/// Where each badge goes, centred over the top of its gantry. Two badges of one base
/// that would overlap stand side by side over the pair.
fn place(
    battle: &Encounter,
    helm: &PlayerHelm,
    view: &ShipView,
    size: Vec2,
) -> Vec<(usize, usize, Slip, Vec2)> {
    let mut placed = Vec::new();
    for (i, port) in battle.ports().iter().enumerate() {
        if !charted(port) {
            continue;
        }
        let start = placed.len();
        for (j, dock) in port.docks().iter().enumerate() {
            let Some(show) = slip(port, dock) else {
                continue;
            };
            let top = port.dock_position(j).extend(dock.top() + 1.).xzy();
            let Some(p) = view.viewport_point(helm.position, top, size) else {
                continue;
            };
            let centre = Vec2::new(p.x, p.y - 0.5 * BADGE - GAP);
            if centre.x < -BADGE || centre.x > size.x + BADGE || centre.y > size.y {
                continue;
            }
            placed.push((i, j, show, centre));
        }
        if let [.., (_, _, _, a), (_, _, _, b)] = placed[start..] {
            let apart = BADGE + GAP;
            if a.distance(b) < apart {
                let mid = a.midpoint(b);
                let along = if a.x <= b.x { 0.5 } else { -0.5 };
                let n = placed.len();
                placed[n - 2].3 = mid - Vec2::X * along * apart;
                placed[n - 1].3 = mid + Vec2::X * along * apart;
            }
        }
    }
    // Keep a badge off the gun sight, as the base's tag is.
    let sight = size * 0.5;
    for (_, _, _, at) in &mut placed {
        if (at.x - sight.x).abs() < 0.5 * BADGE + 22.
            && at.y + 0.5 * BADGE > sight.y - 35.
            && at.y - 0.5 * BADGE < sight.y + 28.
        {
            at.y = sight.y - 35. - 0.5 * BADGE;
        }
    }
    placed
}

pub fn update(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    view: Res<ShipView>,
    viewport: (Single<&Window>, Res<UiScale>),
    art: (Option<Res<BadgeArt>>, Option<Res<crate::insignia::Insignia>>),
    mut badges: Query<(&SlipBadge, &mut Node, &mut UiTransform)>,
    mut rings: Query<&mut BackgroundGradient>,
    mut emblems: Query<&mut ImageNode>,
) {
    // A headless game has no art to dress them with.
    let (Some(art), Some(spanner)) = art else {
        return;
    };
    let mut pool = badges.iter_mut();
    let shown = if run.screen() == Screen::Sailing {
        let (window, scale) = viewport;
        place(&battle, &helm, &view, window.size() / scale.0)
    } else {
        Vec::new()
    };
    // A beat for whatever is ready to launch.
    let beat = 0.5 + 0.5 * (battle.at_sea() * 6.).sin();
    for &(_, _, show, centre) in &shown {
        let Some((badge, mut node, mut transform)) = pool.next() else {
            break;
        };
        node.display = Display::Flex;
        node.left = px(centre.x - 0.5 * BADGE);
        node.top = px(centre.y - 0.5 * BADGE);
        let (gradient, image, tint, grow) = match show {
            Slip::Building(emblem, left) => {
                let color = if left < URGENT { coral() } else { chalk() };
                (ring(left, color), art.emblem(emblem), chalk(), 1.)
            }
            Slip::Ready(emblem) => (
                ring(1., coral()),
                art.emblem(emblem),
                coral().mix(&chalk(), beat),
                1. + 0.12 * beat,
            ),
            Slip::Raising(left) => (ring(left, brass()), &spanner.order, brass(), 1.),
        };
        transform.scale = Vec2::splat(grow);
        if let Ok(mut fill) = rings.get_mut(badge.ring) {
            *fill = gradient;
        }
        if let Ok(mut icon) = emblems.get_mut(badge.emblem) {
            if icon.image != *image {
                icon.image = image.clone();
            }
            icon.color = tint;
        }
    }
    for (_, mut node, _) in pool {
        node.display = Display::None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Works, shell_works};
    use shipvivor_encounter::Battle;

    #[test]
    fn a_wrecked_dock_counts_down_its_rebuild_while_a_factory_stands() {
        let chart = crate::passage::Passage::of(shipvivor_campaign::Stage::OuterRoad).classic();
        let mut battle = Battle::for_chart(60., &chart);
        let index = (battle.ports().iter())
            .position(|p| p.docks().len() == 2)
            .expect("a base with two slips");
        let port = &battle.ports()[index];
        let fresh: Vec<_> = port.docks().iter().map(|d| slip(port, d)).collect();
        assert!(
            fresh
                .iter()
                .all(|s| matches!(s, Some(Slip::Building(Emblem::Patrol, _)))),
            "{fresh:?}"
        );
        shell_works(&mut battle, index, Works::Dock(0));
        let port = &battle.ports()[index];
        let Some(Slip::Raising(left)) = slip(port, &port.docks()[0]) else {
            panic!("{:?}", slip(port, &port.docks()[0]));
        };
        assert!(left > 0.9, "{left}");
        assert_eq!(lift(port), BADGE + GAP);
        // With the factories gone too, a dock that is down shows nothing.
        for j in 0..port.factories().len() {
            shell_works(&mut battle, index, Works::Factory(j));
        }
        let port = &battle.ports()[index];
        assert_eq!(slip(port, &port.docks()[0]), None);
        assert!(slip(port, &port.docks()[1]).is_some());
    }

    #[test]
    fn the_sweep_eats_the_ring_clockwise_from_the_top() {
        let BackgroundGradient(gradients) = ring(0.25, coral());
        let [Gradient::Conic(conic)] = gradients.as_slice() else {
            panic!("one conic gradient");
        };
        let angles: Vec<f32> = conic.stops.iter().map(|s| s.angle.unwrap()).collect();
        assert_eq!(angles, [0., 0.75 * TAU, 0.75 * TAU, TAU]);
        assert_eq!(conic.stops[3].color, coral());
    }
}
