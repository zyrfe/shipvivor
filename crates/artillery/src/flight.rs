//! Vertical motion in the compressed play space. Time is in game seconds.
const DROP: f32 = 74.;

#[derive(Clone, Copy, Debug)]
pub struct ShellArc {
    height: f32,
    rise: f32,
}
impl ShellArc {
    pub fn between(start_height: f32, end_height: f32, duration: f32) -> Self {
        Self {
            height: start_height,
            rise: (end_height - start_height) / duration + 0.5 * DROP * duration,
        }
    }

    pub fn height(self, t: f32) -> f32 {
        self.height + t * (self.rise - 0.5 * DROP * t)
    }

    pub fn slope(self, t: f32) -> f32 {
        self.rise - DROP * t
    }

    /// First time the arc is inside a vertical band during a horizontal overlap.
    pub fn contact(self, start: f32, end: f32, bottom: f32, top: f32) -> Option<f32> {
        let h = self.height(start);
        if (bottom..=top).contains(&h) {
            return Some(start);
        }
        let level = if h > top { top } else { bottom };
        let disc = self.rise * self.rise + 2. * DROP * (self.height - level);
        if disc < 0. {
            return None;
        }
        let root = disc.sqrt();
        [(self.rise - root) / DROP, (self.rise + root) / DROP]
            .into_iter()
            .find(|t| (start..=end).contains(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flight_has_an_apex_and_lands_at_the_chosen_time() {
        let arc = ShellArc::between(3., 0., 1.);
        assert!(arc.slope(0.) > 0. && arc.slope(1.) < 0.);
        assert!(arc.height(0.5) > 9.);
        assert!(arc.height(1.).abs() < 1e-5);
        assert_eq!(arc.contact(0.0, 1.1, 0., 0.), Some(1.));
        assert_eq!(arc.contact(0.35, 0.55, 0., 4.), None);
        let descent = arc.contact(0.55, 1., 1., 4.).unwrap();
        assert!((arc.height(descent) - 4.).abs() < 1e-4);
    }
}
