//! Water-level launch sectors and target-relative markings on the bridge sight.
use crate::{
    camera::ShipView,
    scene::Facets,
    state::{Campaign, Encounter, Guns, PlayerHelm, Screen},
};
use bevy::{light::NotShadowCaster, prelude::*};
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{
    FIXED_HALF_ARC, RACK_HALF_ARC, Raider, SPREAD_HALF_ARC, TORPEDO_RANGE, TORPEDO_SPEED,
};

/// Sectors of each width the sight can show at once: one for each beam of every set of
/// tubes amidships, or for every set in the bow and the stern.
const TRAVERSE_SLOTS: usize = 8;

#[derive(Component)]
pub enum Sector {
    /// One sector a set of tubes covers, by its place among the sectors of its width:
    /// a fixed set's narrow one ahead or astern, or one beam of a set amidships.
    Traverse {
        slot: usize,
        fixed: bool,
    },
    Spread {
        ready: bool,
    },
    Suggested,
}
fn fan(half_angle: f32, reach: f32, opacity: f32) -> Mesh {
    let mut mesh = Facets::default();
    let n = 32;
    let ray = |i| {
        let p = direction(-half_angle + 2. * half_angle * i as f32 / n as f32);
        Vec3::new(p.x, 0., p.y)
    };
    let ink = |alpha| Color::linear_rgba(1., 1., 1., alpha);
    for i in 0..n {
        let a = ray(i);
        let b = ray(i + 1);
        for (inner, outer, alpha) in [(3.5, reach - 1.1, opacity), (reach - 1.1, reach, 0.9)] {
            let col = ink(alpha);
            mesh.triangle([a * inner, a * outer, b * outer], col);
            mesh.triangle([a * inner, b * outer, b * inner], col);
        }
    }
    // A fine edge on each bearing stays visible in the glint off the water.
    for i in [0, n] {
        let a = ray(i);
        let across = Vec3::Y.cross(a) * 0.45;
        mesh.triangle([a * 4., a * reach - across, a * reach + across], ink(0.92));
    }
    mesh.finish()
}
pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut sector = |kind, mesh, color| {
        cmd.spawn((
            kind,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            })),
            Visibility::Hidden,
            Transform::default(),
            NotShadowCaster,
        ));
    };
    for slot in 0..TRAVERSE_SLOTS {
        for (fixed, half_arc) in [(false, RACK_HALF_ARC), (true, FIXED_HALF_ARC)] {
            sector(
                Sector::Traverse { slot, fixed },
                fan(half_arc, 134., 0.15),
                Color::srgb(0.14, 0.89, 0.65),
            );
        }
    }
    sector(
        Sector::Spread { ready: true },
        fan(SPREAD_HALF_ARC, TORPEDO_RANGE, 0.34),
        Color::srgb(0.18, 1., 0.52),
    );
    sector(
        Sector::Spread { ready: false },
        fan(SPREAD_HALF_ARC, TORPEDO_RANGE, 0.11),
        Color::srgb(0.92, 0.64, 0.22),
    );
    sector(
        Sector::Suggested,
        fan(0.72_f32.to_radians(), TORPEDO_RANGE, 0.42),
        Color::srgb(0.91, 0.97, 0.98),
    );
}

/// Where she will be when a round fired now at `speed` gets there, holding her course
/// and speed: the flight is worked out again to the point she will have reached.
fn lead_point(target: &Raider, player: Vec2, speed: f32) -> Vec3 {
    let mut at = target.body.pos;
    for _ in 0..3 {
        at = target.body.pos + target.motion() * (player.distance(at) / speed);
    }
    at.extend(target.class.profile().deck * 0.7).xzy()
}

pub fn place_sectors(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    battle: Res<Encounter>,
    guns: Res<Guns>,
    view: Res<ShipView>,
    sea: Res<crate::seascape::Sea>,
    mut sectors: Query<(&Sector, &mut Transform, &mut Visibility)>,
) {
    use shipvivor_shipyard::TubeAim;
    // Every sector her tubes cover, where it opens and which way it faces.
    let mut arcs: [Vec<(Vec2, f32)>; 2] = [vec![], vec![]];
    for (i, set) in battle.tube_sets().iter().enumerate() {
        let fixed = set.mount.aim != TubeAim::Beam;
        for centre in set.centres(helm.heading) {
            let at = battle.tube_origin(i, helm.position, helm.heading, centre);
            arcs[usize::from(fixed)].push((at, centre));
        }
    }
    for (kind, mut pose, mut vis) in &mut sectors {
        *vis = Visibility::Hidden;
        if run.screen() != Screen::Sailing || !guns.torpedo_mode {
            continue;
        }
        let (at, bearing, layer) = match kind {
            Sector::Traverse { slot, fixed } => {
                let Some(&(at, centre)) = arcs[usize::from(*fixed)].get(*slot) else {
                    continue;
                };
                (at, centre, 0.)
            }
            Sector::Spread { ready } => {
                let Some(set) = battle.tubes_for_aim(helm.heading, view.bearing) else {
                    continue;
                };
                if battle.tube_sets()[set].loaded() != *ready {
                    continue;
                }
                (
                    battle.tube_origin(set, helm.position, helm.heading, view.bearing),
                    view.bearing,
                    0.10,
                )
            }
            Sector::Suggested => {
                let Some(r) = view.target(&battle) else {
                    continue;
                };
                let mark = lead_point(r, helm.position, TORPEDO_SPEED).xz();
                if mark.distance(helm.position) > TORPEDO_RANGE {
                    continue;
                }
                let bearing = heading(mark - helm.position);
                let Some(set) = battle.tubes_for_aim(helm.heading, bearing) else {
                    continue;
                };
                let at = battle.tube_origin(set, helm.position, helm.heading, bearing);
                (at, heading(mark - at), 0.2)
            }
        };
        *pose = Transform::from_xyz(at.x, sea.height(at) + 1.1 + layer, at.y)
            .with_rotation(Quat::from_rotation_y(-bearing));
        *vis = Visibility::Visible;
    }
}

#[derive(Component)]
pub enum Mark {
    Contact,
    Lead,
    Tick(i8),
    Rail,
}
pub fn spawn(cmd: &mut Commands, root: Entity, contact_icon: Handle<Image>) {
    let node = || Node {
        position_type: PositionType::Absolute,
        display: Display::None,
        ..default()
    };
    cmd.spawn((
        Mark::Contact,
        Node {
            width: px(CONTACT_SIZE.x),
            height: px(CONTACT_SIZE.y),
            ..node()
        },
        ImageNode::new(contact_icon.clone()),
        ChildOf(root),
    ));
    let mint = Color::srgb_u8(117, 255, 201);
    // The contact's triangle again, smaller and in the lead's mint, its point on the lead.
    cmd.spawn((
        Mark::Lead,
        Node {
            width: px(LEAD_SIZE.x),
            height: px(LEAD_SIZE.y),
            ..node()
        },
        ImageNode::new(contact_icon).with_color(mint),
        ChildOf(root),
    ));
    let line = Color::srgba_u8(200, 239, 223, 195);
    cmd.spawn((Mark::Rail, node(), BackgroundColor(line), ChildOf(root)));
    for i in -TICKS..=TICKS {
        cmd.spawn((Mark::Tick(i), node(), BackgroundColor(line), ChildOf(root)));
    }
}
pub(crate) const CONTACT_SIZE: Vec2 = Vec2::new(17., 12.);
const CONTACT_GAP: f32 = 22.;
/// The lead triangle, whose point lies where to lay the guns.
const LEAD_SIZE: Vec2 = Vec2::new(13., 9.);
/// Ticks either side of the middle of the scale under the contact, and the pixels
/// between them. The scale is the same for every ship, at any range or speed.
const TICKS: i8 = 4;
const TICK_SPACING: f32 = 18.;

/// Keep the entire triangle above the projected, wave-tilted ship. At the edge
/// of the lens, only show a bearing arrow once the hull is fully out of view.
pub(crate) fn contact_marker(
    view: &ShipView,
    player: Vec2,
    size: Vec2,
    pose: Transform,
    hull: &shipvivor_encounter::Hull,
    masthead: f32,
) -> Option<(Vec2, Rot2)> {
    let plan = Rect::new(-hull.radius, -hull.bow, hull.radius, hull.stern);
    marker_over(view, player, size, pose, plan, masthead)
}
/// The same triangle over a harbour gun, from the foot of her plinth to her roof.
pub(crate) fn battery_marker(
    view: &ShipView,
    player: Vec2,
    size: Vec2,
    at: Vec2,
    gun: &shipvivor_encounter::ShoreTurret,
) -> Option<(Vec2, Rot2)> {
    let plan = Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(gun.radius()));
    let pose = Transform::from_xyz(at.x, gun.elevation(), at.y);
    marker_over(view, player, size, pose, plan, gun.top() - gun.elevation())
}
/// A box standing on the water: `plan` spans starboard and astern, and `top` is its height.
fn marker_over(
    view: &ShipView,
    player: Vec2,
    size: Vec2,
    pose: Transform,
    plan: Rect,
    top: f32,
) -> Option<(Vec2, Rot2)> {
    let mut bounds = Rect::EMPTY;
    let mut projected = 0;
    for x in [plan.min.x, plan.max.x] {
        for z in [plan.min.y, plan.max.y] {
            for y in [0., top] {
                let point = pose.transform_point(Vec3::new(x, y, z));
                if let Some(p) = view.viewport_point(player, point, size) {
                    bounds = bounds.union_point(p);
                    projected += 1;
                }
            }
        }
    }
    if (1..8).contains(&projected) {
        return None;
    }
    let (center, edge) = view.project_mark(player, pose.translation, size);
    if projected == 0
        || bounds
            .intersect(Rect::new(0., 0., size.x, size.y))
            .is_empty()
    {
        let turn = match edge {
            " <" => 90.,
            " >" => -90.,
            " ^" => 180.,
            _ => 0.,
        };
        return Some((center - CONTACT_SIZE * 0.5, Rot2::degrees(turn)));
    }
    if bounds.min.y < CONTACT_SIZE.y + CONTACT_GAP {
        return None;
    }
    let x = center
        .x
        .clamp(CONTACT_SIZE.x * 0.5, size.x - CONTACT_SIZE.x * 0.5);
    Some((
        Vec2::new(
            x - CONTACT_SIZE.x * 0.5,
            bounds.min.y - CONTACT_GAP - CONTACT_SIZE.y,
        ),
        Rot2::IDENTITY,
    ))
}
pub fn update_marks(
    navigation: (Res<PlayerHelm>, Res<ShipView>, Res<Encounter>),
    equipment: (Res<Campaign>, Res<Guns>),
    window: Single<&Window>,
    scale: Res<UiScale>,
    scenery: (Res<crate::battle_view::Kit>, Res<crate::seascape::Sea>),
    mut marks: Query<(&Mark, &mut Node, Option<&mut ImageNode>, &mut UiTransform)>,
) {
    let (helm, view, battle) = navigation;
    let (run, guns) = equipment;
    let size = window.size() / scale.0;
    let selected = view.quarry(&battle);
    // The lead and the range ticks are for a ship, measured off her middle.
    let ship = view.target(&battle).map(|r| {
        let profile = r.class.profile();
        let center = r.body.pos.extend(profile.deck * 0.7).xzy();
        (
            r,
            profile,
            center,
            view.project_mark(helm.position, center, size),
        )
    });
    let (kit, sea) = scenery;
    for (kind, mut node, image, mut transform) in &mut marks {
        node.display = Display::None;
        if size.min_element() < 32. {
            continue;
        }
        let Some(quarry) = selected else {
            continue;
        };
        let pos = match (kind, &ship) {
            (Mark::Contact, _) => {
                let marker = match &ship {
                    Some((r, profile, ..)) => {
                        let pose = sea.float(r.body.pos, r.body.heading, &profile.hull);
                        let masthead = kit.masthead(r.class);
                        contact_marker(&view, helm.position, size, pose, &profile.hull, masthead)
                    }
                    None => quarry.battery(&battle).and_then(|(port, gun)| {
                        battery_marker(&view, helm.position, size, gun.position(&port.body), gun)
                    }),
                };
                let Some((pos, angle)) = marker else {
                    continue;
                };
                transform.rotation = angle;
                if let Some(mut icon) = image {
                    icon.color = if view.locked() {
                        Color::srgb_u8(255, 202, 93)
                    } else {
                        Color::srgb_u8(207, 234, 228)
                    };
                }
                pos
            }
            (_, None) => continue,
            (Mark::Lead, Some((r, _, _, (_, edge)))) => {
                if !run.lead_sight || (!guns.torpedo_mode && view.zoom < 0.45) || !edge.is_empty() {
                    continue;
                }
                let speed = if guns.torpedo_mode {
                    TORPEDO_SPEED
                } else {
                    run.fittings.round().speed()
                };
                let point = lead_point(r, helm.position, speed);
                if guns.torpedo_mode && point.xz().distance(helm.position) > TORPEDO_RANGE {
                    continue;
                }
                let (lead, off) = view.project_mark(helm.position, point, size);
                if !off.is_empty() {
                    continue;
                }
                lead - Vec2::new(LEAD_SIZE.x * 0.5, LEAD_SIZE.y)
            }
            (Mark::Tick(_) | Mark::Rail, Some((_, _, _, (p, edge)))) => {
                if view.zoom < 0.45 || !edge.is_empty() {
                    continue;
                }
                let spacing = TICK_SPACING;
                let reach = f32::from(TICKS) * spacing;
                let y = p.y + 24.;
                match kind {
                    Mark::Tick(i) => {
                        let x = p.x + f32::from(*i) * spacing;
                        if !(12. ..size.x - 12.).contains(&x) || y > size.y - 25. {
                            continue;
                        }
                        node.width = px(1.5);
                        node.height = px(match i {
                            0 => 10.,
                            _ if i % 2 == 0 => 7.,
                            _ => 4.,
                        });
                        Vec2::new(x, y)
                    }
                    _ => {
                        let left = (p.x - reach).max(12.);
                        node.width = px(((p.x + reach).min(size.x - 12.) - left).max(0.));
                        node.height = px(1.);
                        Vec2::new(left, y)
                    }
                }
            }
        };
        node.left = px(pos.x);
        node.top = px(pos.y);
        node.display = Display::Flex;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::MeshAabb;
    use bevy::mesh::VertexAttributeValues;
    use shipvivor_encounter::EnemyClass::*;

    #[test]
    fn triangle_clears_the_rendered_deckwork_in_chase_and_scope() {
        let size = Vec2::new(1120., 666.);
        for class in [
            Patrol,
            TorpedoBoat,
            Destroyer,
            Freighter,
            Coaster,
            Cruiser,
            Battleship,
            Parliament,
        ] {
            let mesh = crate::fleet_models::superstructure(class);
            let mast = mesh.get_aabb().unwrap().max().y;
            let Some(VertexAttributeValues::Float32x3(vertices)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing deckwork")
            };
            let hull = &class.profile().hull;
            for zoom in [0., 1.] {
                let mut view = ShipView::astern(0.);
                view.zoom = zoom;
                for yaw in [0., 1.57, 2.4] {
                    let pose = Transform::from_xyz(0., -0.8, -100.)
                        .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, 0.07, -0.09));
                    // Aim at the model so that even the tallest mast fits in the scope.
                    let at = pose.transform_point(Vec3::Y * (mast * 0.65));
                    view.center_on(Vec2::ZERO, at);
                    let (origin, angle) =
                        contact_marker(&view, Vec2::ZERO, size, pose, hull, mast).unwrap();
                    assert_eq!(angle, Rot2::IDENTITY);
                    let tip = origin.y + CONTACT_SIZE.y;
                    let top = vertices
                        .iter()
                        .map(|v| {
                            view.viewport_point(
                                Vec2::ZERO,
                                pose.transform_point(Vec3::from_array(*v)),
                                size,
                            )
                            .unwrap()
                            .y
                        })
                        .fold(f32::INFINITY, f32::min);
                    assert!(tip + CONTACT_GAP <= top + 0.01, "{class:?} at zoom {zoom}");
                }
            }
        }
    }
    #[test]
    fn a_harbour_gun_carries_the_triangle_over_her_roof() {
        let size = Vec2::new(1120., 666.);
        let mut view = ShipView::astern(0.);
        let at = Vec2::new(30., -260.);
        let passage = crate::passage::Passage::of(shipvivor_campaign::Stage::BorealRoad);
        let battle = passage.battle(&passage.classic());
        let gun = battle
            .ports()
            .last()
            .unwrap()
            .turrets()
            .iter()
            .find(|g| g.cliff_battery())
            .unwrap();
        view.center_on(Vec2::ZERO, at.extend(gun.aim_height()).xzy());
        let (corner, turn) = battery_marker(&view, Vec2::ZERO, size, at, gun).unwrap();
        assert_eq!(turn, Rot2::IDENTITY);
        let roof = view
            .viewport_point(Vec2::ZERO, at.extend(gun.top()).xzy(), size)
            .unwrap();
        assert!(corner.y + CONTACT_SIZE.y + CONTACT_GAP <= roof.y + 0.01);
        assert!((corner.x + CONTACT_SIZE.x * 0.5 - roof.x).abs() < 1.);
    }
    #[test]
    fn a_clipped_mast_never_pushes_the_triangle_down_onto_the_target() {
        let size = Vec2::new(1120., 666.);
        let pose = Transform::from_xyz(0., 0., -40.);
        let mut view = ShipView::astern(0.);
        view.zoom = 1.;
        view.center_on(Vec2::ZERO, pose.translation + Vec3::Y);
        let mast = crate::fleet_models::superstructure(Parliament)
            .get_aabb()
            .unwrap()
            .max()
            .y;
        let hull = &Parliament.profile().hull;
        assert!(contact_marker(&view, Vec2::ZERO, size, pose, hull, mast).is_none());
        let outside = Transform::from_xyz(300., 0., -40.);
        let (pos, rotation) = contact_marker(&view, Vec2::ZERO, size, outside, hull, mast).unwrap();
        assert_eq!(rotation, Rot2::degrees(-90.));
        assert!(pos.x > size.x * 0.9 && pos.x < size.x);
    }
}
