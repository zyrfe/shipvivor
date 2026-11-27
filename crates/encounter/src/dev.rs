//! Changes made to a battle by hand while it is under way or paused, for tuning a fight
//! from the dev tools (the in-game MCP server): enemy ships placed, moved, taken away,
//! rebuilt as another class, regunned in another weight and damaged. Nothing in play
//! calls these, and the simulator never does.
//!
//! Every ship keeps her body id through these changes but a placed one, so her squadron,
//! the captain's mark and the view's models of her all follow her. The view keys a ship's
//! models by her class and barrels as well, so a ship rebuilt or regunned is redrawn.
use super::{Battle, EnemyClass, Raider, Vec2, Weight, body};

impl Battle {
    /// Place a ship of `class` at `pos`, heading `heading` (radians, the chart's
    /// convention), gunned in `weight` or, without one, as the sector would gun her next.
    /// She sails on her own, in no squadron. Her new body id, or why she can't go there.
    pub fn dev_spawn(
        &mut self,
        class: EnemyClass,
        pos: Vec2,
        heading: f32,
        weight: Option<Weight>,
    ) -> Result<u64, String> {
        self.dev_check_water(class.profile().hull.radius, pos)?;
        let mut raider = Raider::new(body(&mut self.serial, pos, heading), class);
        match (weight, &mut self.muster) {
            (Some(weight), _) => raider.reweigh(weight),
            (None, Some(muster)) => raider.reweigh(muster.draw(class)),
            (None, None) => {}
        }
        let id = raider.body.id;
        self.raiders.push(raider);
        Ok(id)
    }

    /// Put ship `id` at `pos`, on `heading` if one is given, with no way on. A ship at
    /// anchor weighs and sails as a warship does.
    pub fn dev_move(&mut self, id: u64, pos: Vec2, heading: Option<f32>) -> Result<(), String> {
        let class = self.dev_ship(id)?.class;
        self.dev_check_water(class.profile().hull.radius, pos)?;
        let r = self.dev_ship_mut(id)?;
        r.body.pos = pos;
        if let Some(heading) = heading {
            r.body.heading = heading;
        }
        r.way = Vec2::ZERO;
        r.knock = Vec2::ZERO;
        r.order = None;
        if r.moored_at().is_some() {
            r.cast_off();
        }
        Ok(())
    }

    /// Take ship `id` out of the battle. She is not sunk: no wreck, no stores, no credit.
    pub fn dev_remove(&mut self, id: u64) -> Result<EnemyClass, String> {
        let index = (self.raiders.iter())
            .position(|r| r.body.id == id)
            .ok_or_else(|| no_ship(id))?;
        let r = self.raiders.remove(index);
        if self.marked == Some(id) {
            self.marked = None;
        }
        // Her squadron lets her go on its next tick, as it does a ship sunk.
        Ok(r.class)
    }

    /// Rebuild ship `id` as a fresh ship of `class` where she lies, keeping her id, her
    /// heading, her way and her squadron. Gunned in `weight`, or without one in the
    /// weight she had, or for a class she couldn't have had, as the sector would gun her.
    /// The same class and weight mends her completely.
    pub fn dev_replace(
        &mut self,
        id: u64,
        class: EnemyClass,
        weight: Option<Weight>,
    ) -> Result<(), String> {
        let old = self.dev_ship(id)?;
        let (pose, way, contact, sighting, moored) = (
            old.body,
            old.way,
            old.has_contact,
            old.last_sighting,
            old.moored_at(),
        );
        let kept = (old.class == class || Weight::row(old.class).is_some()).then(|| old.weight());
        self.dev_check_water(class.profile().hull.radius, pose.pos)?;
        let mut fresh = Raider::new(pose, class);
        match (weight.or(kept), &mut self.muster) {
            (Some(weight), _) => fresh.reweigh(weight),
            (None, Some(muster)) => fresh.reweigh(muster.draw(class)),
            (None, None) => {}
        }
        fresh.way = way;
        fresh.has_contact = contact;
        fresh.last_sighting = sighting;
        if let Some(base) = moored {
            fresh.anchor(base);
        }
        *self.dev_ship_mut(id)? = fresh;
        Ok(())
    }

    /// Regun ship `id` in `weight`: barrels in every main turret. A class that comes in
    /// no weights (merchants, torpedo boats, the bosses) refuses.
    pub fn dev_reweigh(&mut self, id: u64, weight: Weight) -> Result<(), String> {
        let r = self.dev_ship_mut(id)?;
        if Weight::row(r.class).is_none() {
            return Err(format!("a {:?} comes in no weights", r.class));
        }
        r.reweigh(weight);
        Ok(())
    }

    /// Strike ship `id`'s hull for `points`, spread evenly along her length. A ship
    /// struck below water sinks on the next step of the battle, and counts as sunk.
    /// Her hull's share left, from 0 to 1.
    pub fn dev_damage(&mut self, id: u64, points: f32) -> Result<f32, String> {
        let r = self.dev_ship_mut(id)?;
        let hull = r.class.profile().hull;
        let length = hull.bow + hull.stern;
        const STRIKES: usize = 5;
        for i in 0..STRIKES {
            let aft = length * (i as f32 + 0.5) / STRIKES as f32;
            r.ship.strike_hull(aft, points / STRIKES as f32);
        }
        Ok(r.ship.integrity())
    }

    fn dev_ship(&self, id: u64) -> Result<&Raider, String> {
        self.ship(id).ok_or_else(|| no_ship(id))
    }
    fn dev_ship_mut(&mut self, id: u64) -> Result<&mut Raider, String> {
        (self.raiders.iter_mut())
            .find(|r| r.body.id == id)
            .ok_or_else(|| no_ship(id))
    }
    /// Whether a hull `radius` metres in the beam fits at `pos` clear of the land and
    /// the harbours, or where it would be ashore.
    pub fn dev_check_water(&self, radius: f32, pos: Vec2) -> Result<(), String> {
        if !pos.is_finite() {
            return Err("that position isn't a number".into());
        }
        let margin = radius + 4.;
        let island = (self.landmarks.iter()).find(|l| l.body.pos.distance(pos) < l.radius + margin);
        if let Some(l) = island {
            return Err(format!(
                "({:.0}, {:.0}) is ashore on the {:?} at ({:.0}, {:.0}), {:.0} m across",
                pos.x,
                pos.y,
                l.kind,
                l.body.pos.x,
                l.body.pos.y,
                l.radius * 2.
            ));
        }
        let harbour = (self.ports.iter().enumerate()).find(|(_, port)| {
            port.shores()
                .any(|(at, radius)| at.distance(pos) < radius + margin)
        });
        if let Some((index, _)) = harbour {
            return Err(format!(
                "({:.0}, {:.0}) is ashore at base {index}",
                pos.x, pos.y
            ));
        }
        Ok(())
    }
}

impl Weight {
    /// Whether ships of `class` come in weights at all: destroyers, cruisers and
    /// battleships do.
    pub fn comes_in(class: EnemyClass) -> bool {
        Self::row(class).is_some()
    }
}

fn no_ship(id: u64) -> String {
    format!("no ship afloat has id {id}")
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn battle() -> Battle {
        let gates = [
            Vec2::new(0., 400.),
            Vec2::new(400., 0.),
            Vec2::new(0., -400.),
            Vec2::new(-400., 0.),
            Vec2::new(0., 400.),
        ];
        Battle::for_route(30., gates)
    }
    /// Somewhere in open water a long way from every island and harbour.
    fn open(b: &Battle) -> Vec2 {
        (0..400)
            .map(|i| Vec2::from_angle(i as f32 * 0.7) * (100. + i as f32 * 9.))
            .find(|&p| b.dev_check_water(20., p).is_ok())
            .expect("some open water")
    }

    #[test]
    fn a_placed_ship_joins_the_battle_and_can_be_taken_away() {
        let mut b = battle();
        let before = b.raiders().len();
        let at = open(&b);
        let id = b
            .dev_spawn(EnemyClass::Cruiser, at, 1., Some(Weight::Heavy))
            .unwrap();
        assert_eq!(b.raiders().len(), before + 1);
        let r = b.ship(id).unwrap();
        assert_eq!(
            (r.class, r.weight(), r.body.pos),
            (EnemyClass::Cruiser, Weight::Heavy, at)
        );
        assert_eq!(b.dev_remove(id), Ok(EnemyClass::Cruiser));
        assert!(b.ship(id).is_none());
        assert!(b.dev_remove(id).is_err());
    }

    #[test]
    fn no_ship_is_placed_or_moved_ashore() {
        let mut b = battle();
        let island = b.landmarks()[0].body.pos;
        assert!(
            b.dev_spawn(EnemyClass::Destroyer, island, 0., None)
                .is_err()
        );
        let id = b
            .dev_spawn(EnemyClass::Destroyer, open(&b), 0., None)
            .unwrap();
        assert!(b.dev_move(id, island, None).is_err());
        assert!(b.dev_move(id, Vec2::NAN, None).is_err());
    }

    #[test]
    fn a_rebuilt_ship_keeps_her_id_and_place_and_a_regun_changes_her_barrels() {
        let mut b = battle();
        let at = open(&b);
        let id = b
            .dev_spawn(EnemyClass::Destroyer, at, 0.5, Some(Weight::Light))
            .unwrap();
        b.dev_replace(id, EnemyClass::Battleship, None).unwrap();
        let r = b.ship(id).unwrap();
        assert_eq!(
            (r.class, r.weight(), r.body.pos),
            (EnemyClass::Battleship, Weight::Light, at)
        );
        let light = r.barrels();
        b.dev_reweigh(id, Weight::UltraHeavy).unwrap();
        assert!(b.ship(id).unwrap().barrels() > light);
        let coaster = b
            .dev_spawn(EnemyClass::Coaster, open(&b), 0., None)
            .unwrap();
        assert!(b.dev_reweigh(coaster, Weight::Heavy).is_err());
    }

    #[test]
    fn damage_wears_her_hull_and_a_rebuild_mends_it() {
        let mut b = battle();
        let id = b
            .dev_spawn(EnemyClass::Cruiser, open(&b), 0., Some(Weight::Standard))
            .unwrap();
        let left = b.dev_damage(id, 20.).unwrap();
        assert!(left < 1., "{left}");
        b.dev_replace(id, EnemyClass::Cruiser, None).unwrap();
        assert_eq!(b.ship(id).unwrap().vessel().integrity(), 1.);
    }
}
