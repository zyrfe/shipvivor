//! The mainland a coastal passage runs along, raised as one terrain over every mainland
//! piece of the chart: a sea cliff at the shore, a ledge above it, and crags and hills
//! behind under snow. Its waterline is the shore the ships and the current know, never
//! beyond it.
use super::foreshore::{self, SNOW, bright, tint};
use crate::scene::Facets;
use crate::state::{Campaign, Encounter, Mode};
use crate::surfaces::{self, sample};
use bevy::prelude::*;
use shipvivor_encounter::{LandmarkKind, MAINLAND_CLIFF, MAINLAND_CREST, mainland_top};

/// Spacing of the terrain's grid in metres.
const SPACING: f32 = 8.;
/// Grid squares along each side of one mesh, so the camera culls what it cannot see.
const CHUNK: i32 = 40;
/// Depth the sea floor reaches at the foot of the cliffs.
const FOOT: f32 = -12.;

pub(crate) struct MainlandPlugin;
impl Plugin for MainlandPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, prepare).add_systems(Update, raise);
    }
}

#[derive(Resource)]
struct Ground(Handle<StandardMaterial>);

/// A piece of the mainland's terrain.
#[derive(Component)]
struct Terrain;

fn prepare(
    mut cmd: Commands,
    mut paint: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    cmd.insert_resource(Ground(paint.add(foreshore::material(&mut images))));
}

/// The chart's mainland pieces, as centre and radius.
pub(super) fn pieces(battle: &shipvivor_encounter::Battle) -> Vec<(Vec2, f32)> {
    battle
        .landmarks()
        .iter()
        .filter(|l| l.kind == LandmarkKind::Mainland)
        .map(|l| (l.body.pos, l.radius))
        .collect()
}

/// Raise the terrain whenever the chart's mainland changes, and show it at sea.
fn raise(
    mut cmd: Commands,
    ground: Res<Ground>,
    run: Res<Campaign>,
    battle: Res<Encounter>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut built: Local<Vec<(Vec2, f32)>>,
    mut terrain: Query<(Entity, &mut Visibility), With<Terrain>>,
) {
    let shown = if matches!(run.mode, Mode::Title | Mode::Trial) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    let land = pieces(&battle);
    if land != *built {
        for (entity, _) in &terrain {
            cmd.entity(entity).despawn();
        }
        for mesh in terrain_meshes(&land) {
            cmd.spawn((
                Terrain,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(ground.0.clone()),
                Transform::IDENTITY,
                shown,
            ));
        }
        *built = land;
        return;
    }
    for (_, mut visibility) in &mut terrain {
        visibility.set_if_neq(shown);
    }
}

/// Height of one piece of the mainland at `inside` metres in from its shore: under water
/// beyond it, a sea cliff, a ledge, a scarp to the high ground and a low dome on top.
pub(super) fn profile(radius: f32, inside: f32) -> f32 {
    if inside < 0. {
        return (-1.5 + 1.1 * inside).max(FOOT);
    }
    let cliff = smooth(inside / 2.5);
    let ledge = MAINLAND_CLIFF + 2. * (inside / (radius * 0.2)).min(1.);
    // The scarp rises where the high ground the lookouts see begins.
    let crest = radius * (1. - MAINLAND_CREST);
    let rise = (radius * 0.1).max(4.);
    let scarp = smooth((inside - (crest - rise)) / rise);
    let top = mainland_top(radius);
    let dome = top + 0.12 * top * (inside / radius).sqrt();
    let shelf = -1.5 + (ledge + 1.5) * cliff;
    shelf + (dome - shelf) * scarp
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

/// Repeatable value noise in [0, 1) over the chart, `scale` metres to a cell.
fn noise(p: Vec2, scale: f32, seed: u32) -> f32 {
    let q = p / scale;
    let i = q.floor();
    let f = q - i;
    let s = f * f * (Vec2::splat(3.) - 2. * f);
    let at = |x: f32, y: f32| sample((i.x + x) as i32 as u32, (i.y + y) as i32 as u32, seed);
    let top = at(0., 0.) + (at(1., 0.) - at(0., 0.)) * s.x;
    let bottom = at(0., 1.) + (at(1., 1.) - at(0., 1.)) * s.x;
    top + (bottom - top) * s.y
}

/// Sharp-crested noise for crags and ridges.
fn ridged(p: Vec2, scale: f32, seed: u32) -> f32 {
    1. - (2. * noise(p, scale, seed) - 1.).abs()
}

/// Height of the mainland at a point: the highest any piece stands there, broken into
/// crags and ridges away from the water.
fn height(land: &[(Vec2, f32)], p: Vec2) -> f32 {
    let base = land
        .iter()
        .filter(|&&(c, r)| p.distance_squared(c) < (r + 12.) * (r + 12.))
        .map(|&(c, r)| profile(r, r - p.distance(c)))
        .fold(FOOT, f32::max);
    // The cliff and the ledge keep their line. Above them the rock breaks up.
    let lift = smooth((base - MAINLAND_CLIFF - 1.) / 10.);
    if lift <= 0. {
        return base;
    }
    let crags = 0.55 * ridged(p, 70., 401) + 0.3 * ridged(p, 26., 402) + 0.15 * noise(p, 9., 403);
    base * (1. + lift * 0.45 * (crags - 0.55))
}

/// The terrain in chunks. Squares wholly under water are left out.
fn terrain_meshes(land: &[(Vec2, f32)]) -> Vec<Mesh> {
    if land.is_empty() {
        return vec![];
    }
    let (lo, hi) = land
        .iter()
        .fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &(c, r)| {
            (lo.min(c - r - SPACING), hi.max(c + r + SPACING))
        });
    let cols = ((hi.x - lo.x) / SPACING).ceil() as i32;
    let rows = ((hi.y - lo.y) / SPACING).ceil() as i32;
    let heights: Vec<f32> = (0..=rows)
        .flat_map(|j| (0..=cols).map(move |i| (i, j)))
        .map(|(i, j)| height(land, lo + Vec2::new(i as f32, j as f32) * SPACING))
        .collect();
    let vertex = |i: i32, j: i32| {
        let h = heights[(j * (cols + 1) + i) as usize];
        let p = lo + Vec2::new(i as f32, j as f32) * SPACING;
        Vec3::new(p.x, h, p.y)
    };
    let mut meshes = vec![];
    for cj in (0..rows).step_by(CHUNK as usize) {
        for ci in (0..cols).step_by(CHUNK as usize) {
            let mut facets = Facets::default();
            let mut any = false;
            for j in cj..(cj + CHUNK).min(rows) {
                for i in ci..(ci + CHUNK).min(cols) {
                    let corners = [
                        vertex(i, j),
                        vertex(i + 1, j),
                        vertex(i + 1, j + 1),
                        vertex(i, j + 1),
                    ];
                    if corners.iter().all(|c| c.y < -1.) {
                        continue;
                    }
                    any = true;
                    // Alternate the diagonal so ridges do not all run one way.
                    let [a, b, c, d] = corners;
                    let tris = if (i + j) % 2 == 0 {
                        [[a, d, b], [b, d, c]]
                    } else {
                        [[a, d, c], [a, c, b]]
                    };
                    for tri in tris {
                        facets.triangle(tri, colour(tri));
                    }
                }
            }
            if any {
                meshes.push(surfaces::weathered_mesh(facets, 5.6));
            }
        }
    }
    meshes
}

/// Wet rock at the waterline, bare rock on the cliffs and scarps, tundra breaking
/// through on the ledges and snow lying on everything level enough to hold it.
fn colour(v: [Vec3; 3]) -> Color {
    let mid = (v[0] + v[1] + v[2]) / 3.;
    let up = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero().y.abs();
    let p = mid.xz();
    let grain = noise(p, 13., 404);
    let patch = noise(p, 47., 405);
    let (rock, snow) = (0.86 + 0.22 * grain, 0.93 + 0.08 * grain);
    if mid.y < 1.1 + 0.5 * patch {
        tint([84, 94, 98], rock)
    } else if up < 0.5 {
        tint([96, 102, 101], rock)
    } else if up < 0.74 || (mid.y < MAINLAND_CLIFF + 1.5 && patch < 0.35) {
        tint([150, 152, 149], rock)
    } else if mid.y < MAINLAND_CLIFF + 4. && patch > 0.7 {
        tint([128, 124, 98], rock)
    } else {
        bright(SNOW, snow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waterline_never_stands_beyond_the_charted_shore() {
        for radius in [14., 40., 120., 400.] {
            // Out from the shore the sea floor falls away; just in, the cliff stands up.
            assert!(profile(radius, -0.01) < 0.);
            assert!(profile(radius, -8.) <= -9.);
            assert!(profile(radius, 3.) >= MAINLAND_CLIFF - 0.01);
            // The high ground stands where the lookouts' sight is blocked.
            let crest = radius * (1. - MAINLAND_CREST);
            assert!(
                profile(radius, crest + 1.) > mainland_top(radius) * 0.85,
                "{radius}"
            );
        }
        let land = [(Vec2::ZERO, 150.), (Vec2::new(180., 0.), 60.)];
        for k in 0..72 {
            let out = Vec2::from_angle(k as f32 * 0.0873);
            for (c, r) in land {
                let p = c + out * (r + 0.5);
                if land.iter().all(|&(o, s)| p.distance(o) > s) {
                    assert!(height(&land, p) < 0., "{p}");
                }
            }
        }
    }

    #[test]
    fn a_coast_is_raised_in_chunks_with_snow_on_its_heights() {
        let land: Vec<(Vec2, f32)> = (0..8)
            .map(|k| (Vec2::new(k as f32 * 70., 0.), 110.))
            .chain([(Vec2::new(240., -380.), 360.)])
            .collect();
        let meshes = terrain_meshes(&land);
        assert!(meshes.len() >= 4);
        let highest = meshes
            .iter()
            .flat_map(|m| {
                m.attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .as_float3()
                    .unwrap()
                    .iter()
                    .map(|p| p[1])
            })
            .fold(0., f32::max);
        assert!(highest > 40. && highest < 100., "{highest}");
    }
}
