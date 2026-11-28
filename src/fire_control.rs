//! Water-level launch sectors and target-relative markings on the bridge sight.
use crate::{
    camera::ShipView,
    state::{Campaign, Encounter, Guns, PlayerHelm, Screen},
};
use bevy::{prelude::*, render::render_resource::ShaderType};
use shipvivor_compass::heading;
use shipvivor_encounter::{
    FIXED_HALF_ARC, RACK_HALF_ARC, Raider, SPREAD_HALF_ARC, TORPEDO_RANGE, TORPEDO_SPEED,
};

/// Sectors of each width the sight can show at once: one for each beam of every set of
/// tubes amidships, or for every set in the bow and the stern.
const TRAVERSE_SLOTS: usize = 8;
/// Every fan the sea can paint at once: each traverse, the spread and the suggested lead.
/// The sea shader holds as many.
pub const FANS: usize = 2 * TRAVERSE_SLOTS + 2;

/// The launch sectors, painted into the water by the sea shader so the swell cannot
/// swallow them and a hull standing over them still hides them. Painted in order, so
/// the spread lies over the traverse and the suggested lead over both.
#[derive(ShaderType, Clone, Debug)]
pub struct Fans {
    /// Three for each fan: where it opens, east and south, its bearing and half-arc;
    /// then where it starts, its reach, the width of its rim and the opacity inside;
    /// then its linear colour.
    fans: [Vec4; 3 * FANS],
    count: u32,
}
impl Default for Fans {
    fn default() -> Self {
        Self {
            fans: [Vec4::ZERO; 3 * FANS],
            count: 0,
        }
    }
}
impl Fans {
    #[cfg(test)]
    pub fn laid(&self) -> usize {
        self.count as usize
    }
    /// A fan of `reach` metres. Its dead water by the tubes and its rim are those of
    /// the stock fan, `scale` times over.
    fn push(&mut self, at: Vec2, bearing: f32, half_arc: f32, (reach, scale): (f32, f32), look: (Color, f32)) {
        let i = self.count as usize;
        if i >= FANS {
            return;
        }
        let (color, opacity) = look;
        self.fans[3 * i] = Vec4::new(at.x, at.y, bearing, half_arc);
        self.fans[3 * i + 1] = Vec4::new(3.5 * scale, reach, 1.1 * scale, opacity);
        self.fans[3 * i + 2] = color.to_linear().to_vec4();
        self.count += 1;
    }
}

const TRAVERSE: (Color, f32) = (Color::srgb(0.14, 0.89, 0.65), 0.15);
const TRAVERSE_REACH: f32 = 134.;
const SPREAD_READY: (Color, f32) = (Color::srgb(0.18, 1., 0.52), 0.34);
const SPREAD_LOADING: (Color, f32) = (Color::srgb(0.92, 0.64, 0.22), 0.11);
const SUGGESTED: (Color, f32) = (Color::srgb(0.91, 0.97, 0.98), 0.42);
const SUGGESTED_HALF_ARC: f32 = 0.72 * std::f32::consts::PI / 180.;

/// Where she will be when a round fired now at `speed` gets there, holding her course
/// and speed: the flight is worked out again to the point she will have reached.
fn lead_point(target: &Raider, player: Vec2, speed: f32) -> Vec3 {
    let mut at = target.body.pos;
    for _ in 0..3 {
        at = target.body.pos + target.motion() * (player.distance(at) / speed);
    }
    at.extend(target.class.profile().deck * 0.7).xzy()
}

/// Lay out the fans of the tubes for the sea to paint while she lays her torpedoes.
pub fn place_sectors(
    run: Res<Campaign>,
    helm: Res<PlayerHelm>,
    battle: Res<Encounter>,
    guns: Res<Guns>,
    view: Res<ShipView>,
    mut sea: ResMut<crate::seascape::Sea>,
) {
    use shipvivor_shipyard::TubeAim;
    let mut fans = Fans::default();
    if run.screen() == Screen::Sailing && guns.torpedo_mode {
        // Every sector her tubes cover, where it opens and which way it faces: the
        // sets amidships first, then the fixed ones.
        for fixed in [false, true] {
            let half_arc = if fixed { FIXED_HALF_ARC } else { RACK_HALF_ARC };
            let mut slots = 0;
            for (i, set) in battle.tube_sets().iter().enumerate() {
                if (set.mount.aim != TubeAim::Beam) != fixed {
                    continue;
                }
                for centre in set.centres(helm.heading) {
                    if slots < TRAVERSE_SLOTS {
                        let at = battle.tube_origin(i, helm.position, helm.heading, centre);
                        fans.push(at, centre, half_arc, (TRAVERSE_REACH, 1.), TRAVERSE);
                    }
                    slots += 1;
                }
            }
        }
        // The spread's fans are stretched from the stock run to hers.
        let range = battle.torpedo_range();
        let reach = (range, range / TORPEDO_RANGE);
        if let Some(set) = battle.tubes_for_aim(helm.heading, view.bearing) {
            let look = if battle.tube_sets()[set].loaded() {
                SPREAD_READY
            } else {
                SPREAD_LOADING
            };
            let at = battle.tube_origin(set, helm.position, helm.heading, view.bearing);
            fans.push(at, view.bearing, SPREAD_HALF_ARC, reach, look);
        }
        if let Some(r) = view.target(&battle) {
            let mark = lead_point(r, helm.position, TORPEDO_SPEED).xz();
            let bearing = heading(mark - helm.position);
            if mark.distance(helm.position) <= range
                && let Some(set) = battle.tubes_for_aim(helm.heading, bearing)
            {
                let at = battle.tube_origin(set, helm.position, helm.heading, bearing);
                fans.push(at, heading(mark - at), SUGGESTED_HALF_ARC, reach, SUGGESTED);
            }
        }
    }
    sea.fans = fans;
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
                if guns.torpedo_mode && point.xz().distance(helm.position) > battle.torpedo_range()
                {
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
