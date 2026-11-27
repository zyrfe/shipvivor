use glam::{FloatExt, Vec2};

// Speeds are in metres per second. See `shipvivor_compass` for the chart convention.
const STOP: usize = 3;
const ORDERS: [(&str, f32); 8] = [
    ("FULL ASTERN", -5.0),
    ("HALF ASTERN", -3.5),
    ("SLOW ASTERN", -1.7),
    ("STOP", 0.0),
    ("SLOW AHEAD", 3.4),
    ("HALF AHEAD", 6.8),
    ("THREE-QUARTER AHEAD", 10.2),
    ("FULL AHEAD", 13.6),
];

#[derive(Clone)]
pub struct Helm {
    pub position: Vec2,
    pub heading: f32,
    pub speed: f32,
    pub rudder: f32,
    turn_rate: f32,
    order: usize,
}

impl Default for Helm {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            heading: 0.0,
            speed: 0.0,
            rudder: 0.0,
            turn_rate: 0.0,
            order: STOP,
        }
    }
}

impl Helm {
    /// Speed of the highest engine order before any refit.
    pub const FULL_AHEAD: f32 = ORDERS[ORDERS.len() - 1].1;

    pub fn notch(&self) -> i8 {
        self.order as i8 - STOP as i8
    }

    /// Speed the telegraph is asking for, before any refit.
    pub fn ordered_speed(&self) -> f32 {
        ORDERS[self.order].1
    }

    pub fn order_label(&self) -> &'static str {
        ORDERS[self.order].0
    }

    pub fn ring(&mut self, step: isize) {
        self.order = self.order.saturating_add_signed(step).min(ORDERS.len() - 1);
    }

    pub fn forward(&self) -> Vec2 {
        shipvivor_compass::direction(self.heading)
    }

    pub fn tick(&mut self, rudder_order: f32, dt: f32) {
        self.tick_with_speed(rudder_order, dt, 1.);
    }

    pub fn tick_with_speed(&mut self, rudder_order: f32, dt: f32, speed_factor: f32) {
        self.tick_with_handling(rudder_order, dt, speed_factor, 0.38);
    }

    pub fn tick_with_handling(
        &mut self,
        rudder_order: f32,
        dt: f32,
        speed_factor: f32,
        turn_rate: f32,
    ) {
        self.rudder = self.rudder.move_towards(rudder_order, 0.8 * dt);
        // Turning bleeds a little speed. Even a full-astern order takes time to arrest way.
        let target_speed = ORDERS[self.order].1 * speed_factor * (1.0 - 0.15 * self.rudder.abs());
        let acceleration = if target_speed == 0.0 { 0.8 } else { 1.35 };
        self.speed = self.speed.move_towards(target_speed, acceleration * dt);
        let water_flow = (self.speed / 7.0).clamp(-1.0, 1.0);
        let target_turn = turn_rate * self.rudder * water_flow;
        self.turn_rate = self.turn_rate.move_towards(target_turn, 0.29 * dt);
        self.heading = (self.heading + self.turn_rate * dt).rem_euclid(std::f32::consts::TAU);
        self.position += self.forward() * self.speed * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turbine_refit_raises_the_speed_of_the_same_engine_order() {
        let mut stock = Helm::default();
        let mut refit = Helm::default();
        stock.ring(4);
        refit.ring(4);
        for _ in 0..1200 {
            stock.tick(0., 1. / 60.);
            refit.tick_with_speed(0., 1. / 60., 1.15);
        }
        assert!((refit.speed / stock.speed - 1.15).abs() < 0.001);
        assert!(refit.position.y < stock.position.y);
        assert_eq!(refit.order_label(), stock.order_label());
    }

    #[test]
    fn each_telegraph_detent_holds_its_own_speed() {
        let mut helm = Helm::default();
        helm.ring(-20);
        for (index, &(name, speed)) in ORDERS.iter().enumerate() {
            assert_eq!(helm.order_label(), name);
            assert_eq!(helm.notch(), index as i8 - STOP as i8);
            for _ in 0..900 {
                helm.tick(0., 1. / 60.);
            }
            assert!((helm.speed - speed).abs() < 0.001);
            helm.ring(1);
        }
        assert_eq!(helm.order_label(), "FULL AHEAD");
    }

    #[test]
    fn stop_and_astern_are_distinct_orders_with_momentum() {
        let mut helm = Helm::default();
        helm.ring(10);
        for _ in 0..300 {
            helm.tick(0.0, 1.0 / 60.0);
        }
        let ahead_speed = helm.speed;
        helm.ring(-3);
        helm.ring(-1);
        assert_eq!(helm.order_label(), "STOP");
        helm.tick(0.0, 1.0 / 60.0);
        assert!(helm.speed > 0.0 && helm.speed < ahead_speed);
        helm.ring(-10);
        for _ in 0..600 {
            helm.tick(1.0, 1.0 / 60.0);
        }
        assert!(helm.speed < 0.0 && helm.turn_rate < 0.0);
    }

    #[test]
    fn stationary_ship_cannot_pivot_and_released_rudder_centres() {
        let mut helm = Helm::default();
        for _ in 0..120 {
            helm.tick(1.0, 1.0 / 60.0);
        }
        assert_eq!(helm.rudder, 1.0);
        assert_eq!(helm.heading, 0.0);
        for _ in 0..120 {
            helm.tick(0.0, 1.0 / 60.0);
        }
        assert_eq!(helm.rudder, 0.0);
    }

    #[test]
    fn port_and_starboard_orders_produce_mirrored_turns_and_cost_speed() {
        let mut port = Helm::default();
        let mut starboard = Helm::default();
        let mut straight = Helm::default();
        for ship in [&mut port, &mut starboard, &mut straight] {
            ship.ring(4);
        }
        for _ in 0..720 {
            port.tick(-1.0, 1.0 / 60.0);
            starboard.tick(1.0, 1.0 / 60.0);
            straight.tick(0.0, 1.0 / 60.0);
        }
        assert!(starboard.position.x > 0.0);
        assert!((port.position.x + starboard.position.x).abs() < 0.01);
        assert!((port.position.y - starboard.position.y).abs() < 0.01);
        assert_eq!(port.speed, starboard.speed);
        assert!(starboard.speed < straight.speed);
    }
}
