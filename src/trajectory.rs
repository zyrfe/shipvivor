//! A faint line along each main battery round's flight while the sight is scoped, for the
//! mounts laid on the mark. Mounts still training round or short of the mark draw none,
//! and the gun rings say which they are.
use crate::{
    camera::{FollowCamera, ShipView},
    gun_sight::out_of_reach,
    state::{Campaign, Encounter, Guns, PlayerHelm, Screen},
};
use bevy::{
    asset::RenderAssetUsages, light::NotShadowCaster, prelude::*,
    render::render_resource::PrimitiveTopology,
};
use shipvivor_encounter::{Aim, Battle, MainMount, MainShell, MountStatus, Station, Trajectory};

/// Straight pieces in each drawn flight.
const ARC_STEPS: usize = 64;
/// Chords used to find what the flight strikes first.
const PROBE_STEPS: usize = 12;
/// Metres from the gunhouse over which a flight fades in, so the deck stays clear.
const FADE_IN: f32 = 24.;
/// Half the width of a flight line, in screen pixels: the next gun to fire, then the rest.
const NEXT_HALF_WIDTH: f32 = 1.1;
const OTHER_HALF_WIDTH: f32 = 0.8;
const NEXT_ALPHA: f32 = 0.27;
const OTHER_ALPHA: f32 = 0.14;
/// Lens magnification over which the flights fade in as the scope narrows.
const SCOPE_SHOWS: f32 = 0.5;
const SCOPE_FULL: f32 = 1.;

#[derive(Component)]
pub struct FlightLines;

fn flight_ink() -> Color {
    Color::srgb_u8(226, 244, 236)
}
/// A soft dark edge under each line keeps it readable over glinting water and pale sky.
fn shadow_ink() -> Color {
    Color::srgb_u8(10, 26, 32)
}
const SHADOW_WIDTH: f32 = 2.6;
const SHADOW_ALPHA: f32 = 0.3;

pub fn setup(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    cmd.spawn((
        FlightLines,
        Mesh3d(meshes.add(Strokes::default().finish())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));
}

/// Screen-facing strips of constant on-screen width, coloured per vertex.
#[derive(Default)]
struct Strokes {
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}
impl Strokes {
    /// A line with a soft dark edge under it.
    fn edged(&mut self, points: &[(Vec3, f32)], eye: Vec3, per_pixel: f32, half: f32, ink: Color) {
        let shade: Vec<(Vec3, f32)> = points.iter().map(|&(p, a)| (p, a * SHADOW_ALPHA)).collect();
        self.line(&shade, eye, per_pixel, half * SHADOW_WIDTH, shadow_ink());
        self.line(points, eye, per_pixel, half, ink);
    }
    /// A polyline `half_width` screen pixels either side, where `per_pixel` is the world
    /// size of one pixel one metre from the eye.
    fn line(&mut self, points: &[(Vec3, f32)], eye: Vec3, per_pixel: f32, half: f32, ink: Color) {
        let ink = ink.to_linear();
        let colour = |alpha: f32| ink.with_alpha(ink.alpha * alpha).to_f32_array();
        for pair in points.windows(2) {
            let [(a, fa), (b, fb)] = [pair[0], pair[1]];
            let along = b - a;
            if along.length_squared() < 1e-8 || (fa <= 0. && fb <= 0.) {
                continue;
            }
            let side = |p: Vec3| {
                along.cross(eye - p).normalize_or_zero() * half * per_pixel * p.distance(eye)
            };
            let (sa, sb) = (side(a), side(b));
            let corners = [a - sa, a + sa, b + sb, b - sb];
            let alphas = [fa, fa, fb, fb];
            for i in [0, 1, 2, 0, 2, 3] {
                self.positions.push(corners[i].to_array());
                self.colors.push(colour(alphas[i]));
            }
        }
    }
    fn finish(self) -> Mesh {
        let normals = vec![[0., 1., 0.]; self.positions.len()];
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
    }
}

/// Seconds into the flight when the round strikes rock, ice, a building or a ship, or
/// comes down on the water.
fn flight_end(path: &Trajectory, battle: &Battle) -> Option<f32> {
    let water = path.descends_to(0.)?;
    let mut before = path.point(0.);
    for step in 1..=PROBE_STEPS {
        let t = water * step as f32 / PROBE_STEPS as f32;
        let mut next = path.point(t);
        if step == PROBE_STEPS {
            next.y = 0.;
        }
        let chord = next - before;
        if let Some(ray) = chord.try_normalize()
            && chord.xz().length() > 0.01
        {
            let sea = Vec3::new(next.x, 0., next.z);
            let struck = battle.sight(before, ray, next.xz()).point;
            if struck.distance_squared(sea) > 1e-4 {
                let share = before.xz().distance(struck.xz()) / chord.xz().length();
                let t0 = water * (step - 1) as f32 / PROBE_STEPS as f32;
                return Some(t0 + (t - t0) * share.clamp(0., 1.));
            }
        }
        before = next;
    }
    Some(water)
}

/// The mounts that draw a flight: laid on the mark and able to reach it.
pub(crate) fn laid<'a>(
    battle: &'a Battle,
    helm: &PlayerHelm,
    aim: Vec3,
    round: MainShell,
) -> Vec<(Station, &'a MainMount)> {
    Station::SLOTS
        .iter()
        .copied()
        .zip(battle.main_mounts())
        .filter(|(_, mount)| {
            matches!(mount.status(), MountStatus::Ready | MountStatus::Loading)
                && !out_of_reach(mount, helm, aim, round)
        })
        .collect()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub fn draw(
    run: Res<Campaign>,
    sight: (Res<PlayerHelm>, Res<ShipView>),
    battle: Res<Encounter>,
    guns: Res<Guns>,
    camera: Single<(&Camera, &Transform, &Projection), With<FollowCamera>>,
    mut lines: Single<(&Mesh3d, &mut Visibility), With<FlightLines>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (helm, view) = sight;
    let (handle, shown) = &mut *lines;
    let aim = match view.main_aim(helm.position, &battle, guns.firing, guns.nearest) {
        Aim::Sight(p) | Aim::Track(p) => Some(p),
        _ => None,
    };
    let (lens, pose, projection) = *camera;
    let fov = match projection {
        Projection::Perspective(lens) => lens.fov,
        _ => 0.,
    };
    let tall = lens.logical_viewport_size().map_or(0., |size| size.y);
    // Flights draw only through the scope, fading in with the lens.
    let scope = smoothstep(SCOPE_SHOWS, SCOPE_FULL, view.zoom);
    let Some(aim) = aim.filter(|_| {
        run.screen() == Screen::Sailing
            && !guns.torpedo_mode
            && fov > 0.
            && tall > 1.
            && scope > 0.01
    }) else {
        shown.set_if_neq(Visibility::Hidden);
        return;
    };
    let eye = pose.translation;
    let per_pixel = 2. * (fov * 0.5).tan() / tall;
    let round = run.fittings.round();
    // The one that fires next draws a little stronger, or while none is ready, the first
    // that will be.
    let bearing = laid(&battle, &helm, aim, round);
    let next = battle
        .next_barrel()
        .map(|(station, _)| station)
        .or_else(|| bearing.first().map(|(station, _)| *station));
    let mut strokes = Strokes::default();
    for (station, mount) in bearing {
        let Some(path) = mount.trajectory(helm.position, helm.heading, aim, round) else {
            continue;
        };
        let origin = path.point(0.);
        let Some(end) = flight_end(&path, &battle) else {
            continue;
        };
        let (half, alpha) = if Some(station) == next {
            (NEXT_HALF_WIDTH, NEXT_ALPHA * scope)
        } else {
            (OTHER_HALF_WIDTH, OTHER_ALPHA * scope)
        };
        let points: Vec<(Vec3, f32)> = (0..=ARC_STEPS)
            .map(|i| {
                let p = path.point(end * i as f32 / ARC_STEPS as f32);
                let out = origin.xz().distance(p.xz());
                (p, alpha * (out / FADE_IN).clamp(0., 1.))
            })
            .collect();
        strokes.edged(&points, eye, per_pixel, half, flight_ink());
    }

    if strokes.positions.is_empty() {
        shown.set_if_neq(Visibility::Hidden);
        return;
    }
    shown.set_if_neq(Visibility::Visible);
    if let Some(mut mesh) = meshes.get_mut(&handle.0) {
        *mesh = strokes.finish();
    }
}
