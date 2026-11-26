//! How an enemy ship is conned. Her rudder brings her round only as fast as her way
//! through the water allows, and her course bends round land and heavy ice.
use super::entry;
use glam::{FloatExt, Vec2};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Share of hard over the rudder swings through in a second, as on Breakwater.
const RUDDER_RATE: f32 = 0.8;
/// Error off her course, in radians, at which the helmsman puts the rudder hard over.
const HARD_OVER: f32 = 0.35;
/// Radians she gives a hazard beyond the edge of the water it blocks.
const BERTH: f32 = 0.04;
/// How much a course near her present heading is preferred to one near her goal. It
/// keeps her going round a hazard the way she has begun.
const HOLD: f32 = 0.5;

fn signed(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Helm {
    /// Rudder angle as a share of hard over, positive to starboard.
    pub rudder: f32,
    /// Way through the water along her heading, in metres per second.
    pub speed: f32,
}

impl Helm {
    pub fn under_way(speed: f32) -> Self {
        Self { rudder: 0., speed }
    }

    /// Radians a second she swings: her way over the radius of the circle her rudder
    /// would turn her in. Without way on, the rudder does nothing.
    pub fn yaw(&self, turning: f32) -> f32 {
        self.rudder * self.speed.max(0.) / turning
    }

    /// Gather or lose way towards `wanted`, by `rate` metres a second each second.
    pub fn make_way(&mut self, wanted: f32, rate: f32, dt: f32) {
        self.speed = self.speed.move_towards(wanted, rate * dt);
    }

    /// Swing the rudder to bring her round to `course`, easing it back in time to meet
    /// her on the new heading.
    pub fn steer(&mut self, heading: f32, course: f32, turning: f32, dt: f32) {
        let error = signed(course - heading);
        // She carries on swinging while the rudder comes back amidships.
        let carry = self.yaw(turning) * self.rudder.abs() / (2. * RUDDER_RATE);
        let order = ((error - carry) / HARD_OVER).clamp(-1., 1.);
        self.rudder = self.rudder.move_towards(order, RUDDER_RATE * dt);
    }

    pub fn amidships(&mut self, dt: f32) {
        self.rudder = self.rudder.move_towards(0., RUDDER_RATE * dt);
    }

    /// Her heading after a tick of swinging under the rudder.
    pub fn turn(&self, heading: f32, turning: f32, dt: f32) -> f32 {
        let yaw = self.yaw(turning);
        if yaw == 0. {
            heading
        } else {
            signed(heading + yaw * dt)
        }
    }
}

/// Her course from `from`: `along` if the water that way is open for `reach` metres,
/// or else the nearest course that clears every hazard, a circle she keeps out of.
/// Of two courses about as near `along`, she keeps to the one nearer `ahead`, her
/// present heading. Hemmed in, she takes the course with the most open water.
pub(super) fn clear_course(
    from: Vec2,
    along: Vec2,
    ahead: Vec2,
    reach: f32,
    hazards: impl IntoIterator<Item = (Vec2, f32)>,
) -> Vec2 {
    let near: Vec<(Vec2, f32)> = hazards
        .into_iter()
        .filter(|&(centre, radius)| from.distance(centre) - radius < reach)
        .collect();
    if near.is_empty() || along == Vec2::ZERO {
        return along;
    }
    // Metres of open water ahead on a course, up to `reach`.
    let open = |course: Vec2| {
        near.iter().fold(reach, |run, &(centre, radius)| {
            let out = from - centre;
            if out.length_squared() < radius * radius {
                // Already inside the circle, any course that does not close it is open.
                if out.dot(course) < 0. { 0. } else { run }
            } else {
                entry(from, from + course * reach, centre, radius)
                    .map_or(run, |t| run.min(t * reach))
            }
        })
    };
    if open(along) >= reach {
        return along;
    }
    // Each hazard blocks a fan of courses. Its edges are the courses that just clear it.
    let edges = near.iter().flat_map(|&(centre, radius)| {
        let to = centre - from;
        let distance = to.length();
        let spread = if distance > radius {
            (radius / distance).asin()
        } else {
            FRAC_PI_2
        } + BERTH;
        let middle = to.normalize_or(-along);
        [-spread, spread].map(|a| Vec2::from_angle(a).rotate(middle))
    });
    let cost = |course: Vec2| along.angle_to(course).abs() + HOLD * ahead.angle_to(course).abs();
    edges
        .chain([along])
        .map(|course| (open(course) >= reach, open(course), cost(course), course))
        .max_by(|a, b| {
            a.0.cmp(&b.0).then_with(|| {
                if a.0 {
                    b.2.total_cmp(&a.2)
                } else {
                    a.1.total_cmp(&b.1)
                }
            })
        })
        .map_or(along, |(.., course)| course)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shipvivor_compass::{direction, heading as bearing};

    /// Swing a ship at `speed` round to `course` and report her largest overshoot and
    /// the error left after `seconds`.
    fn come_round(speed: f32, turning: f32, course: f32, seconds: f32) -> (f32, f32) {
        let mut helm = Helm::under_way(speed);
        let mut heading = 0.;
        let mut overshoot: f32 = 0.;
        for _ in 0..(seconds * 60.) as usize {
            helm.steer(heading, course, turning, 1. / 60.);
            heading = helm.turn(heading, turning, 1. / 60.);
            overshoot = overshoot.max(signed(heading - course) * course.signum());
        }
        (overshoot, signed(course - heading))
    }

    #[test]
    fn she_comes_round_no_faster_than_her_way_allows_and_meets_her_new_course() {
        for (speed, turning) in [(5.4, 16.), (7.6, 20.), (3.3, 40.), (3.4, 110.)] {
            let cap = speed / turning;
            let quarter = FRAC_PI_2 / cap;
            for course in [FRAC_PI_2, -FRAC_PI_2, 3.] {
                let mut helm = Helm::under_way(speed);
                let mut heading = 0.;
                for _ in 0..600 {
                    helm.steer(heading, course, turning, 1. / 60.);
                    let next = helm.turn(heading, turning, 1. / 60.);
                    assert!(signed(next - heading).abs() <= cap / 60. + 1e-6);
                    heading = next;
                }
                let (overshoot, left) = come_round(speed, turning, course, quarter * 3. + 6.);
                assert!(
                    overshoot < 0.5_f32.to_radians(),
                    "{speed} {turning}: {overshoot}"
                );
                assert!(
                    left.abs() < 0.2_f32.to_radians(),
                    "{speed} {turning}: {left}"
                );
            }
        }
    }

    #[test]
    fn without_way_on_the_rudder_does_not_turn_her() {
        let mut helm = Helm::under_way(0.);
        let mut heading = 1.;
        for _ in 0..600 {
            helm.steer(heading, -2., 20., 1. / 60.);
            heading = helm.turn(heading, 20., 1. / 60.);
        }
        assert_eq!(helm.rudder, -1.);
        assert_eq!(heading, 1.);
        // Gathering way, she answers the rudder already over.
        helm.make_way(4., 1., 1.);
        assert!(helm.turn(heading, 20., 1. / 60.) < heading);
    }

    #[test]
    fn a_course_into_an_island_bends_to_just_clear_it_and_holds_as_she_closes() {
        let island = (Vec2::new(0., -100.), 30.);
        let north = direction(0.);
        // Open water: she holds her course.
        assert_eq!(
            clear_course(Vec2::ZERO, north, north, 60., [(Vec2::new(200., 0.), 30.)]),
            north
        );
        let course = clear_course(Vec2::ZERO, north, north, 120., [island]);
        let clearance = (30_f32 / 100.).asin() + BERTH;
        assert!((north.angle_to(course).abs() - clearance).abs() < 1e-4);
        // Steaming down that course she keeps the side she chose, all the way past.
        let side = north.angle_to(course).signum();
        let goal = Vec2::new(0., -260.);
        let mut at = Vec2::ZERO;
        let mut ahead = course;
        for _ in 0..1000 {
            if at.distance(goal) < 2. {
                return;
            }
            let along = (goal - at).normalize();
            let next = clear_course(at, along, ahead, 120., [island]);
            assert!(
                along.angle_to(next) * side >= -1e-4,
                "she swapped sides at {at}"
            );
            assert!(
                next.angle_to(ahead).abs() < 0.1,
                "her course jumped at {at}"
            );
            ahead = next;
            at += ahead * 0.5;
            assert!(at.distance(island.0) > island.1);
        }
        panic!("she never reached her goal: {at}");
    }

    #[test]
    fn dead_ahead_she_rounds_the_island_on_the_side_she_is_already_turning_to() {
        let island = (Vec2::new(0., -100.), 30.);
        let north = direction(0.);
        for side in [-1., 1.] {
            let ahead = direction(side * 0.1);
            let course = clear_course(Vec2::ZERO, north, ahead, 120., [island]);
            assert_eq!(bearing(course).signum(), side);
        }
    }

    #[test]
    fn two_overlapping_shores_turn_her_round_the_outside_of_both() {
        let shores = [(Vec2::new(-20., -80.), 30.), (Vec2::new(25., -85.), 30.)];
        let north = direction(0.);
        let course = clear_course(Vec2::ZERO, north, north, 150., shores);
        for (centre, radius) in shores {
            assert!(entry(Vec2::ZERO, course * 150., centre, radius).is_none());
        }
    }

    #[test]
    fn inside_a_hazard_margin_she_may_run_along_it_or_out_but_not_in() {
        let island = (Vec2::ZERO, 30.);
        let from = Vec2::new(0., 28.);
        let course = clear_course(from, Vec2::NEG_Y, Vec2::NEG_Y, 60., [island]);
        assert!(course.dot(from) >= 0.);
        assert!(course.dot(from.normalize().perp()).abs() > 0.99);
    }

    #[test]
    fn ships_in_a_sector_swing_steadily_and_never_flap_between_headings() {
        use crate::{Aim, Battle, Hull, MainShell, Orders};
        use std::collections::HashMap;
        let mut b = Battle::for_route(
            14.,
            [
                (0., -152.),
                (168., -424.),
                (496., -300.),
                (416., 76.),
                (72., 164.),
            ]
            .map(Vec2::from),
        );
        let orders = Orders {
            player: Vec2::ZERO,
            heading: 0.,
            speed: 0.,
            hull: Hull::CRUISER,
            aim: Aim::Hold,
            reload: 3.2,
            shell: MainShell::STOCK,
        };
        let dt = 1. / 60.;
        // Heading, last swing, swings reversed and ticks seen, for each ship.
        let mut log: HashMap<u64, (f32, f32, u32, u32)> = HashMap::new();
        for _ in 0..60 * 60 {
            b.step(dt, &orders);
            for r in b.raiders() {
                let (heading, swing, reversed, ticks) =
                    log.entry(r.body.id).or_insert((r.body.heading, 0., 0, 0));
                let turn = signed(r.body.heading - *heading);
                // No class swings faster than a destroyer at full speed and full rudder.
                assert!(
                    turn.abs() <= 22_f32.to_radians() * dt,
                    "{:?} swung {turn}",
                    r.class
                );
                if turn != 0. {
                    if *swing != 0. && turn.signum() != swing.signum() {
                        *reversed += 1;
                    }
                    *swing = turn;
                }
                *heading = r.body.heading;
                *ticks += 1;
            }
        }
        assert!(log.len() >= 4);
        for (id, (_, _, reversed, ticks)) in log {
            let seconds = ticks as f32 * dt;
            assert!(
                reversed as f32 <= 1. + seconds,
                "ship {id} reversed {reversed} times in {seconds}s"
            );
        }
    }

    #[test]
    fn a_flagship_bound_round_a_headland_makes_her_way_up_the_channel() {
        use crate::{Battle, Chart, EnemyClass, Waters, fairway_point, fairway_run, pilotage};
        // On these charts the player, seen at the head, is hidden behind a bend. The
        // fairway mark she is led by lies nearer than her stand-off from the quarry.
        for seed in [1, 3] {
            let (chart, _) = Chart::generate(seed, &Waters::SOUND).unwrap();
            let mut b = Battle::for_chart(20., &chart);
            let length = fairway_run(&b.fairway, *b.fairway.last().unwrap());
            let foot = fairway_point(&b.fairway, length * 0.15);
            let head = fairway_point(&b.fairway, length * 0.95);
            b.raiders.clear();
            b.spawn(
                EnemyClass::Parliament,
                foot,
                fairway_point(&b.fairway, length * 0.15 + 20.),
            );
            let start = fairway_run(&b.fairway, foot);
            let mut sailed = 0.;
            for _ in 0..60 * 60 {
                let r = &mut b.raiders[0];
                let bound = pilotage(&b.fairway, &b.landmarks, r.body.pos, head);
                r.sail(
                    1. / 60.,
                    bound,
                    bound != head,
                    &b.ports,
                    &b.landmarks,
                    &[],
                    Vec2::ZERO,
                );
                sailed += r.motion().length() / 60.;
            }
            let made = fairway_run(&b.fairway, b.raiders[0].body.pos) - start;
            assert!(
                made > 0.8 * sailed,
                "seed {seed}: made {made} of {sailed} m"
            );
        }
    }
}
