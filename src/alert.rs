//! Which bases have raised the alarm. A base is alerted while it has word of the ship
//! or is being hit, and for a while after. Its siren and its flares both go by this.
use crate::state::Encounter;
use bevy::prelude::*;
use shipvivor_encounter::{EffectKind, ISLAND_RADIUS};

/// Seconds a base stays alerted after it last had word of the ship or was hit.
const ALARM: f32 = 12.;
/// A burst within this many metres of the middle of a base has landed on it.
const ASHORE: f32 = 1.5 * ISLAND_RADIUS;

/// When each base's alarm comes down, by the battle's clock.
#[derive(Resource, Default)]
pub struct BaseAlerts {
    seen: f32,
    until: Vec<f32>,
}
impl BaseAlerts {
    /// Whether this base has its alarm up. A base with nothing left standing, not even a
    /// gun, has nobody left to raise one.
    pub fn raised(&self, base: usize, battle: &Encounter) -> bool {
        self.until
            .get(base)
            .is_some_and(|&until| battle.at_sea() < until)
            && battle.ports()[base].lookout().is_some()
    }
}

/// Runs each tick after the battle steps, and before anything that goes by the alarm.
pub fn watch(battle: Res<Encounter>, mut alerts: ResMut<BaseAlerts>) {
    let now = battle.at_sea();
    if now < alerts.seen || alerts.until.len() != battle.ports().len() {
        // The clock has gone back, so this is a new voyage and no alarm is up yet.
        alerts.until = vec![0.; battle.ports().len()];
    }
    alerts.seen = now;
    for (port, until) in battle.ports().iter().zip(&mut alerts.until) {
        let struck = battle.effects().iter().any(|fx| {
            let burst = matches!(
                fx.kind,
                EffectKind::Burst | EffectKind::Demolition { .. } | EffectKind::MountLost(_)
            );
            burst && fx.age == 0. && fx.body.pos.distance(port.body.pos) < ASHORE
        });
        if port.has_contact || struck {
            *until = now + ALARM;
        }
    }
}
