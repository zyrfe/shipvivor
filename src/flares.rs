//! Parachute flares over a base that has raised the alarm.
//!
//! A salvo goes up from the yard towards where the ship was last seen, bursts at the
//! top of its climb and comes down slowly under canopies, swinging as it drifts on the
//! wind. Another follows for as long as the alarm is up.
use crate::{
    alert::BaseAlerts,
    scene::Facets,
    state::{Campaign, Encounter, Mode, PlayerHelm, Screen},
};
use bevy::{light::NotShadowCaster, prelude::*};
use std::f32::consts::TAU;

/// Flares in a salvo, the seconds between them and the seconds before the next salvo.
const SALVO: usize = 3;
const BETWEEN: f32 = 0.45;
const AGAIN: f32 = 16.;
/// Seconds a flare climbs, and the metres it gains.
const CLIMB: f32 = 2.2;
const APEX: f32 = 60.;
/// A salvo is thrown this share of the way to the ship, between these distances, and
/// spread this many radians to either side.
const THROW: (f32, f32, f32) = (0.4, 45., 150.);
const FAN: f32 = 0.5;
/// Metres per second a flare sinks under its canopy, the seconds it burns and the
/// seconds it takes to gutter out.
const SINK: f32 = 3.4;
const BURN: f32 = 15.;
const GUTTER: f32 = 1.5;
/// The wind it drifts on, in metres per second, how hard it swings from side to side
/// and how many radians of that swing go by each second.
const WIND: Vec2 = Vec2::new(1.1, 0.6);
const SWING: (f32, f32) = (1.3, 2.6);
/// Light it gives at its brightest, in lumens, and the metres that reaches.
const GLARE: (f32, f32) = (3e7, 160.);

/// One flare in the air.
pub struct Flare {
    id: u64,
    pub position: Vec3,
    velocity: Vec3,
    /// Seconds since it left the ground, and where in its swing it began.
    pub age: f32,
    swing: f32,
}
impl Flare {
    /// How brightly it burns, from 0 to 1. On the way up it is only a spark.
    pub fn glow(&self) -> f32 {
        let burnt = self.age - CLIMB;
        if burnt < 0. {
            0.12
        } else {
            (burnt / 0.25).min(1.) * ((BURN + GUTTER - burnt) / GUTTER).clamp(0., 1.)
        }
    }
    /// How far its canopy has opened, from 0 to 1.
    fn opened(&self) -> f32 {
        ((self.age - CLIMB) / 0.5).clamp(0., 1.)
    }
    /// How it leans as it swings: metres per second across the wind, east and south.
    fn lean(&self) -> Vec2 {
        let turn = SWING.1 * self.age + self.swing;
        SWING.0 * self.opened() * Vec2::new(turn.cos(), (0.8 * turn).sin())
    }
}

/// Every flare in the air, and what each base's battery is doing.
#[derive(Resource, Default)]
pub struct Flares {
    live: Vec<Flare>,
    batteries: [Battery; 3],
    seen: f32,
    made: u64,
}
impl Flares {
    pub fn live(&self) -> &[Flare] {
        &self.live
    }
}
#[derive(Clone, Copy, Default)]
struct Battery {
    alerted: bool,
    /// Flares still to go in this salvo, when the next of them goes, and when the next
    /// salvo is due.
    left: usize,
    next: f32,
    again: f32,
}

/// Fire and fly the flares. Runs each tick after the alarms have been reckoned.
pub fn fly(
    battle: Res<Encounter>,
    alerts: Res<BaseAlerts>,
    helm: Res<PlayerHelm>,
    clock: Res<Time<Fixed>>,
    mut flares: ResMut<Flares>,
) {
    let (now, dt) = (battle.at_sea(), clock.delta_secs());
    if now < flares.seen {
        // A new voyage, with nothing in the air.
        *flares = Flares::default();
    }
    flares.seen = now;
    let flares = &mut *flares;
    for (base, (port, battery)) in battle.ports().iter().zip(&mut flares.batteries).enumerate() {
        let alerted = alerts.raised(base, &battle);
        if alerted && (!battery.alerted || now >= battery.again) {
            (battery.left, battery.next, battery.again) = (SALVO, now, now + AGAIN);
        }
        if !alerted {
            battery.left = 0;
        }
        battery.alerted = alerted;
        while battery.left > 0 && now >= battery.next {
            let (yard, top) = (port.yard_position(), port.yard_top());
            let away = helm.position - yard;
            let reach = (THROW.0 * away.length()).clamp(THROW.1, THROW.2);
            // Each goes a little wide of the last, and none quite where it was meant.
            let nth = (SALVO - battery.left) as f32;
            let stray = (flares.made as f32 * 2.4).sin();
            let wide = FAN * (nth - 0.5 * (SALVO - 1) as f32) + 0.12 * stray;
            let along = Vec2::from_angle(wide).rotate(away.normalize_or(Vec2::X));
            let level = along * reach * (1. + 0.1 * stray) / CLIMB;
            flares.live.push(Flare {
                id: flares.made,
                position: Vec3::new(yard.x, top + 1., yard.y),
                velocity: Vec3::new(level.x, 2. * APEX / CLIMB, level.y),
                age: 0.,
                swing: TAU * (flares.made as f32 * 0.618).fract(),
            });
            flares.made += 1;
            battery.left -= 1;
            battery.next += BETWEEN;
        }
    }
    for flare in &mut flares.live {
        flare.age += dt;
        if flare.age < CLIMB {
            // It is thrown, and slows all the way to the top.
            flare.velocity.y -= 2. * APEX / (CLIMB * CLIMB) * dt;
        } else {
            // Under its canopy it sinks steadily and goes where the wind takes it.
            let drift = WIND + flare.lean();
            let ease = |secs: f32| 1. - (-dt / secs).exp();
            // The canopy snatches it out of its flight as it opens.
            flare.velocity.x += (drift.x - flare.velocity.x) * ease(0.3);
            flare.velocity.z += (drift.y - flare.velocity.z) * ease(0.3);
            flare.velocity.y += (-SINK - flare.velocity.y) * ease(0.5);
        }
        flare.position += flare.velocity * dt;
    }
    flares
        .live
        .retain(|flare| flare.age < CLIMB + BURN + GUTTER && flare.position.y > 0.3);
}

/// The drawn flare, and the parts of it that change as it burns.
#[derive(Component)]
pub struct FlareModel {
    id: u64,
    star: Entity,
    canopy: Entity,
    light: Entity,
}

/// Meshes and paint, made once.
#[derive(Resource)]
pub struct FlareKit {
    star: Handle<Mesh>,
    canopy: Handle<Mesh>,
    fire: Handle<StandardMaterial>,
    glare: [Handle<StandardMaterial>; 2],
    silk: Handle<StandardMaterial>,
}

/// Radius of the star when it burns full, and of the canopy. The canopy's rim hangs
/// this far above the star and its crown that much higher again.
const STAR: f32 = 0.8;
/// The glare about the star, drawn as two shells of light: how many times the star's
/// size each is, and how much it adds to whatever is seen through it.
const HALO: [(f32, f32); 2] = [(2.2, 0.45), (4.2, 0.16)];
const CANOPY: (f32, f32, f32) = (2.4, 3.3, 1.3);

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const SILK: Color = Color::srgb(0.93, 0.9, 0.82);
    const RED: Color = Color::srgb(0.76, 0.17, 0.12);
    const INK: Color = Color::srgb(0.15, 0.22, 0.24);
    // Eight gores, white and red by turns, each in two panels from rim to crown.
    let mut canopy = Facets::default();
    let (radius, rim, rise) = CANOPY;
    let ring = |gore: usize, up: f32| {
        let (sin, cos) = (gore as f32 * TAU / 8.).sin_cos();
        let out = radius * (1. - up * up).sqrt();
        Vec3::new(out * cos, rim + rise * up, out * sin)
    };
    for gore in 0..8 {
        let paint = if gore % 2 == 0 { SILK } else { RED };
        for (low, high) in [(0., 0.6), (0.6, 0.98)] {
            canopy.quad(
                [
                    ring(gore, low),
                    ring(gore, high),
                    ring(gore + 1, high),
                    ring(gore + 1, low),
                ],
                paint,
            );
        }
        if gore % 2 == 0 {
            canopy.tube(Vec3::Y * 0.5, ring(gore, 0.), 0.035, 0.035, INK);
        }
    }
    commands.insert_resource(FlareKit {
        star: meshes.add(Sphere::new(STAR)),
        canopy: meshes.add(canopy.finish()),
        // Far brighter than anything the sun lights, so it stays white-hot by day.
        fire: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(14., 11., 6.),
            unlit: true,
            ..default()
        }),
        glare: HALO.map(|(_, adds)| {
            materials.add(StandardMaterial {
                base_color: Color::linear_rgba(2.6, 1.9, 0.9, adds),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })
        }),
        // Seen from below as well as from above.
        silk: materials.add(StandardMaterial {
            cull_mode: None,
            double_sided: true,
            perceptual_roughness: 0.9,
            ..default()
        }),
    });
}

/// Draw each flare where it is, and take away those that have gone out.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut commands: Commands,
    run: Res<Campaign>,
    flares: Res<Flares>,
    kit: Res<FlareKit>,
    time: Res<Time>,
    mut models: Query<(Entity, &FlareModel, &mut Transform)>,
    mut parts: Query<&mut Transform, Without<FlareModel>>,
    mut lights: Query<&mut PointLight>,
) {
    let at_sea = !matches!(run.mode, Mode::Title | Mode::Trial)
        && !matches!(run.screen(), Screen::Drydock(_) | Screen::Chart);
    let live = if at_sea { flares.live() } else { &[] };
    let mut drawn = Vec::new();
    for (entity, model, mut pose) in &mut models {
        let Some(flare) = live.iter().find(|flare| flare.id == model.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        drawn.push(model.id);
        // The star hangs under the canopy and the whole swings together.
        let lean = flare.lean();
        pose.translation = flare.position;
        pose.rotation =
            Quat::from_rotation_z(-0.16 * lean.x) * Quat::from_rotation_x(0.16 * lean.y);
        // A flame is never steady.
        let flicker = 0.88 + 0.12 * (time.elapsed_secs() * 31. + flare.id as f32).sin();
        let glow = flare.glow() * flicker;
        if let Ok(mut star) = parts.get_mut(model.star) {
            star.scale = Vec3::splat(0.25 + 0.75 * glow);
        }
        if let Ok(mut canopy) = parts.get_mut(model.canopy) {
            canopy.scale = Vec3::splat(flare.opened().max(0.001));
        }
        if let Ok(mut light) = lights.get_mut(model.light) {
            light.intensity = GLARE.0 * glow;
        }
    }
    for flare in live.iter().filter(|flare| !drawn.contains(&flare.id)) {
        let star = commands
            .spawn((
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(kit.fire.clone()),
                Transform::from_scale(Vec3::splat(0.25)),
                NotShadowCaster,
            ))
            .id();
        // The glare grows and gutters with the star it hangs about.
        for ((size, _), glare) in HALO.into_iter().zip(&kit.glare) {
            commands.spawn((
                Mesh3d(kit.star.clone()),
                MeshMaterial3d(glare.clone()),
                Transform::from_scale(Vec3::splat(size)),
                NotShadowCaster,
                ChildOf(star),
            ));
        }
        let canopy = commands
            .spawn((
                Mesh3d(kit.canopy.clone()),
                MeshMaterial3d(kit.silk.clone()),
                Transform::from_scale(Vec3::splat(0.001)),
                NotShadowCaster,
            ))
            .id();
        let light = commands
            .spawn((
                PointLight {
                    color: Color::srgb(1., 0.9, 0.7),
                    intensity: 0.,
                    range: GLARE.1,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::default(),
            ))
            .id();
        commands
            .spawn((
                FlareModel {
                    id: flare.id,
                    star,
                    canopy,
                    light,
                },
                Transform::from_translation(flare.position),
                Visibility::default(),
            ))
            .add_children(&[star, canopy, light]);
    }
}
