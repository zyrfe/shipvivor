//! The moving machinery and coast of the two late-game works. The port owns their
//! damage, deployment clocks and geography so a paused battle also freezes the yard.
use super::{Actor, Shape, foreshore, harbour_models, pose};
use bevy::prelude::*;
use harbour_models::IndustrialPart;
use shipvivor_encounter::{Body, IndustrialYard, Port};
use std::collections::HashMap;

pub(super) fn meshes(
    meshes: &mut Assets<Mesh>,
) -> HashMap<(IndustrialYard, IndustrialPart), Handle<Mesh>> {
    let mut assets = HashMap::new();
    for kind in [IndustrialYard::RedShift, IndustrialYard::BellWorks] {
        for part in IndustrialPart::ALL {
            assets.insert(
                (kind, part),
                meshes.add(harbour_models::industrial(kind, part)),
            );
        }
    }
    assets
}

pub(super) fn draw(
    wanted: &mut HashMap<Actor, Transform>,
    port: &Port,
    kind: IndustrialYard,
    now: f32,
) {
    let id = port.body.id;
    let key = |part, i| Actor(id, Shape::Industrial(kind, part, i));
    let at = |pos, height| pose(&Body { pos, ..port.body }, height);
    let hall = at(port.industrial_center(), 3.2);
    let collapse = port.industrial_collapse();
    let progress = (collapse.unwrap_or(0.) / 2.8).clamp(0., 1.);
    let stage = (progress * 3.9) as u8;
    wanted.insert(key(IndustrialPart::Hall(stage), 0), hall);
    wanted.insert(key(IndustrialPart::Heat(stage), 0), hall);
    let machine = harbour_models::machine_pose(kind, progress, now);
    wanted.insert(key(IndustrialPart::Machine, 0), hall.mul_transform(machine));
    if kind == IndustrialYard::RedShift && (0.12..0.93).contains(&progress) {
        let chute = harbour_models::ore_fall_pose(progress);
        wanted.insert(key(IndustrialPart::Discharge, 0), hall.mul_transform(chute));
    }
    if let Some(elapsed) = collapse {
        draw_collapse(wanted, id, kind, hall, elapsed);
    } else {
        let chimney = hall.transform_point(harbour_models::industrial_chimney(kind));
        wanted.insert(
            Actor(id, Shape::Exhaust(0)),
            Transform::from_translation(chimney),
        );
    }
    for (i, factory) in port.factories().iter().enumerate() {
        let ruined = !factory.live();
        let part = if i == IndustrialYard::FUEL_TANK {
            IndustrialPart::Fuel(ruined)
        } else {
            IndustrialPart::Governor(ruined)
        };
        let foot = at(port.factory_position(i), 3.2);
        wanted.insert(key(part, i as u8), foot);
        if part == IndustrialPart::Governor(false) {
            wanted.insert(key(IndustrialPart::GaugeLight, i as u8), foot);
        }
    }
    for i in 0..port.docks().len() {
        wanted.insert(
            key(IndustrialPart::Quay, i as u8),
            at(port.dock_position(i), 3.2),
        );
    }
    for (i, (center, radius, height)) in port.industrial_rocks().enumerate() {
        let scale = foreshore::industrial_rock_scale(radius, height);
        wanted.insert(
            Actor(id, Shape::IndustrialRock(i as u8)),
            at(center, 0.).with_scale(scale),
        );
    }
}

/// The ore streams out along the folding roof bays. The forging hammer instead
/// lands one final blow, blowing steam out of both sides of its bed.
fn draw_collapse(
    wanted: &mut HashMap<Actor, Transform>,
    id: u64,
    kind: IndustrialYard,
    hall: Transform,
    elapsed: f32,
) {
    if elapsed > 4.5 {
        return;
    }
    match kind {
        IndustrialYard::RedShift => {
            for i in 0..24 {
                let column = f32::from(i % 6);
                let t = elapsed - 0.30 - column * 0.17 - f32::from(i / 6) * 0.18;
                if !(0.0..1.1).contains(&t) {
                    continue;
                }
                let load = Transform::from_xyz(
                    -17. + column * 3.6 + t * 1.5,
                    17.4 - column * 0.8 - 11. * t * t,
                    -3.9 - 5.4 * t + (f32::from(i) * 1.7).sin(),
                )
                .with_rotation(Quat::from_rotation_z(t * 3. + column))
                .with_scale(Vec3::splat(1.4 + 0.4 * (f32::from(i) * 2.).sin()));
                wanted.insert(Actor(id, Shape::Debris(i)), hall.mul_transform(load));
            }
        }
        IndustrialYard::BellWorks => {
            for i in 0..8 {
                let t = elapsed - 1.38 - f32::from(i / 2) * 0.18;
                if !(0.0..2.2).contains(&t) {
                    continue;
                }
                let sign = if i % 2 == 0 { -1. } else { 1. };
                let puff = Transform::from_xyz(sign * (4. + 7. * t), 1.7 + t * t * 1.4, -4. + t)
                    .with_scale(Vec3::splat(2.7 * (std::f32::consts::PI * t / 2.2).sin()));
                wanted.insert(Actor(id, Shape::Soot(i)), hall.mul_transform(puff));
            }
        }
    }
}

/// The shield slides up before the gun carriage runs out on its rails. The gun's
/// final centre and its muzzle coincide with the encounter's live emplacement.
pub(super) fn draw_casemate(
    wanted: &mut HashMap<Actor, Transform>,
    kind: IndustrialYard,
    body: &Body,
    index: u8,
    elevation: f32,
    deployment: f32,
) -> Vec3 {
    let frame = pose(body, elevation);
    let key = |part| Actor(body.id, Shape::Industrial(kind, part, index));
    let lift = (deployment / 0.44).clamp(0., 1.);
    let travel = ((deployment - 0.34) / 0.66).clamp(0., 1.);
    wanted.insert(key(IndustrialPart::Casemate), frame);
    wanted.insert(
        key(IndustrialPart::Shutter),
        frame.mul_transform(
            Transform::from_xyz(0., 12. * lift, 0.).with_scale(Vec3::new(1., 1. - 0.92 * lift, 1.)),
        ),
    );
    frame.rotation * Vec3::Z * (18. * (1. - travel))
}

/// The reservoir's upper plate tumbles free while its fuel burns below.
pub(super) fn draw_tank_lid(
    wanted: &mut HashMap<Actor, Transform>,
    ports: &[Port],
    effect: &shipvivor_encounter::Effect,
) {
    let age = effect.age;
    let height = 14.7 + 23. * age - 6.4 * age * age;
    if height <= 1. {
        return;
    }
    let Some(port) = ports.iter().find(|p| {
        p.industrial_yard().is_some()
            && p.factory_position(IndustrialYard::FUEL_TANK)
                .distance_squared(effect.body.pos)
                < 1.
    }) else {
        return;
    };
    let tank = Body {
        pos: effect.body.pos,
        ..port.body
    };
    let mut lid = pose(&tank, height);
    lid.translation += lid.rotation * Vec3::new(3.4, 0., -4.6) * age;
    lid.rotate_local_z(age * 3.2);
    lid.rotate_local_x(age * 0.9);
    wanted.insert(Actor(effect.body.id, Shape::TankLid), lid);
}
