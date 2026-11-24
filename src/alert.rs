//! Which bases have raised the alarm. A base is alerted while it has word of the ship
//! or is being hit, and for a while after. Its siren and its flares both go by this.
use crate::state::Encounter;
use bevy::prelude::*;
use shipvivor_encounter::{EffectKind, ISLAND_RADIUS, YardState};

/// Seconds a base stays alerted after it last had word of the ship or was hit.
const ALARM: f32 = 12.;
/// A burst within this many metres of the middle of a base has landed on it.
const ASHORE: f32 = 1.5 * ISLAND_RADIUS;

/// When each base's alarm comes down, by the battle's clock.
#[derive(Resource, Default)]
pub struct BaseAlerts {
    seen: f32,
    until: [f32; 3],
}
impl BaseAlerts {
    /// Whether this base has its alarm up. A base whose yard is ruined has nobody
    /// left to raise one.
    pub fn raised(&self, base: usize, battle: &Encounter) -> bool {
        battle.at_sea() < self.until[base] && battle.ports()[base].yard_state() != YardState::Ruined
    }
}

/// Runs each tick after the battle steps, and before anything that goes by the alarm.
pub fn watch(battle: Res<Encounter>, mut alerts: ResMut<BaseAlerts>) {
    let now = battle.at_sea();
    if now < alerts.seen {
        // The clock has gone back, so this is a new voyage and no alarm is up yet.
        alerts.until = [0.; 3];
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
