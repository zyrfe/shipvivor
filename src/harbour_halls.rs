//! Three workshops that share an eight-metre footprint, each built in its own trade.
use super::*;

const GLASS: Color = Color::srgb(0.49, 0.65, 0.66);
const OLD_ZINC: Color = Color::srgb(0.64, 0.67, 0.63);
const RUST: Color = Color::srgb(0.54, 0.33, 0.24);
const RED_LEAD: Color = Color::srgb(0.70, 0.35, 0.22);
const MORTAR: Color = Color::srgb(0.72, 0.65, 0.52);

impl Works {
    /// Runs of galvanized roofing from ridge to eave. Different sheets carry old repairs.
    fn sheet_roof(&mut self, ridge: Vec2, eave: Vec2, z_span: f32, painted: Color) {
        let v = Vec3::new;
        let p = |xz: Vec2, z: f32| v(xz.x, xz.y, z);
        let sign = (eave.x - ridge.x).signum();
        for i in 0..8 {
            let z = sign * z_span * (i as f32 / 4. - 1.);
            let next = z + sign * z_span / 4.;
            let sheet = match (i + usize::from(sign > 0.)) % 8 {
                2 => OLD_ZINC,
                4 => TIN,
                6 => RUST,
                _ => painted,
            };
            self.quad(
                [p(eave, z), p(ridge, z), p(ridge, next), p(eave, next)],
                sheet,
            );
            self.girder(
                p(ridge, z) + Vec3::Y * 0.038,
                p(eave, z) + Vec3::Y * 0.038,
                0.075,
                OLD_ZINC,
            );
        }
        self.girder(p(eave, -z_span), p(eave, z_span), 0.14, IRON);
    }

    fn gable_end(&mut self, x: f32, eaves: f32, half: f32, rise: f32, z: f32, col: Color) {
        let v = Vec3::new;
        let side = z.signum();
        let ridge = v(x, eaves + rise, z);
        let a = v(x - side * half, eaves, z);
        let b = v(x + side * half, eaves, z);
        self.0.triangle([a, b, ridge], col);
        self.girder(a, ridge, 0.13, TIMBER);
        self.girder(b, ridge, 0.13, TIMBER);
    }

    /// A factory sash has a recessed frame, glazing bars, and a projecting stone sill.
    fn sash(&mut self, at: Vec3, across: Vec3, width: f32, height: f32, glass: Color) {
        let normal = across.cross(Vec3::Y);
        let v = |w, h, d| Vec3::new(w, h, d);
        let rotation = Quat::from_rotation_y(-across.z.atan2(across.x));
        self.block(at, v(width + 0.17, height + 0.18, 0.13), rotation, IRON);
        self.block(at + normal * 0.083, v(width, height, 0.08), rotation, glass);
        let face = at + normal * 0.147;
        for fraction in [-0.25, 0., 0.25] {
            self.block(
                face + across * (width * fraction),
                v(0.078, height, 0.054),
                rotation,
                IRON,
            );
        }
        self.block(
            face + Vec3::Y * height * 0.05,
            v(width, 0.085, 0.056),
            rotation,
            IRON,
        );
        self.block(
            at - Vec3::Y * (height * 0.5 + 0.11),
            v(width + 0.28, 0.13, 0.31),
            rotation,
            OLD_ZINC,
        );
    }

    pub(super) fn cannery(&mut self) {
        let v = Vec3::new;
        let x = -1.2;
        let eaves = 4.56;
        self.slab(v(x, 2.4, 0.), v(4.5, 4.32, 6.96), RED_LEAD);
        self.slab(v(2.36, 1.52, 0.), v(2.64, 2.64, 6.96), OXIDE);
        let ridge = Vec2::new(x, 6.24);
        for side in [-1., 1.] {
            self.sheet_roof(ridge, Vec2::new(x + side * 2.48, eaves), 3.67, TIN);
            self.gable_end(x, eaves, 2.25, 1.68, side * 3.48, RED_LEAD);
        }
        self.sheet_roof(Vec2::new(1.03, 3.98), Vec2::new(3.87, 2.82), 3.69, OLD_ZINC);

        for side in [-1., 1.] {
            let z = side * 3.51;
            // Uneven planks on the old gable and paired sliding doors, pulled off centre.
            for i in 0..13 {
                let board_x = x - 2.09 + i as f32 * 0.348;
                let top = eaves + (1. - ((board_x - x) / 2.25).abs()) * 1.57;
                self.slab(
                    v(board_x, (top + 0.3) * 0.5, z),
                    v(0.072, top - 0.3, 0.09),
                    OXIDE,
                );
            }
            self.slab(v(x, 1.25, z + 0.07 * side), v(2.38, 2.05, 0.11), IRON);
            self.slab(
                v(x, 1.38, z + 0.13 * side),
                v(0.42, 1.60, 0.08),
                Color::srgb(0.70, 0.49, 0.25),
            );
            self.slab(v(x, 2.44, z + 0.14 * side), v(4.04, 0.15, 0.18), IRON);
            for leaf in [-1., 1.] {
                let cx = x + leaf * 1.19;
                self.slab(v(cx, 1.33, z + side * 0.16), v(1.32, 2.1, 0.16), TIMBER);
                let face = z + side * 0.27;
                self.girder(
                    v(cx - 0.55, 0.41, face),
                    v(cx + 0.55, 2.20, face),
                    0.115,
                    OLD_ZINC,
                );
                self.girder(
                    v(cx - 0.55, 2.20, face),
                    v(cx + 0.55, 0.41, face),
                    0.115,
                    OLD_ZINC,
                );
            }
            self.sash(
                v(x, 3.57, z + side * 0.1),
                Vec3::X * side,
                2.78,
                0.86,
                GLASS,
            );
            self.sash(v(2.4, 1.65, z), Vec3::X * side, 1.56, 0.86, GLASS);
        }
        for z in [-2.23, 0., 2.23] {
            self.sash(v(-3.48, 2.71, z), Vec3::Z, 1.5, 1.34, GLASS);
            self.sash(v(3.73, 1.83, z), Vec3::NEG_Z, 1.52, 0.86, GLASS);
        }
    }

    pub(super) fn tidal_mill(&mut self) {
        let v = Vec3::new;
        let whitewash = Color::srgb(0.70, 0.70, 0.59);
        self.slab(v(0., 1.99, 0.), v(7.3, 3.56, 7.02), whitewash);
        // Open triangular cheeks and glazed vertical north-lights give the roof its teeth.
        for bay in 0..3 {
            let a = -3.60 + bay as f32 * 2.40;
            let b = a + 2.34;
            let lo = 3.76;
            let high = 5.26;
            for i in 0..8 {
                let x = -3.83 + i as f32 * 0.958;
                let nx = x + 0.958;
                self.quad(
                    [v(x, lo, a), v(x, high, b), v(nx, high, b), v(nx, lo, a)],
                    match (i + 2 * bay) % 6 {
                        0 => RUST,
                        2 => OLD_ZINC,
                        _ => TIN,
                    },
                );
                self.girder(v(x, lo, a), v(x, high, b), 0.075, OLD_ZINC);
            }
            for side in [-1., 1.] {
                let x = side * 3.65;
                let (a, bottom_b) = (v(x, lo, a), v(x, lo, b));
                let top = v(x, high, b);
                if side > 0. {
                    self.0.triangle([a, top, bottom_b], MARINE_BLUE);
                } else {
                    self.0.triangle([a, bottom_b, top], MARINE_BLUE);
                }
            }
            self.slab(
                v(0., (lo + high) * 0.5, b),
                v(7.4, high - lo, 0.18),
                MARINE_BLUE,
            );
            for x in [-2.48, 0., 2.48] {
                self.sash(v(x, 4.55, b + 0.09), Vec3::X, 1.99, 0.73, GLASS);
            }
        }
        for side in [-1., 1.] {
            let z = side * 3.54;
            self.slab(v(0., 1.57, z), v(2.84, 2.69, 0.12), RED_LEAD);
            for leaf in [-1., 1.] {
                let x = leaf * 0.69;
                self.girder(
                    v(x - 0.54, 0.38, z + side * 0.09),
                    v(x + 0.54, 2.77, z + side * 0.09),
                    0.13,
                    TIMBER,
                );
            }
            for x in [-2.61, 2.61] {
                self.sash(v(x, 2.16, z), Vec3::X * side, 1.19, 1.24, WINDOW);
            }
            for bay in [-2.35, 0., 2.35] {
                self.sash(
                    v(side * 3.67, 2.29, bay),
                    Vec3::NEG_Z * side,
                    1.66,
                    1.42,
                    GLASS,
                );
            }
            self.slab(v(side * 3.71, 0.58, 0.), v(0.17, 0.59, 7.), MARINE_BLUE);
        }
    }

    pub(super) fn foundry_hall(&mut self) {
        let v = Vec3::new;
        let eaves = 4.96;
        self.slab(v(0., 2.61, 0.), v(7.28, 4.70, 7.07), BRICK);
        let ridge = Vec2::new(0., 6.96);
        for side in [-1., 1.] {
            self.gable_end(0., eaves, 3.64, 2., side * 3.54, BRICK);
            self.sheet_roof(ridge, Vec2::new(side * 3.87, eaves), 3.78, IRON);
            let z = side * 3.58;
            // Furnace light behind a great arched iron sash and a row of three draw doors.
            self.sash(v(0., 3.72, z), Vec3::X * side, 4.92, 1.78, FURNACE);
            self.slab(v(0., 4.99, z), v(3.85, 0.66, 0.13), FURNACE);
            self.slab(v(0., 5.42, z), v(2.78, 0.35, 0.13), FURNACE);
            for x in [-1.23, 0., 1.23] {
                self.slab(v(x, 5.03, z + side * 0.10), v(0.14, 1.03, 0.10), IRON);
                self.slab(v(x * 1.52, 1.28, z), v(1.41, 1.64, 0.15), CHAR);
                self.slab(
                    v(x * 1.52, 1.17, z + 0.10 * side),
                    v(0.94, 0.83, 0.10),
                    OXIDE,
                );
                self.slab(
                    v(x * 1.52, 0.99, z + 0.16 * side),
                    v(0.76, 0.49, 0.06),
                    FURNACE,
                );
            }
            self.slab(v(0., 2.42, z + 0.04 * side), v(7.25, 0.17, 0.22), MORTAR);
            for x in [-3.43, 3.43] {
                self.slab(v(x, 2.6, z), v(0.42, 4.67, 0.38), OXIDE);
                self.slab(v(x, 4.69, z), v(0.56, 0.22, 0.48), MORTAR);
            }
            for z in [-2.34, 0., 2.34] {
                self.sash(
                    v(side * 3.67, 3.38, z),
                    Vec3::NEG_Z * side,
                    1.53,
                    1.84,
                    FURNACE,
                );
                self.slab(v(side * 3.67, 2.40, z - 1.05), v(0.39, 4.51, 0.30), OXIDE);
            }
        }
        self.slab(v(0., 6.95, 0.), v(1.38, 0.8, 4.04), WINDOW);
        self.slab(v(0., 7.42, 0.), v(1.8, 0.22, 4.34), TIN);
    }

    pub(super) fn chimney(&mut self, harbour: Harbour) {
        let lip = harbour.chimney();
        let foot = lip - Vec3::Y * 5.35;
        let r = match harbour {
            Harbour::Cannery => 0.47,
            Harbour::TwinMills => 0.57,
            Harbour::Foundry => 0.76,
        };
        self.stack(foot, 5.17, r * 1.29, r, BRICK);
        for h in [0.75, 1.92, 3.29, 4.68] {
            let radius = r * (1.33 - h * 0.05);
            self.stack(foot + Vec3::Y * h, 0.16, radius, radius, IRON);
        }
        self.stack(lip - Vec3::Y * 0.42, 0.42, r * 1.21, r * 1.21, OXIDE);
        self.stack(lip - Vec3::Y * 0.14, 0.15, r * 1.29, r * 1.29, CHAR);
        // A fixed ladder reaches the cap from the roof.
        let rail = foot + Vec3::Z * r * 1.36;
        for side in [-1., 1.] {
            self.girder(
                rail + Vec3::X * side * 0.22,
                rail + Vec3::X * side * 0.22 + Vec3::Y * 5.12,
                0.07,
                IRON,
            );
        }
        for step in 2..11 {
            let c = rail + Vec3::Y * (step as f32 * 0.44);
            self.girder(c - Vec3::X * 0.24, c + Vec3::X * 0.24, 0.065, STRIPE);
        }
    }
}
