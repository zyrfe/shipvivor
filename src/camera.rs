//! Ship-centred view. The helm and the sight have independent world bearings.
use crate::state::{Campaign, Mode, PlayerHelm};
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use shipvivor_compass::{direction, heading};
use shipvivor_encounter::{Battle, Raider, Sighting};
use std::f32::consts::{PI, TAU};

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

/// What the sight can mark: an enemy ship, or one of a harbour's guns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quarry {
    Ship(u64),
    /// A harbour, and which of her guns.
    Battery(usize, usize),
}
impl Quarry {
    /// A harbour gun's hit volume in the encounter: a drum five metres across its middle
    /// that stands on its plinth up to 7.5 metres, marked halfway up its gunhouse.
    pub const GUN_RADIUS: f32 = 5.;
    pub const GUN_TOP: f32 = 7.5;
    const GUN_MARK: f32 = 5.7;

    /// Whatever ship or gun a sighting has under the reticle.
    fn sighted(sight: &Sighting) -> Option<Self> {
        sight.ship_id.map(Self::Ship).or(sight
            .shore_turret
            .map(|(port, gun)| Self::Battery(port, gun)))
    }
    fn seen(self, sight: &Sighting) -> bool {
        Self::sighted(sight) == Some(self)
    }
    /// Every harbour gun still able to fire.
    fn batteries(battle: &Battle) -> impl Iterator<Item = Self> + '_ {
        battle.ports().iter().enumerate().flat_map(|(p, port)| {
            (port.turrets().iter().enumerate())
                .filter(|(_, gun)| gun.live())
                .map(move |(g, _)| Self::Battery(p, g))
        })
    }
    /// Her place on the water and the way her frame faces, while she is still a threat.
    pub fn frame(self, battle: &Battle) -> Option<(Vec2, Vec2)> {
        match self {
            Self::Ship(id) => battle.ship(id).map(|r| (r.body.pos, r.body.forward())),
            Self::Battery(port, gun) => {
                let port = battle.ports().get(port)?;
                let gun = port.turrets().get(gun).filter(|gun| gun.live())?;
                Some((gun.position(&port.body), port.body.forward()))
            }
        }
    }
    /// The point the soft lock scores and the scoped sight rides on.
    fn mark(self, battle: &Battle) -> Option<Vec3> {
        match self {
            Self::Ship(id) => battle
                .ship(id)
                .map(|r| r.body.pos.extend(r.class.profile().deck * 0.65).xzy()),
            Self::Battery(..) => {
                (self.frame(battle)).map(|(at, _)| at.extend(Self::GUN_MARK).xzy())
            }
        }
    }
    /// Where the free sight's snap lays the guns.
    fn aim_point(self, battle: &Battle, player: Vec2) -> Option<Vec3> {
        match self {
            Self::Ship(id) => battle.ship(id).map(|r| r.aim_point(player)),
            Self::Battery(..) => self.mark(battle),
        }
    }
    /// Where X pins her when the reticle is off her.
    fn pin(self, battle: &Battle) -> Option<Vec3> {
        match self {
            Self::Ship(id) => battle
                .ship(id)
                .map(|r| r.body.pos.extend(r.class.profile().deck * 0.75).xzy()),
            Self::Battery(..) => self.mark(battle),
        }
    }
    /// Bow and stern along a ship's deck, which the scope scores a near miss against.
    fn hull_line(self, battle: &Battle) -> Option<(Vec3, Vec3)> {
        let Self::Ship(id) = self else {
            return None;
        };
        let r = battle.ship(id)?;
        let middle = self.mark(battle)?;
        let ahead = r.body.forward().extend(0.).xzy();
        let hull = &r.class.profile().hull;
        Some((middle + ahead * hull.bow, middle - ahead * hull.stern))
    }
}

#[derive(Clone, Copy)]
struct TargetLock {
    quarry: Quarry,
    // Starboard, height and forward displacement in her frame.
    offset: Vec3,
}
impl TargetLock {
    fn point(self, battle: &Battle) -> Option<Vec3> {
        let (at, f) = self.quarry.frame(battle)?;
        Some(
            (at + f.perp() * self.offset.x + f * self.offset.z)
                .extend(self.offset.y)
                .xzy(),
        )
    }
}

/// The contact the scoped reticle rides with, and how far off her middle it sits.
#[derive(Clone, Copy)]
struct Zero {
    quarry: Quarry,
    // Bearing and depression from the sight that would centre her.
    offset: Vec2,
}

/// Screen distance from the reticle to the nearest point of a projected hull line.
fn miss_distance(a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = (-a).dot(ab) / ab.length_squared().max(1e-9);
    (a + ab * t.clamp(0., 1.)).length()
}

#[derive(Resource)]
pub struct ShipView {
    pub bearing: f32,
    depression: f32,
    pub zoom: f32,
    pub hull_extension: f32,
    had_capture: bool,
    lock: Option<TargetLock>,
    soft_target: Option<Quarry>,
    /// Shift is held: the lens is narrowing or narrowed, and the guns follow the reticle.
    scoped: bool,
    zero: Option<Zero>,
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
            scoped: false,
            zero: None,
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
        self.scoped = precision;
        self.zoom = self
            .zoom
            .move_towards(if precision { 1. } else { 0. }, dt * 6.);
        // Keep on-screen travel per mouse pixel consistent as the lens narrows.
        let sensitivity = LOOK_PER_PIXEL * (self.fov() / 2.).tan() / (CRUISE_FOV / 2.).tan();
        self.bearing = (self.bearing + delta.x * sensitivity).rem_euclid(TAU);
        self.depression =
            (self.depression + delta.y * sensitivity).clamp(-MAX_ELEVATION, MAX_DEPRESSION);
    }

    pub fn locked(&self) -> bool {
        self.lock.is_some()
    }
    /// The ship or gun pinned or picked, while she is still a threat.
    pub fn quarry(&self, battle: &Battle) -> Option<Quarry> {
        (self.lock.map(|lock| lock.quarry).or(self.soft_target))
            .filter(|quarry| quarry.frame(battle).is_some())
    }
    /// The marked ship, when the mark is a ship.
    pub fn target<'a>(&self, battle: &'a Battle) -> Option<&'a Raider> {
        match self.quarry(battle)? {
            Quarry::Ship(id) => battle.ship(id),
            Quarry::Battery(..) => None,
        }
    }
    pub fn tracked_point(&self, battle: &Battle) -> Option<Vec3> {
        self.lock.and_then(|lock| lock.point(battle))
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
            let quarry = self.quarry(battle);
            if Quarry::sighted(&contact).is_some()
                || (!fire_selected && (contact.yard.is_some() || contact.factory.is_some()))
            {
                contact.point
            } else if self.scoped && !fire_selected {
                // No snap through the scope: the guns follow the reticle, ranged on the
                // contact it rides with.
                quarry
                    .and_then(|quarry| quarry.frame(battle))
                    .map_or(contact.point, |(at, _)| {
                        self.zeroed(player, at, contact.point)
                    })
            } else if let Some(point) = quarry.and_then(|quarry| quarry.aim_point(battle, player)) {
                point
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
    /// Where the reticle crosses the upright plane through the contact, square to the
    /// line of sight. Rock or ice short of her stops the round first.
    fn zeroed(&self, player: Vec2, at: Vec2, contact: Vec3) -> Vec3 {
        let pose = self.pose(player);
        let ray = pose.forward().as_vec3();
        let reach = at - pose.translation.xz();
        let across = reach.normalize_or_zero();
        let closing = ray.xz().dot(across);
        if closing < 0.05 {
            return contact;
        }
        let range = reach.length();
        let solid = contact.y > 0.01 && (contact.xz() - pose.translation.xz()).dot(across) < range;
        if solid {
            contact
        } else {
            pose.translation + ray * (range / closing)
        }
    }

    /// Pick the soft target, then, through the scope, note where the reticle sits off her.
    pub fn update_contact(&mut self, player: Vec2, battle: &Battle) {
        self.pick_contact(player, battle);
        self.zero = None;
        if !self.scoped || self.lock.is_some() {
            return;
        }
        let Some(quarry) = self.soft_target else {
            return;
        };
        if let Some(mark) = quarry.mark(battle)
            && let Some((bearing, depression)) = self.centering(player, mark)
        {
            let turn = (self.bearing - bearing + PI).rem_euclid(TAU) - PI;
            self.zero = Some(Zero {
                quarry,
                offset: Vec2::new(turn, self.depression - depression),
            });
        }
    }

    /// Carry the scoped reticle along as her contact and the ship both move.
    pub fn ride_zero(&mut self, player: Vec2, battle: &Battle) {
        let Some(zero) = self.zero else {
            return;
        };
        if let Some(mark) = zero.quarry.mark(battle)
            && let Some((bearing, depression)) = self.centering(player, mark)
        {
            self.bearing = (bearing + zero.offset.x).rem_euclid(TAU);
            self.depression = (depression + zero.offset.y).clamp(-MAX_ELEVATION, MAX_DEPRESSION);
        }
    }

    /// Score distance from the reticle in units of half the viewport height.
    /// A slightly wider release radius keeps the pale triangle steady near the edge.
    /// Through the scope the score runs to a ship's whole hull line at the open lens's
    /// angular reach, so narrowing the lens keeps her, and so does land behind her.
    /// Harbour guns are scored like ships, and the free sight still finds one from the
    /// rock it stands on.
    fn pick_contact(&mut self, player: Vec2, battle: &Battle) {
        if let Some(lock) = self.lock {
            if lock.quarry.frame(battle).is_some() {
                return;
            }
            self.lock = None;
        }
        let sight = self.sight(player, battle);
        let hit = Quarry::sighted(&sight);
        if hit.is_some() || sight.yard.is_some() || sight.factory.is_some() {
            self.soft_target = hit;
            return;
        }
        let ashore = sight.point.y > 0.01 && !self.scoped;
        let ships = battle.raiders().iter().map(|r| Quarry::Ship(r.body.id));
        let candidates: Vec<Quarry> = if ashore {
            Quarry::batteries(battle).collect()
        } else {
            ships.chain(Quarry::batteries(battle)).collect()
        };
        let pose = self.pose(player);
        let inverse = pose.rotation.inverse();
        let screen =
            |p: Vec3| self.screen_offset(inverse * (p - pose.translation), Vec2::splat(2.));
        let lens = if self.scoped {
            (CRUISE_FOV * 0.5).tan() / (self.fov() * 0.5).tan()
        } else {
            1.
        };
        let mut best = f32::INFINITY;
        let mut picked = None;
        for quarry in candidates {
            let Some(p) = quarry.mark(battle) else {
                continue;
            };
            if player.distance_squared(p.xz()) > shipvivor_encounter::SIGHT_LIMIT.powi(2) {
                continue;
            }
            let Some(offset) = screen(p) else {
                continue;
            };
            let line = quarry.hull_line(battle).filter(|_| self.scoped);
            let miss = match line.map(|(bow, stern)| (screen(bow), screen(stern))) {
                Some((Some(bow), Some(stern))) => miss_distance(bow, stern),
                _ => offset.length(),
            };
            let radius = lens
                * if self.soft_target == Some(quarry) {
                    0.18
                } else {
                    0.14
                };
            let score = miss / radius;
            if score <= 1. && score < best {
                let ray = (p - pose.translation).normalize_or_zero();
                if quarry.seen(&battle.sight(pose.translation, ray, p.xz())) {
                    picked = Some(quarry);
                    best = score;
                }
            }
        }
        self.soft_target = picked;
    }
    fn toggle_lock(&mut self, player: Vec2, battle: &Battle) {
        if self.lock.take().is_some() {
            self.update_contact(player, battle);
            return;
        }
        let sight = self.sight(player, battle);
        let Some(quarry) = (self.quarry(battle))
            .or_else(|| battle.nearest_ship(player).map(|r| Quarry::Ship(r.body.id)))
        else {
            return;
        };
        let point = if quarry.seen(&sight) {
            Some(sight.point)
        } else {
            quarry.pin(battle)
        };
        let (Some(p), Some((at, f))) = (point, quarry.frame(battle)) else {
            return;
        };
        let d = p.xz() - at;
        self.lock = Some(TargetLock {
            quarry,
            offset: Vec3::new(d.dot(f.perp()), p.y, d.dot(f)),
        });
        self.soft_target = None;
    }
    pub(crate) fn center_on(&mut self, player: Vec2, mark: Vec3) {
        if let Some((bearing, depression)) = self.centering(player, mark) {
            self.bearing = bearing;
            self.depression = depression;
        }
    }
    /// The bearing and depression that put `mark` under the reticle.
    fn centering(&self, player: Vec2, mark: Vec3) -> Option<(f32, f32)> {
        let delta = mark.xz() - player;
        if delta.length_squared() < 0.01 {
            return None;
        }
        // The chase camera boom sits fourteen degrees above the viewing ray.
        let range = delta.length();
        let height = 3. - mark.y;
        let boom = self.boom_length();
        let lift = (boom * BOOM_OFFSET.sin() / range.hypot(height)).clamp(-1., 1.);
        let depression = height.atan2(range) + lift.asin();
        // Looking aloft holds the eye above the deck while the lens follows the plane.
        let depression = if depression + BOOM_OFFSET < MIN_BOOM_PITCH {
            (height + boom * MIN_BOOM_PITCH.sin()).atan2(range + boom * MIN_BOOM_PITCH.cos())
        } else {
            depression
        };
        Some((
            heading(delta),
            depression.clamp(-MAX_ELEVATION, MAX_DEPRESSION),
        ))
    }
    fn trim(&mut self, player: Vec2, battle: &Battle, delta: Vec2) {
        let sensitivity = LOOK_PER_PIXEL * (self.fov() * 0.5).tan() / (CRUISE_FOV * 0.5).tan();
        if let Some(lock) = &mut self.lock
            && let Some((at, f)) = lock.quarry.frame(battle)
            && let Some(point) = lock.point(battle)
        {
            let m = at.distance(player).max(16.) * sensitivity;
            let right = (point.xz() - player).normalize_or_zero().perp();
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
    let eye = helm.position + ahead * (20.3 + 13. * (1. - wide)) + ahead.perp() * 8.2;
    let height = 0.93 + sea.height(eye);
    let focus = (helm.position + ahead * 10.1)
        .extend(2.75 + 0.25 * sea.swell.height(helm.position))
        .xzy();
    let mut pose = Transform::from_translation(eye.extend(height).xzy()).looking_at(focus, Vec3::Y);
    let half_lens = (fov * 0.5).tan();
    pose.rotate_y((0.125 * wide * aspect.min(2.3) * half_lens).atan());
    pose.rotate_local_x(-((0.07 + 0.34 * (1. - wide)) * half_lens).atan());
    (pose, fov)
}

/// Stand off the face of the flag at the masthead, the side that reads with the hoist on
/// the left, a little below it so it flies against the sky. The cloth turns with the
/// wind over the deck and the camera turns with it, keeping the panel's side of the
/// frame clear. The hull's roll is left out, so the flag sways in the frame.
fn flag_pose(
    helm: &PlayerHelm,
    sea: &crate::seascape::Sea,
    masthead: &Transform,
    aspect: f32,
) -> (Transform, f32) {
    let fov = 24_f32.to_radians();
    let half_lens = (fov * 0.5).tan();
    let hull = Transform::from_xyz(
        helm.position.x,
        0.25 * sea.swell.height(helm.position),
        helm.position.y,
    )
    .with_rotation(Quat::from_rotation_y(-helm.heading));
    let cloth = hull * *masthead;
    // The cloth runs 5.5 along its x from the hoist and hangs 1.96 below its head.
    let middle = cloth.transform_point(Vec3::new(2.75, -1.1, 0.));
    let face = (cloth.rotation * Vec3::Z).with_y(0.).normalize_or(Vec3::X);
    // Wide enough that the cloth takes about half the frame's width.
    let distance = 5.5 * masthead.scale.x / (0.96 * half_lens * aspect);
    let eye = middle + face * distance - Vec3::Y * (0.12 * distance);
    let mut pose = Transform::from_translation(eye).looking_at(middle, Vec3::Y);
    pose.rotate_y((0.36 * half_lens * aspect).atan());
    pose.rotate_local_x(-(0.08 * half_lens).atan());
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
    masthead: Single<&Transform, (With<crate::scene::Masthead>, Without<FollowCamera>)>,
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
        let aspect = window.width() / window.height().max(1.);
        let (next_pose, fov) = if run.screen() == crate::state::Screen::Customise {
            flag_pose(&helm, &sea, &masthead, aspect)
        } else if run.mode == Mode::Title {
            title_pose(&helm, &sea, aspect)
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

    /// The classic Iron Narrows, base C's first gun, and a place with a clear view of her.
    fn harbour_gun() -> (Battle, Vec2, Quarry) {
        let stage = shipvivor_campaign::Stage::IronNarrows;
        let battle = Battle::for_chart(60., &crate::passage::Passage::of(stage).classic());
        let quarry = Quarry::Battery(2, 0);
        let (at, facing) = quarry.frame(&battle).unwrap();
        let mark = quarry.mark(&battle).unwrap();
        let player = (0..24)
            .flat_map(|k| [300., 500., 800.].map(move |d| (k, d)))
            .find_map(|(k, d): (i32, f32)| {
                let way = Vec2::from_angle(k as f32 * TAU / 24.).rotate(facing);
                let player = at + way * d;
                let mut view = ShipView::default();
                view.center_on(player, mark);
                quarry.seen(&view.sight(player, &battle)).then_some(player)
            })
            .expect("a clear view of the gun");
        (battle, player, quarry)
    }

    #[test]
    fn a_harbour_gun_takes_the_snap_the_scope_ride_and_the_pin_like_a_ship() {
        use shipvivor_encounter::Aim;
        let (battle, player, quarry) = harbour_gun();
        let mark = quarry.mark(&battle).unwrap();
        let size = Vec2::splat(600.);
        let mut view = ShipView::default();
        view.center_on(player, mark);
        view.update_contact(player, &battle);
        assert_eq!(view.quarry(&battle), Some(quarry));
        assert!(view.target(&battle).is_none());
        let on = view.sight(player, &battle).point;
        assert!(matches!(view.main_aim(player, &battle, false, false), Aim::Track(p) if p == on));

        // A near miss in the open lens snaps the guns onto her.
        let near = [0.04, -0.04_f32]
            .into_iter()
            .find(|turn| {
                let mut off = ShipView::default();
                off.center_on(player, mark);
                off.bearing += turn;
                Quarry::sighted(&off.sight(player, &battle)).is_none()
            })
            .unwrap();
        view.bearing += near;
        view.update_contact(player, &battle);
        assert_eq!(view.quarry(&battle), Some(quarry));
        assert!(matches!(view.main_aim(player, &battle, false, false), Aim::Track(p) if p == mark));

        // Through the scope the guns follow the reticle, and the sight rides with her
        // while the ship moves.
        view.look(Vec2::ZERO, true, 1.);
        view.update_contact(player, &battle);
        assert_eq!(view.quarry(&battle), Some(quarry));
        let Aim::Track(p) = view.main_aim(player, &battle, false, false) else {
            panic!("the scope tracks the reticle");
        };
        assert_ne!(p, mark);
        let centre = view.viewport_point(player, p, size).unwrap();
        assert!(centre.distance(size * 0.5) < 0.5);
        let before = view.viewport_point(player, mark, size).unwrap();
        let moved = player + (mark.xz() - player).normalize().perp() * 40.;
        view.ride_zero(moved, &battle);
        let after = view.viewport_point(moved, mark, size).unwrap();
        assert!(after.distance(before) < 1., "{before} drifted to {after}");

        // X pins her.
        view.toggle_lock(moved, &battle);
        assert!(view.locked());
        assert_eq!(view.quarry(&battle), Some(quarry));
        assert!(view.tracked_point(&battle).unwrap().distance(mark) < 0.01);
    }
}
