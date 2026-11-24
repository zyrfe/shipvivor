//! A ring on the water around the flagship. A pip on it marks the bearing of each armed
//! enemy in sight with a plan-view emblem of what it is, turned to its heading, and an
//! arrow points in from the pip while that enemy has rounds in the air. The chase camera
//! never sees the arc straight behind it, so pips there are pinned to the edge of the
//! view on the line from the ship. The same emblems ride over every enemy ship and
//! harbour gun in view. Each harbour gun is a threat of its own.
use crate::{
    camera::ShipView,
    seascape::Sea,
    state::{Campaign, Encounter, PlayerHelm, Screen},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use icons::Emblem;
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{Battery, Battle, Hull, SIGHT_LIMIT};

mod icons;

const SEGMENTS: u32 = 128;
/// Width of the painted band.
const BAND: f32 = 0.5;
/// Air between the ring and the highest crest the swell can raise.
const CLEARANCE: f32 = 0.3;
/// Clear water between the ring and the hull's ends.
const STANDOFF: f32 = 5.;
/// A threat keeps its shell this long after its last round lands.
const FIRING_HOLD: f32 = 2.5;
/// A round whose muzzle is further than this from every known gun gets a pip of its own.
const ATTRIBUTION: f32 = 120.;
const POOL: usize = 20;
/// Badges over ships and guns, and the gap kept between one and the targeting triangle's place.
const TAGS: usize = 18;
const TAG_GAP: f32 = 3.;
/// Sides of a ring pip, a badge over a ship, and the arrow a firing pip points with.
const PIP: f32 = 30.;
const TAG: f32 = 24.;
const ARROW: f32 = 14.;
/// HUD margin kept clear around pinned pips.
const INSET: Vec2 = Vec2::new(24., 30.);

#[derive(Component)]
pub struct Ring;
#[derive(Component)]
pub struct PipLayer;
/// A disc with an emblem in it, on the ring or over a ship.
#[derive(Component)]
pub struct Badge {
    emblem: Entity,
    arrow: Entity,
}
#[derive(Component)]
pub struct OnRing;
#[derive(Component)]
pub struct OverShip;
#[derive(Component)]
pub struct BadgeEmblem;
#[derive(Component)]
pub struct BadgeArrow;

#[derive(Resource)]
pub struct BadgeArt {
    disc: Handle<Image>,
    gold_disc: Handle<Image>,
    arrow: Handle<Image>,
    emblems: Vec<(Emblem, Handle<Image>)>,
}
impl BadgeArt {
    fn emblem(&self, kind: Emblem) -> &Handle<Image> {
        &self.emblems.iter().find(|(k, _)| *k == kind).unwrap().1
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Source {
    Ship(u64),
    Aircraft(u64),
    /// A harbour, and which of her guns.
    Battery(usize, usize),
    /// Fire from a gun this module does not know of, marked where it was seen.
    Stray(Vec2),
}

#[derive(Clone, Copy, Debug)]
struct Threat {
    at: Vec2,
    emblem: Emblem,
    heading: Option<f32>,
    /// Seconds since a round from this source was last seen in the air.
    quiet: Option<f32>,
}

/// When each source last had a round in the air, in encounter seconds.
#[derive(Default)]
pub struct FireLog(Vec<(Source, f32)>);

fn ring_radius(run: &Campaign) -> f32 {
    Hull::CRUISER
        .bow
        .max(Hull::CRUISER.stern + run.added_length())
        + STANDOFF
}

/// The ring lies level and still, just clear of the tallest crest the swell can stack
/// up, so no wave ever cuts it and it never moves with the water.
fn ring_height(swell: &shipvivor_sea::Swell) -> f32 {
    swell
        .trains()
        .iter()
        .map(|train| train.amplitude)
        .sum::<f32>()
        + CLEARANCE
}

pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut ring = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let n = SEGMENTS;
    ring.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0_f32; 3]; 2 * n as usize]);
    ring.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 2 * n as usize]);
    ring.insert_indices(Indices::U32(
        (0..n)
            .flat_map(|i| {
                let j = (i + 1) % n;
                [2 * i, 2 * i + 1, 2 * j + 1, 2 * i, 2 * j + 1, 2 * j]
            })
            .collect(),
    ));
    cmd.spawn((
        Ring,
        Mesh3d(meshes.add(ring)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.91, 0.97, 0.98, 0.55),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
        // A lengthened hull widens the ring, which leaves the bounds stored at spawn too small.
        NoFrustumCulling,
    ));
    let art = BadgeArt {
        disc: images.add(icons::disc(false)),
        gold_disc: images.add(icons::disc(true)),
        arrow: images.add(icons::arrow()),
        emblems: Emblem::ALL
            .into_iter()
            .map(|kind| (kind, images.add(icons::emblem(kind))))
            .collect(),
    };
    let layer = cmd
        .spawn((
            PipLayer,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                display: Display::None,
                ..default()
            },
            // The ring's beam and astern pips fall under the corner panels; a threat
            // matters more than the few letters a pip covers.
            GlobalZIndex(1),
        ))
        .id();
    for ring in std::iter::repeat_n(true, POOL).chain(std::iter::repeat_n(false, TAGS)) {
        let emblem = cmd
            .spawn((
                BadgeEmblem,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ImageNode::new(art.emblem(Emblem::Unknown).clone()),
                UiTransform::default(),
            ))
            .id();
        let arrow = cmd
            .spawn((
                BadgeArrow,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(ARROW),
                    height: px(ARROW),
                    display: Display::None,
                    ..default()
                },
                ImageNode::new(art.arrow.clone()),
                UiTransform::default(),
            ))
            .id();
        let mut badge = cmd.spawn((
            Badge { emblem, arrow },
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            ImageNode::new(art.disc.clone()),
            UiTransform::default(),
            ZIndex(0),
            ChildOf(layer),
        ));
        badge.add_children(&[emblem, arrow]);
        if ring {
            badge.insert(OnRing);
        } else {
            badge.insert(OverShip);
        }
    }
    cmd.insert_resource(art);
}

/// The gun nearest a round's muzzle fired it. Ships move a little while their rounds fly.
fn attribute(guns: &[(Source, Vec2)], origin: Vec2) -> Source {
    guns.iter()
        .map(|&(source, at)| (source, at.distance_squared(origin)))
        .filter(|&(_, d)| d <= ATTRIBUTION * ATTRIBUTION)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Source::Stray(origin), |(source, _)| source)
}

/// Every armed enemy in sight, and every source with rounds recently in the air.
fn threats(battle: &Battle, player: Vec2, log: &mut FireLog) -> Vec<Threat> {
    let now = battle.at_sea();
    // A new encounter restarts the clock.
    log.0
        .retain(|&(_, at)| at <= now && now - at <= FIRING_HOLD);
    let mut guns = Vec::new();
    for r in battle.raiders() {
        guns.push((Source::Ship(r.body.id), r.body.pos));
    }
    for (index, port) in battle.ports().iter().enumerate() {
        for (gun, turret) in port.turrets().iter().enumerate() {
            guns.push((Source::Battery(index, gun), turret.position(&port.body)));
        }
    }
    let aircraft = battle.air_station().and_then(|base| base.aircraft.as_ref());
    for shot in battle.shots() {
        if !matches!(
            shot.battery,
            Battery::Raider | Battery::EnemySecondary | Battery::Shore | Battery::Aircraft
        ) {
            continue;
        }
        let source = if shot.battery == Battery::Aircraft {
            aircraft.map_or(Source::Stray(shot.origin()), |plane| {
                Source::Aircraft(plane.body.id)
            })
        } else {
            attribute(&guns, shot.origin())
        };
        let same = |s: &Source| match (s, source) {
            (Source::Stray(a), Source::Stray(b)) => a.distance(b) <= ATTRIBUTION,
            (a, b) => *a == b,
        };
        match log.0.iter_mut().find(|entry| same(&entry.0)) {
            Some(entry) => *entry = (source, now),
            None => log.0.push((source, now)),
        }
    }
    let quiet = |source: Source| {
        log.0
            .iter()
            .find(|(s, _)| *s == source)
            .map(|&(_, at)| now - at)
    };
    let in_sight = |at: Vec2| at.distance_squared(player) <= SIGHT_LIMIT * SIGHT_LIMIT;
    let mut out = Vec::new();
    for r in battle.raiders() {
        let source = Source::Ship(r.body.id);
        let armed = r.afloat() && r.vessel().powered() && r.live_guns() > 0;
        let quiet = quiet(source);
        if (armed && in_sight(r.body.pos)) || quiet.is_some() {
            out.push(Threat {
                at: r.body.pos,
                emblem: Emblem::of(r.class),
                heading: Some(r.body.heading),
                quiet,
            });
        }
    }
    for (index, port) in battle.ports().iter().enumerate() {
        for (gun, turret) in port.turrets().iter().enumerate() {
            let quiet = quiet(Source::Battery(index, gun));
            let at = turret.position(&port.body);
            if (turret.live() && in_sight(at)) || quiet.is_some() {
                out.push(Threat {
                    at,
                    emblem: Emblem::Fort,
                    heading: None,
                    quiet,
                });
            }
        }
    }
    if let Some(plane) = aircraft.filter(|plane| plane.inbound() && in_sight(plane.body.pos)) {
        out.push(Threat {
            at: plane.body.pos,
            emblem: Emblem::Plane,
            heading: Some(plane.body.heading),
            quiet: quiet(Source::Aircraft(plane.body.id)),
        });
    }
    for &(source, at) in &log.0 {
        if let Source::Stray(origin) = source {
            out.push(Threat {
                at: origin,
                emblem: Emblem::Unknown,
                heading: None,
                quiet: Some(now - at),
            });
        }
    }
    out
}

/// Where a pip for a bearing sits on the HUD: on the ring where the ring is in view,
/// otherwise at the edge of the view on the line from the ship's centre.
fn place(view: &ShipView, ship: Vec2, radius: f32, bearing: f32, height: f32, size: Vec2) -> Vec2 {
    let lo = INSET;
    let hi = (size - INSET).max(lo);
    let point = (ship + direction(bearing) * radius).extend(height).xzy();
    let projected = view.viewport_point(ship, point, size);
    if let Some(p) = projected
        && p.cmpge(lo).all()
        && p.cmple(hi).all()
    {
        return p;
    }
    let centre = view
        .viewport_point(ship, ship.extend(height).xzy(), size)
        .unwrap_or(Vec2::new(size.x * 0.5, hi.y))
        .clamp(lo, hi);
    // Behind the lens: dead ahead is up the screen and dead astern is down it.
    let way = match projected {
        Some(p) if p != centre => p - centre,
        _ => {
            let relative = bearing - view.bearing;
            Vec2::new(relative.sin(), -relative.cos())
        }
    };
    let reach = |d: f32, from: f32, lo: f32, hi: f32| {
        if d > 0. {
            (hi - from) / d
        } else if d < 0. {
            (lo - from) / d
        } else {
            f32::INFINITY
        }
    };
    let t = reach(way.x, centre.x, lo.x, hi.x).min(reach(way.y, centre.y, lo.y, hi.y));
    centre + way * t.max(0.)
}

/// How one badge shows this frame.
struct Look {
    centre: Vec2,
    side: f32,
    emblem: Emblem,
    /// The emblem's turn on screen, clockwise. Bow up is the way the camera looks.
    turn: Rot2,
    alpha: f32,
    /// Extra scale while a fresh round is in the air.
    pop: f32,
    /// The way to the flagship on screen, while this enemy is firing.
    inward: Option<Vec2>,
}
impl Look {
    fn turn(view: &ShipView, emblem: Emblem, heading: Option<f32>) -> Rot2 {
        match heading {
            Some(heading) if emblem.turns() => Rot2::radians(heading - view.bearing),
            _ => Rot2::IDENTITY,
        }
    }
    /// Distant enemies fade, so the near ones stand out.
    fn alpha(range: f32) -> f32 {
        1. - 0.45 * ((range - 400.) / (SIGHT_LIMIT - 400.)).clamp(0., 1.)
    }
}

type Badges<'w, 's, F> = Query<
    'w,
    's,
    (
        &'static Badge,
        &'static mut Node,
        &'static mut ImageNode,
        &'static mut UiTransform,
        &'static mut ZIndex,
    ),
    F,
>;
type Emblems<'w, 's> = Query<
    'w,
    's,
    (&'static mut ImageNode, &'static mut UiTransform),
    (With<BadgeEmblem>, Without<Badge>),
>;
type Arrows<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static mut UiTransform),
    (With<BadgeArrow>, Without<Badge>, Without<BadgeEmblem>),
>;

type LayerOnly = (With<PipLayer>, Without<Badge>, Without<BadgeArrow>);
type RingOnly = (With<OnRing>, Without<OverShip>);
type TagsOnly = (With<OverShip>, Without<OnRing>);

/// Show each badge in a pool with the next look, and hide the ones left over.
fn dress<'a, F: bevy::ecs::query::QueryFilter>(
    badges: &mut Badges<F>,
    looks: impl IntoIterator<Item = &'a Look>,
    art: &BadgeArt,
    emblems: &mut Emblems,
    arrows: &mut Arrows,
) {
    let mut looks = looks.into_iter();
    for (badge, mut node, mut disc, mut transform, mut z) in badges.iter_mut() {
        let Some(look) = looks.next() else {
            node.display = Display::None;
            continue;
        };
        let gold = look.emblem.gold();
        let face = if gold { &art.gold_disc } else { &art.disc };
        if disc.image != *face {
            disc.image = face.clone();
        }
        disc.color = Color::WHITE.with_alpha(look.alpha);
        transform.scale = Vec2::splat(1. + look.pop);
        z.0 = i32::from(look.inward.is_some());
        node.width = px(look.side);
        node.height = px(look.side);
        node.left = px(look.centre.x - look.side * 0.5);
        node.top = px(look.centre.y - look.side * 0.5);
        node.display = Display::Flex;
        if let Ok((mut emblem, mut turn)) = emblems.get_mut(badge.emblem) {
            let picture = art.emblem(look.emblem);
            if emblem.image != *picture {
                emblem.image = picture.clone();
            }
            let tint = if gold {
                Color::srgb_u8(255, 200, 94)
            } else {
                Color::srgb_u8(246, 248, 229)
            };
            emblem.color = tint.with_alpha(look.alpha);
            turn.rotation = look.turn;
        }
        if let Ok((mut arrow, mut turn)) = arrows.get_mut(badge.arrow) {
            match look.inward {
                Some(way) => {
                    // Tucked against the rim, tip toward the flagship.
                    let at = Vec2::splat(look.side * 0.5) + way * ((look.side + ARROW) * 0.5 - 2.);
                    arrow.left = px(at.x - ARROW * 0.5);
                    arrow.top = px(at.y - ARROW * 0.5);
                    arrow.display = Display::Flex;
                    turn.rotation = Rot2::radians(way.x.atan2(-way.y));
                }
                None => arrow.display = Display::None,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    navigation: (
        Res<Campaign>,
        Res<PlayerHelm>,
        Res<Encounter>,
        Res<ShipView>,
    ),
    sea: Res<Sea>,
    window: Single<&Window>,
    scale: Res<UiScale>,
    art: Res<BadgeArt>,
    mut meshes: ResMut<Assets<Mesh>>,
    ring: Single<(&Mesh3d, &mut Transform, &mut Visibility), With<Ring>>,
    mut layer: Single<&mut Node, LayerOnly>,
    mut parts: (Badges<RingOnly>, Emblems, Arrows),
    mut log: Local<FireLog>,
    mut built: Local<f32>,
) {
    let (run, helm, battle, view) = navigation;
    let (pips, emblems, arrows) = &mut parts;
    let (mesh, mut pose, mut shown) = ring.into_inner();
    let sailing = run.screen() == Screen::Sailing;
    shown.set_if_neq(if sailing {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });
    let display = if sailing {
        Display::Flex
    } else {
        Display::None
    };
    if layer.display != display {
        layer.display = display;
    }
    if !sailing {
        log.0.clear();
        return;
    }
    let ship = helm.position;
    let radius = ring_radius(&run);
    let height = ring_height(&sea.swell);
    *pose = Transform::from_xyz(ship.x, height, ship.y);
    if *built != radius
        && let Some(mut mesh) = meshes.get_mut(&mesh.0)
    {
        let positions: Vec<[f32; 3]> = (0..SEGMENTS)
            .flat_map(|i| {
                let way = direction(i as f32 / SEGMENTS as f32 * std::f32::consts::TAU);
                [radius - BAND * 0.5, radius + BAND * 0.5].map(|r| [way.x * r, 0., way.y * r])
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        *built = radius;
    }
    let size = window.size() / scale.0;
    let hub = view
        .viewport_point(ship, ship.extend(height).xzy(), size)
        .unwrap_or(Vec2::new(size.x * 0.5, size.y));
    let mut threats = threats(&battle, ship, &mut log);
    // Firing pips draw over quiet ones.
    threats.sort_by_key(|t| t.quiet.is_some());
    let looks: Vec<Look> = threats
        .iter()
        .map(|threat| {
            let centre = place(&view, ship, radius, heading(threat.at - ship), height, size);
            Look {
                centre,
                side: PIP,
                emblem: threat.emblem,
                turn: Look::turn(&view, threat.emblem, threat.heading),
                alpha: if threat.quiet.is_some() {
                    1.
                } else {
                    Look::alpha(threat.at.distance(ship))
                },
                pop: threat
                    .quiet
                    .map_or(0., |quiet| (1. - quiet / 0.35).max(0.) * 0.3),
                inward: threat
                    .quiet
                    .map(|_| (hub - centre).normalize_or(Vec2::NEG_Y)),
            }
        })
        .collect();
    dress(pips, &looks, &art, emblems, arrows);
}

/// An emblem over each enemy ship in view, in the place above her mast where the
/// targeting triangle sits, and just above the triangle when she is the target. The
/// flagship has her own tag, and ships out of view are left to the ring. Each live
/// harbour gun in view carries one over her roof the same way.
/// A tag sits just above the place of the targeting triangle, whose corner is given.
fn tag_centre(corner: Vec2) -> Vec2 {
    Vec2::new(
        corner.x + crate::fire_control::CONTACT_SIZE.x * 0.5,
        corner.y - TAG_GAP - TAG * 0.5,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn tag_ships(
    navigation: (
        Res<Campaign>,
        Res<PlayerHelm>,
        Res<Encounter>,
        Res<ShipView>,
    ),
    scenery: (Res<Sea>, Res<crate::battle_view::Kit>),
    window: Single<&Window>,
    scale: Res<UiScale>,
    art: Res<BadgeArt>,
    mut parts: (Badges<TagsOnly>, Emblems, Arrows),
) {
    let (run, helm, battle, view) = navigation;
    let (sea, kit) = scenery;
    let (tags, emblems, arrows) = &mut parts;
    let size = window.size() / scale.0;
    let ship = helm.position;
    let mut looks = Vec::new();
    if run.screen() == Screen::Sailing && size.min_element() >= 32. {
        for r in battle.raiders() {
            if r.class.is_flagship()
                || !r.afloat()
                || r.body.pos.distance_squared(ship) > SIGHT_LIMIT * SIGHT_LIMIT
            {
                continue;
            }
            let profile = r.class.profile();
            let pose = sea.float(r.body.pos, r.body.heading, &profile.hull);
            let Some((corner, edge)) = crate::fire_control::contact_marker(
                &view,
                ship,
                size,
                pose,
                &profile.hull,
                kit.masthead(r.class),
            ) else {
                continue;
            };
            // A turned marker is the edge arrow for a ship out of view.
            if edge != Rot2::IDENTITY {
                continue;
            }
            let emblem = Emblem::of(r.class);
            looks.push(Look {
                centre: tag_centre(corner),
                side: TAG,
                emblem,
                turn: Look::turn(&view, emblem, Some(r.body.heading)),
                alpha: Look::alpha(r.body.pos.distance(ship)),
                pop: 0.,
                inward: None,
            });
        }
        for port in battle.ports() {
            for turret in port.turrets().iter().filter(|turret| turret.live()) {
                let at = turret.position(&port.body);
                if at.distance_squared(ship) > SIGHT_LIMIT * SIGHT_LIMIT {
                    continue;
                }
                let Some((corner, edge)) =
                    crate::fire_control::battery_marker(&view, ship, size, at)
                else {
                    continue;
                };
                if edge != Rot2::IDENTITY {
                    continue;
                }
                looks.push(Look {
                    centre: tag_centre(corner),
                    side: TAG,
                    emblem: Emblem::Fort,
                    turn: Rot2::IDENTITY,
                    alpha: Look::alpha(at.distance(ship)),
                    pop: 0.,
                    inward: None,
                });
            }
        }
    }
    dress(tags, &looks, &art, emblems, arrows);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    const SIZE: Vec2 = Vec2::new(1280., 760.);

    fn inside(p: Vec2) -> bool {
        p.cmpge(INSET - 0.01).all() && p.cmple(SIZE - INSET + 0.01).all()
    }

    #[test]
    fn bearings_in_view_sit_on_the_ring_and_astern_pins_to_the_bottom_edge() {
        let view = ShipView::astern(0.);
        let flat = CLEARANCE;
        let r = Hull::CRUISER.bow + STANDOFF;
        let ahead = place(&view, Vec2::ZERO, r, 0., flat, SIZE);
        let point = view
            .viewport_point(Vec2::ZERO, (direction(0.) * r).extend(flat).xzy(), SIZE)
            .unwrap();
        assert!(ahead.distance(point) < 0.5, "{ahead} vs {point}");
        let astern = place(&view, Vec2::ZERO, r, PI, flat, SIZE);
        assert!((astern.y - (SIZE.y - INSET.y)).abs() < 0.5, "{astern}");
        assert!((astern.x - SIZE.x * 0.5).abs() < 1., "{astern}");
        let starboard = place(&view, Vec2::ZERO, r, FRAC_PI_2, flat, SIZE);
        let port = place(&view, Vec2::ZERO, r, -FRAC_PI_2, flat, SIZE);
        assert!(starboard.x > SIZE.x * 0.5 && port.x < SIZE.x * 0.5);
        assert!((starboard.y - port.y).abs() < 0.5);
    }

    #[test]
    fn pips_go_round_the_ring_in_order_and_never_leave_the_view() {
        for look in [0., 1.1, -2.6] {
            let mut view = ShipView::astern(look);
            view.bearing = look;
            let flat = CLEARANCE;
            let r = Hull::CRUISER.bow + STANDOFF;
            let mut last = None;
            for step in 0..=72 {
                // Sweep from dead ahead clockwise to dead astern: always to starboard.
                let relative = step as f32 / 72. * PI;
                let p = place(&view, Vec2::new(40., -300.), r, look + relative, flat, SIZE);
                assert!(inside(p), "{p} at {relative}");
                assert!(p.x >= SIZE.x * 0.5 - 0.5, "{p} at {relative}");
                if let Some(q) = last {
                    assert!(p.distance(q) < 40., "jump {q} -> {p} at {relative}");
                }
                last = Some(p);
            }
        }
    }

    #[test]
    fn the_ring_rides_level_just_clear_of_every_crest() {
        // The stages' sea, then a storm.
        for state in [0.31, 1.] {
            let mut swell = shipvivor_sea::Swell::new(state, 0.4);
            let ring = ring_height(&swell);
            let mut highest = f32::MIN;
            for _ in 0..240 {
                swell.advance(0.37);
                for x in -12..=12 {
                    for y in -12..=12 {
                        let point = Vec2::new(x as f32, y as f32) * 2.9;
                        highest = highest.max(swell.height(point));
                    }
                }
            }
            assert!(
                highest < ring - CLEARANCE * 0.5,
                "sea {state}: crest {highest} vs ring {ring}"
            );
            // Close above the water, not floating well clear of it.
            assert!(
                highest > (ring - CLEARANCE) * 0.5,
                "sea {state}: crest {highest} vs ring {ring}"
            );
        }
    }

    #[test]
    fn every_live_harbour_gun_in_sight_has_a_pip_of_her_own() {
        let stage = shipvivor_campaign::Stage::IronNarrows;
        let battle = Battle::for_chart(60., &crate::passage::Passage::of(stage).classic());
        let port = &battle.ports()[2];
        assert!(port.turrets().len() >= 3);
        let player = port.body.pos + port.body.forward() * 300.;
        let forts: Vec<Vec2> = threats(&battle, player, &mut FireLog::default())
            .iter()
            .filter(|threat| threat.emblem == Emblem::Fort)
            .map(|threat| threat.at)
            .collect();
        for gun in port.turrets() {
            assert!(forts.contains(&gun.position(&port.body)), "{forts:?}");
        }
    }

    #[test]
    fn rounds_belong_to_the_nearest_gun_or_stand_alone() {
        let guns = [
            (Source::Ship(7), Vec2::new(0., 0.)),
            (Source::Battery(1, 0), Vec2::new(200., 0.)),
            (Source::Battery(1, 1), Vec2::new(200., 40.)),
        ];
        assert_eq!(attribute(&guns, Vec2::new(30., 10.)), Source::Ship(7));
        assert_eq!(attribute(&guns, Vec2::new(150., 0.)), Source::Battery(1, 0));
        assert_eq!(
            attribute(&guns, Vec2::new(205., 30.)),
            Source::Battery(1, 1)
        );
        let far = Vec2::new(900., 900.);
        assert_eq!(attribute(&guns, far), Source::Stray(far));
    }
}
