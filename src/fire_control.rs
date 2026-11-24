//! Water-level launch sectors and target-relative markings on the bridge sight.
use crate::{
    camera::ShipView,
    scene::Facets,
    state::{Campaign, Encounter, Guns, PlayerHelm, Screen},
};
use bevy::{light::NotShadowCaster, prelude::*};
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{
    RACK_HALF_ARC, Rack, Raider, SPREAD_HALF_ARC, TORPEDO_RANGE, TORPEDO_SPEED,
};

#[derive(Component)]
pub enum Sector {
    Traverse(Rack),
    Spread { ready: bool },
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
    for rack in Rack::BOTH {
        sector(
            Sector::Traverse(rack),
            fan(RACK_HALF_ARC, 134., 0.15),
            Color::srgb(0.14, 0.89, 0.65),
        );
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

// An in-game ghost holds the actor's current movement for its approximate travel time.
fn lead_point(target: &Raider, player: Vec2, speed: f32) -> Vec3 {
    let seconds = player.distance(target.body.pos) / speed;
    (target.body.pos + target.motion() * seconds)
        .extend(target.class.profile().deck * 0.7)
        .xzy()
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
    for (kind, mut pose, mut vis) in &mut sectors {
        *vis = Visibility::Hidden;
        if run.screen() != Screen::Sailing || !guns.torpedo_mode {
            continue;
        }
        let (at, bearing, layer) = match kind {
            Sector::Traverse(r) => (
                battle.rack_origin(*r, helm.position, helm.heading),
                r.center(helm.heading),
                0.,
            ),
            Sector::Spread { ready } => {
                let Some(r) = Rack::for_aim(helm.heading, view.bearing) else {
                    continue;
                };
                if (battle.rack_reload(r) == 0.) != *ready {
                    continue;
                }
                (
                    battle.rack_origin(r, helm.position, helm.heading),
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
                let Some(rack) = Rack::for_aim(helm.heading, bearing) else {
                    continue;
                };
                let at = battle.rack_origin(rack, helm.position, helm.heading);
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
    let pale = Color::srgb_u8(208, 239, 226);
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
        ImageNode::new(contact_icon),
        ChildOf(root),
    ));
    let mint = Color::srgb_u8(117, 255, 201);
    cmd.spawn((
        Mark::Lead,
        Node {
            width: px(9),
            height: px(9),
            border: UiRect::all(px(1.5)),
            border_radius: BorderRadius::MAX,
            ..node()
        },
        BorderColor::all(mint),
        ChildOf(root),
    ));
    let line = Color::srgba_u8(200, 239, 223, 195);
    cmd.spawn((Mark::Rail, node(), BackgroundColor(line), ChildOf(root)));
    for i in -4_i8..=4 {
        let major = i % 2 == 0;
        let tick = cmd
            .spawn((Mark::Tick(i), node(), BackgroundColor(line), ChildOf(root)))
            .id();
        if major {
            let label = if i == 0 {
                "0".to_string()
            } else {
                format!("{:+}", i / 2)
            };
            cmd.spawn((
                Text::new(label),
                TextFont::from_font_size(10.),
                TextColor(pale),
                TextShadow::default(),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(8),
                    left: px(-6),
                    ..default()
                },
                ChildOf(tick),
            ));
        }
    }
}
const CONTACT_SIZE: Vec2 = Vec2::new(17., 12.);
const CONTACT_GAP: f32 = 22.;

/// Keep the entire triangle above the projected, wave-tilted ship. At the edge
/// of the lens, only show a bearing arrow once the hull is fully out of view.
fn contact_marker(
    view: &ShipView,
    player: Vec2,
    size: Vec2,
    pose: Transform,
    hull: &shipvivor_encounter::Hull,
    masthead: f32,
) -> Option<(Vec2, Rot2)> {
    let mut bounds = Rect::EMPTY;
    let mut projected = 0;
    for x in [-hull.radius, hull.radius] {
        for z in [-hull.bow, hull.stern] {
            for y in [0., masthead] {
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
    let selected = view.target(&battle);
    let (kit, sea) = scenery;
    for (kind, mut node, image, mut transform) in &mut marks {
        node.display = Display::None;
        if size.min_element() < 32. {
            continue;
        }
        let Some(r) = selected else {
            continue;
        };
        let profile = r.class.profile();
        let center = r.body.pos.extend(profile.deck * 0.7).xzy();
        let (p, edge) = view.project_mark(helm.position, center, size);
        let pos = match kind {
            Mark::Contact => {
                let pose = sea.float(r.body.pos, r.body.heading, &profile.hull);
                let Some((pos, angle)) = contact_marker(
                    &view,
                    helm.position,
                    size,
                    pose,
                    &profile.hull,
                    kit.masthead(r.class),
                ) else {
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
            Mark::Lead => {
                if (!guns.torpedo_mode && view.zoom < 0.45) || !edge.is_empty() {
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
                lead - Vec2::splat(4.5)
            }
            Mark::Tick(_) | Mark::Rail => {
                if view.zoom < 0.45 || !edge.is_empty() {
                    continue;
                }
                let half = (profile.hull.bow + profile.hull.stern) * 0.5;
                // Project a half ship length at the contact's actual depth.
                let spacing = view.screen_span(helm.position, center, half, size);
                let y = p.y + 24.;
                match kind {
                    Mark::Tick(i) => {
                        let x = p.x + f32::from(*i) * spacing;
                        if !(12. ..size.x - 12.).contains(&x) || y > size.y - 25. {
                            continue;
                        }
                        node.width = px(1.5);
                        node.height = px(if i % 2 == 0 { 8. } else { 4. });
                        Vec2::new(x, y)
                    }
                    _ => {
                        let left = (p.x - 4. * spacing).max(12.);
                        node.width = px(((p.x + 4. * spacing).min(size.x - 12.) - left).max(0.));
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
            Patrol, Destroyer, Freighter, Cruiser, Battleship, Parliament,
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
