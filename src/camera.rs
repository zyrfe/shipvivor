//! Ship-centred view. The helm and the sight have independent world bearings.
use crate::state::{Campaign, Mode, PlayerHelm};
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{Battle, Raider};

const CRUISE_FOV: f32 = 56_f32.to_radians();
const SIGHT_FOV: f32 = 20_f32.to_radians();
const MAX_ELEVATION: f32 = 72_f32.to_radians();
const MAX_DEPRESSION: f32 = 55_f32.to_radians();
const BOOM_OFFSET: f32 = 14_f32.to_radians();
const MIN_BOOM_PITCH: f32 = 8_f32.to_radians();
const LOOK_PER_PIXEL: f32 = 0.003;
pub const FAR_CLIP: f32 = 5200.;
pub const SKY: Color = Color::srgb(0.62, 0.74, 0.81);

#[derive(Component)]
pub struct FollowCamera;

#[derive(Clone, Copy)]
struct TargetLock {
    id: u64,
    // Starboard, height and forward displacement in the selected ship's frame.
    offset: Vec3,
}
impl TargetLock {
    fn point(self, r: &Raider) -> Vec3 {
        let f = r.body.forward();
        (r.body.pos + f.perp() * self.offset.x + f * self.offset.z)
            .extend(self.offset.y)
            .xzy()
    }
}

#[derive(Resource)]
pub struct ShipView {
    pub bearing: f32,
    depression: f32,
    pub zoom: f32,
    pub hull_extension: f32,
    had_capture: bool,
    lock: Option<TargetLock>,
    soft_target: Option<u64>,
}

impl Default for ShipView {
    fn default() -> Self {
        Self::astern(0.)
    }
}

impl ShipView {
    pub fn astern(heading: f32) -> Self {
        Self {
            bearing: heading,
            depression: 10_f32.to_radians(),
            zoom: 0.,
            hull_extension: 0.,
            had_capture: false,
            lock: None,
            soft_target: None,
        }
    }
    fn boom_length(&self) -> f32 {
        // Keep the extended stern inside the cruising view at the original sight angle.
        43. * (1. + self.hull_extension / shipvivor_encounter::Hull::CRUISER.stern)
    }

    fn fov(&self) -> f32 {
        CRUISE_FOV.lerp(SIGHT_FOV, self.zoom)
    }

    fn look(&mut self, delta: Vec2, precision: bool, dt: f32) {
        self.zoom = self
            .zoom
            .move_towards(if precision { 1. } else { 0. }, dt * 6.);
        // Keep on-screen travel per mouse pixel consistent as the lens narrows.
        let sensitivity = LOOK_PER_PIXEL * (self.fov() / 2.).tan() / (CRUISE_FOV / 2.).tan();
        self.bearing = (self.bearing + delta.x * sensitivity).rem_euclid(std::f32::consts::TAU);
        self.depression =
            (self.depression + delta.y * sensitivity).clamp(-MAX_ELEVATION, MAX_DEPRESSION);
    }

    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }
    pub fn target<'a>(&self, battle: &'a Battle) -> Option<&'a Raider> {
        battle.ship(self.lock.map(|lock| lock.id).or(self.soft_target)?)
    }
    pub fn tracked_point(&self, battle: &Battle) -> Option<Vec3> {
        self.lock
            .and_then(|lock| Some(lock.point(battle.ship(lock.id)?)))
    }
    pub fn main_aim(
        &self,
        player: Vec2,
        battle: &Battle,
        fire: bool,
        fire_selected: bool,
    ) -> shipvivor_encounter::Aim {
        use shipvivor_encounter::Aim;
        let point = if let Some(mark) = self.tracked_point(battle) {
            mark
        } else {
            let contact = self.sight(player, battle);
            if contact.ship_id.is_some()
                || (!fire_selected && (contact.yard.is_some() || contact.factory.is_some()))
            {
                contact.point
            } else if let Some(r) = self.target(battle) {
                r.aim_point(player)
            } else if fire_selected {
                match battle.nearest_ship(player) {
                    Some(r) => r.aim_point(player),
                    None => return Aim::Nearest,
                }
            } else {
                contact.point
            }
        };
        if fire || fire_selected {
            Aim::Sight(point)
        } else {
            Aim::Track(point)
        }
    }
    /// Score distance from the reticle in units of half the viewport height.
    /// A slightly wider release radius keeps the pale triangle steady near the edge.
    pub fn update_contact(&mut self, player: Vec2, battle: &Battle) {
        if let Some(lock) = self.lock {
            if battle.ship(lock.id).is_some() {
                return;
            }
            self.lock = None;
        }
        let sight = self.sight(player, battle);
        if sight.ship_id.is_some()
            || sight.yard.is_some()
            || sight.factory.is_some()
            || sight.point.y > 0.01
        {
            self.soft_target = sight.ship_id;
            return;
        }
        let pose = self.pose(player);
        let inverse = pose.rotation.inverse();
        let mut best = f32::INFINITY;
        let mut id = None;
        for r in battle.raiders() {
            let p = r.body.pos.extend(r.class.profile().deck * 0.65).xzy();
            if player.distance_squared(r.body.pos) > shipvivor_encounter::SIGHT_LIMIT.powi(2) {
                continue;
            }
            let Some(offset) =
                self.screen_offset(inverse * (p - pose.translation), Vec2::splat(2.))
            else {
                continue;
            };
            let radius = if self.soft_target == Some(r.body.id) {
                0.18
            } else {
                0.14
            };
            let score = offset.length() / radius;
            if score <= 1. && score < best {
                let ray = (p - pose.translation).normalize_or_zero();
                if battle.sight(pose.translation, ray, p.xz()).ship_id == Some(r.body.id) {
                    id = Some(r.body.id);
                    best = score;
                }
            }
        }
        self.soft_target = id;
    }
    fn toggle_lock(&mut self, player: Vec2, battle: &Battle) {
        if self.lock.take().is_some() {
            self.update_contact(player, battle);
            return;
        }
        let sight = self.sight(player, battle);
        let Some(ship) = self.target(battle).or_else(|| battle.nearest_ship(player)) else {
            return;
        };
        let p = if sight.ship_id == Some(ship.body.id) {
            sight.point
        } else {
            ship.body.pos.extend(ship.class.profile().deck * 0.75).xzy()
        };
        let d = p.xz() - ship.body.pos;
        let f = ship.body.forward();
        self.lock = Some(TargetLock {
            id: ship.body.id,
            offset: Vec3::new(d.dot(f.perp()), p.y, d.dot(f)),
        });
        self.soft_target = None;
    }
    pub(crate) fn center_on(&mut self, player: Vec2, mark: Vec3) {
        let delta = mark.xz() - player;
        if delta.length_squared() < 0.01 {
            return;
        }
        self.bearing = heading(delta);
        // The chase camera boom sits fourteen degrees above the viewing ray.
        let range = delta.length();
        let height = 3. - mark.y;
        let boom = self.boom_length();
        let lift = (boom * BOOM_OFFSET.sin() / range.hypot(height)).clamp(-1., 1.);
        let depression = height.atan2(range) + lift.asin();
        // Looking aloft holds the eye above the deck while the lens follows the plane.
        self.depression = if depression + BOOM_OFFSET < MIN_BOOM_PITCH {
            (height + boom * MIN_BOOM_PITCH.sin()).atan2(range + boom * MIN_BOOM_PITCH.cos())
        } else {
            depression
        }
        .clamp(-MAX_ELEVATION, MAX_DEPRESSION);
    }
    fn trim(&mut self, player: Vec2, battle: &Battle, delta: Vec2) {
        let sensitivity = LOOK_PER_PIXEL * (self.fov() * 0.5).tan() / (CRUISE_FOV * 0.5).tan();
        if let Some(lock) = &mut self.lock
            && let Some(ship) = battle.ship(lock.id)
        {
            let m = ship.body.pos.distance(player).max(16.) * sensitivity;
            let f = ship.body.forward();
            let right = (lock.point(ship).xz() - player).normalize_or_zero().perp();
            let across = right * delta.x * m;
            lock.offset.x = (lock.offset.x + across.dot(f.perp())).clamp(-200., 200.);
            lock.offset.z = (lock.offset.z + across.dot(f)).clamp(-200., 200.);
            lock.offset.y = (lock.offset.y - delta.y * m).clamp(0.35, 28.);
        }
        if let Some(mark) = self.tracked_point(battle) {
            self.center_on(player, mark);
        }
    }

    fn pose(&self, ship: Vec2) -> Transform {
        let ahead = direction(self.bearing);
        let forward = Vec3::new(ahead.x, 0., ahead.y);
        // Pitch the boom above the sightline so the hull stays below the crosshair.
        let boom = (self.depression + BOOM_OFFSET).max(MIN_BOOM_PITCH);
        let pivot = Vec3::new(ship.x, 3., ship.y);
        let eye = pivot + self.boom_length() * (Vec3::Y * boom.sin() - forward * boom.cos());
        let sight = forward * self.depression.cos() - Vec3::Y * self.depression.sin();
        Transform::from_translation(eye).looking_to(sight, Vec3::Y)
    }

    pub fn screen_span(&self, ship: Vec2, point: Vec3, width: f32, size: Vec2) -> f32 {
        let pose = self.pose(ship);
        let depth = (point - pose.translation)
            .dot(pose.forward().as_vec3())
            .max(1.);
        width * size.y * 0.5 / (depth * (self.fov() * 0.5).tan())
    }
    /// Unclipped HUD coordinates for geometry in front of the camera.
    pub fn viewport_point(&self, ship: Vec2, point: Vec3, size: Vec2) -> Option<Vec2> {
        let pose = self.pose(ship);
        let p = pose.rotation.inverse() * (point - pose.translation);
        self.screen_offset(p, size)
            .map(|offset| size * 0.5 + offset)
    }
    fn screen_offset(&self, camera_point: Vec3, size: Vec2) -> Option<Vec2> {
        (camera_point.z < -0.01).then(|| {
            Vec2::new(camera_point.x, -camera_point.y)
                * (size.y * 0.5 / ((self.fov() * 0.5).tan() * -camera_point.z))
        })
    }
    /// Offscreen points get an arrow on the edge of the HUD.
    pub fn project_mark(&self, ship: Vec2, point: Vec3, size: Vec2) -> (Vec2, &'static str) {
        let pose = self.pose(ship);
        let p = pose.rotation.inverse() * (point - pose.translation);
        let half = size * 0.5;
        let visible = (half - Vec2::new(29., 39.)).max(Vec2::ONE);
        if let Some(offset) = self.screen_offset(p, size) {
            if offset.abs().cmple(visible).all() {
                return (half + offset, "");
            }
            let relative = offset / visible;
            let edge = (relative.abs()).max_element();
            let label = if relative.x.abs() >= relative.y.abs() {
                if relative.x < 0. { " <" } else { " >" }
            } else if relative.y < 0. {
                " ^"
            } else {
                " v"
            };
            return (half + offset / edge, label);
        }
        let right = p.x >= 0.;
        (
            half + Vec2::X * if right { visible.x } else { -visible.x },
            if right { " >" } else { " <" },
        )
    }

    pub fn sight_target(&self, ship: Vec2) -> Vec2 {
        let pose = self.pose(ship);
        let ray = pose.forward();
        let forward = direction(self.bearing);
        let range = if ray.y < -0.001 {
            let water = pose.translation + ray.as_vec3() * (-pose.translation.y / ray.y);
            (water.xz() - ship).dot(forward)
        } else {
            shipvivor_encounter::SIGHT_LIMIT
        };
        ship + forward * range.clamp(16., shipvivor_encounter::SIGHT_LIMIT)
    }

    pub fn sight(
        &self,
        ship: Vec2,
        battle: &shipvivor_encounter::Battle,
    ) -> shipvivor_encounter::Sighting {
        let pose = self.pose(ship);
        battle.sight(
            pose.translation,
            pose.forward().as_vec3(),
            self.sight_target(ship),
        )
    }
}

pub fn setup(mut commands: Commands) {
    commands.spawn((
        FollowCamera,
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(SKY),
            ..default()
        },
        Tonemapping::Reinhard,
        DistanceFog {
            color: SKY,
            falloff: FogFalloff::Linear {
                start: 480.,
                end: 3650.,
            },
            ..default()
        },
        Projection::from(PerspectiveProjection {
            far: FAR_CLIP,
            ..default()
        }),
    ));
}

/// Track just off the starboard bow. The horizon stays level while the ship pitches
/// independently. A tall window puts the ship above the menu; a wide one frames her to its right.
fn title_pose(helm: &PlayerHelm, sea: &crate::seascape::Sea, aspect: f32) -> (Transform, f32) {
    let wide = ((aspect - 0.95) / 0.55).clamp(0., 1.);
    let fov = 37_f32.to_radians();
    let ahead = helm.forward();
    let eye = helm.position + ahead * (25.5 + 12. * (1. - wide)) + ahead.perp() * 9.6;
    let height = 0.93 + sea.height(eye);
    let focus = (helm.position + ahead * 6.)
        .extend(2.5 + 0.25 * sea.swell.height(helm.position))
        .xzy();
    let mut pose = Transform::from_translation(eye.extend(height).xzy()).looking_at(focus, Vec3::Y);
    let half_lens = (fov * 0.5).tan();
    pose.rotate_y((0.18 * wide * aspect.min(2.3) * half_lens).atan());
    pose.rotate_local_x(-((0.07 + 0.34 * (1. - wide)) * half_lens).atan());
    (pose, fov)
}

pub fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    input: (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>),
    time: Res<Time>,
    run: Res<Campaign>,
    navigation: (Res<PlayerHelm>, Res<crate::state::Encounter>),
    window: Single<(&Window, &CursorOptions)>,
    mut view: ResMut<ShipView>,
) {
    let (helm, battle) = navigation;
    let (keys, mouse) = input;
    let (window, cursor) = *window;
    let captured = run.under_way() && window.focused && cursor.grab_mode == CursorGrabMode::Locked;
    // A backend can deliver a warp as raw motion on the first frame after locking.
    let delta = if captured && view.had_capture {
        motion.delta
    } else {
        Vec2::ZERO
    };
    view.had_capture = captured;
    if !run.under_way() || !window.focused {
        return;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        view.bearing = helm.heading;
        view.depression = ShipView::default().depression;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if view.locked() && captured && mouse.pressed(MouseButton::Right) {
        view.look(Vec2::ZERO, shift, time.delta_secs());
        view.trim(helm.position, &battle, delta);
    } else {
        view.look(delta, shift, time.delta_secs());
    }
    if run.mode == Mode::Patrol {
        view.update_contact(helm.position, &battle);
        if keys.just_pressed(KeyCode::KeyX) {
            view.toggle_lock(helm.position, &battle);
        }
    }
}

pub fn cursor_mode(run: Res<Campaign>, mut window: Single<(&Window, &mut CursorOptions)>) {
    let (window, options) = &mut *window;
    let capture = window.focused && run.under_way();
    let mode = if capture {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    if options.grab_mode != mode || options.visible == capture {
        options.grab_mode = mode;
        options.visible = !capture;
    }
}

pub fn follow(
    helm: Res<PlayerHelm>,
    run: Res<Campaign>,
    view: Res<ShipView>,
    sea: Res<crate::seascape::Sea>,
    window: Single<&Window>,
    mut framed_length: Local<Option<f32>>,
    mut camera: Single<(&mut Transform, &mut Projection), With<FollowCamera>>,
) {
    let (pose, projection) = &mut *camera;
    if matches!(run.screen(), crate::state::Screen::Drydock(_)) {
        let length = run.ship.design().length();
        if *framed_length != Some(length) || !matches!(**projection, Projection::Orthographic(_)) {
            let (dock_pose, dock_projection) = crate::dockyard::camera(&run);
            **pose = dock_pose;
            **projection = dock_projection;
            *framed_length = Some(length);
        }
    } else {
        *framed_length = None;
        let (next_pose, fov) = if run.mode == Mode::Title {
            title_pose(&helm, &sea, window.width() / window.height().max(1.))
        } else {
            (view.pose(helm.position), view.fov())
        };
        **pose = next_pose;
        let lens = match &**projection {
            Projection::Perspective(lens) if lens.fov == fov => return,
            Projection::Perspective(lens) => PerspectiveProjection {
                fov,
                ..lens.clone()
            },
            _ => PerspectiveProjection {
                fov,
                far: FAR_CLIP,
                ..default()
            },
        };
        **projection = Projection::Perspective(lens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_wraps_and_the_sight_stays_above_water_at_both_pitch_limits() {
        let ship = Vec2::new(37., -19.);
        let mut view = ShipView::astern(1.25);
        for delta_y in [-1e5, 1e5] {
            view.look(Vec2::new(1e5, delta_y), false, 1. / 60.);
            assert!((0. ..std::f32::consts::TAU).contains(&view.bearing));
            let pose = view.pose(ship);
            assert!(pose.translation.y > 1.);
            let centre_ray = pose.forward().as_vec3();
            assert_eq!(centre_ray.y > 0., delta_y < 0.);
            let horizontal = Vec2::new(centre_ray.x, centre_ray.z).normalize();
            assert!(horizontal.abs_diff_eq((view.sight_target(ship) - ship).normalize(), 1e-5));
            let shifted = view.pose(ship + Vec2::new(7., -5.));
            assert!(
                (shifted.translation - pose.translation).abs_diff_eq(Vec3::new(7., 0., -5.), 1e-5)
            );
        }
    }

    #[test]
    fn the_bridge_can_follow_a_high_pass_without_dipping_the_camera_underwater() {
        let mut view = ShipView::default();
        let mark = Vec3::new(12., 50., -14.);
        view.center_on(Vec2::ZERO, mark);
        let camera = view.pose(Vec2::ZERO);
        assert!(camera.translation.y > 3.);
        let direction = (mark - camera.translation).normalize();
        assert!(direction.dot(camera.forward().as_vec3()) > 0.9999);
    }

    #[test]
    fn magnification_keeps_the_same_centre_ray_and_scales_mouse_travel() {
        let mut cruise = ShipView::default();
        let mut sight = ShipView::default();
        let point = Vec2::new(-18., 29.);
        sight.look(Vec2::ZERO, true, 0.25);
        assert_eq!(cruise.pose(point), sight.pose(point));
        let gain = (cruise.fov() / 2.).tan() / (sight.fov() / 2.).tan();
        assert!((2.9..3.1).contains(&gain));
        let motion = Vec2::new(10., 12.);
        cruise.look(motion, false, 0.);
        sight.look(motion, true, 0.);
        assert!((cruise.bearing - gain * sight.bearing).abs() < 1e-5);
        let aimed = sight.pose(point);
        sight.look(Vec2::ZERO, false, 0.25);
        assert_eq!(aimed, sight.pose(point));
        assert!((sight.fov() - CRUISE_FOV).abs() < 1e-6);
    }
}
