use super::*;
use std::collections::{BTreeSet, HashSet};

const SEEDS: u64 = 3000;

#[test]
fn the_same_seed_flies_the_same_flag() {
    for seed in [0, 1, 7, u64::MAX, 0xDEAD_BEEF] {
        let flag = Flag::generate(seed);
        assert_eq!(flag, Flag::generate(seed));
        assert_eq!(Flag::rival(seed ^ 1, &flag), Flag::rival(seed ^ 1, &flag));
    }
}

#[test]
fn gold_on_white_and_red_on_green_never_touch() {
    assert!(!Gold.reads_against(White));
    assert!(!Scarlet.reads_against(Green));
    assert!(!Navy.reads_against(Black));
    for t in Tincture::ALL {
        assert!(!t.reads_against(t), "{t} reads against itself");
        assert!(t.reads_against(White) || t.reads_against(Black));
        let partners = Tincture::ALL
            .into_iter()
            .filter(|&o| t.reads_against(o))
            .count();
        assert!(partners >= 3, "{t} has only {partners} partners");
    }
}

#[test]
fn every_slot_shows_on_the_cloth_and_no_pattern_needs_too_many_colours() {
    const COLUMNS: usize = 560;
    const ROWS: usize = 200;
    for pattern in Pattern::ALL {
        assert!((2..=MAX_COLOURS).contains(&pattern.slots()), "{pattern:?}");
        let shown: BTreeSet<usize> = (0..ROWS)
            .flat_map(|r| (0..COLUMNS).map(move |c| (c, r)))
            .map(|(c, r)| {
                pattern.region(
                    (c as f32 + 0.5) / COLUMNS as f32,
                    (r as f32 + 0.5) / ROWS as f32,
                )
            })
            .collect();
        assert_eq!(shown, (0..pattern.slots()).collect(), "{pattern:?}");
    }
    let distinct: HashSet<Pattern> = Pattern::ALL.into_iter().collect();
    assert_eq!(distinct.len(), Pattern::ALL.len());
}

#[test]
fn borders_are_the_edges_the_cloth_shows() {
    let chain = |n: usize| (1..n).map(|i| (i - 1, i)).collect::<Vec<_>>();
    for n in 2..=4 {
        assert_eq!(Bands(n as u8).borders(), chain(n));
        assert_eq!(Pales(n as u8).borders(), chain(n));
        assert_eq!(Slashes(n as u8).borders(), chain(n));
    }
    // Opposite quarters and opposite triangles meet only at the centre.
    assert_eq!(Quarters.borders(), [(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(Triangles.borders(), [(0, 1), (0, 3), (1, 2), (2, 3)]);
    // The X parts head and foot from hoist and fly everywhere.
    assert_eq!(Saltire.borders(), [(0, 2), (1, 2)]);
    // The canton runs beside the bands it covers and sits on the next.
    assert_eq!(Canton(1).borders(), [(0, 1)]);
    assert_eq!(Canton(2).borders(), [(0, 1), (0, 2), (1, 2)]);
    assert_eq!(
        Canton(3).borders(),
        [(0, 1), (0, 2), (0, 3), (1, 2), (2, 3)]
    );
}

#[test]
fn slashes_rise_from_the_lower_hoist_and_the_saltire_runs_corner_to_corner() {
    for n in 2..=4 {
        let slashes = Slashes(n);
        let last = usize::from(n) - 1;
        assert_eq!(slashes.region(0.02, 0.02), 0);
        assert_eq!(slashes.region(0.98, 0.98), last);
        if n > 2 {
            // The slashes run out through both far corners.
            assert!((1..last).contains(&slashes.region(0.003, 0.995)));
            assert!((1..last).contains(&slashes.region(0.997, 0.005)));
        }
    }
    for (u, v, slot) in [
        (0.5, 0.04, 0),
        (0.5, 0.96, 0),
        (0.03, 0.5, 1),
        (0.97, 0.5, 1),
        (0.5, 0.5, 2),
        (0.01, 0.01, 2),
        (0.99, 0.01, 2),
        (0.01, 0.99, 2),
        (0.99, 0.99, 2),
    ] {
        assert_eq!(Saltire.region(u, v), slot, "{u}, {v}");
    }
    // Along the head, each arm takes half its width out of the corner it leaves.
    let arm = (0..1000)
        .filter(|&i| Saltire.region(i as f32 / 1000., 0.001) == 2)
        .count() as f32
        / 1000.;
    assert!((arm - SALTIRE).abs() < 0.01, "{arm}");
}

#[test]
fn every_flag_shows_two_to_four_colours_that_read_where_they_meet() {
    for seed in 0..SEEDS {
        let flag = Flag::generate(seed);
        let pattern = flag.pattern();
        assert_eq!(flag.colours().len(), pattern.slots(), "{seed}: {flag}");
        assert!((2..=4).contains(&flag.scheme().len()), "{seed}: {flag}");
        for (a, b) in pattern.borders() {
            let (a, b) = (flag.colours()[a], flag.colours()[b]);
            assert!(a.reads_against(b), "{seed}: {a} meets {b} in {flag}");
        }
        assert!(flag.readable());
        assert_eq!(Flag::new(pattern, flag.colours().to_vec()), Some(flag));
    }
}

#[test]
fn seeds_cover_every_pattern_colour_count_and_tincture() {
    let flags: Vec<Flag> = (0..SEEDS).map(Flag::generate).collect();
    // Patterns whose slots all touch need colours that all read, so they come up less.
    let even = 1. / Pattern::ALL.len() as f32;
    for pattern in Pattern::ALL {
        let share = flags.iter().filter(|f| f.pattern() == pattern).count() as f32 / SEEDS as f32;
        assert!(
            share > even / 4.,
            "{pattern:?} flies on {share:.3} of seeds"
        );
    }
    for count in 2..=4 {
        let share =
            flags.iter().filter(|f| f.scheme().len() == count).count() as f32 / SEEDS as f32;
        assert!(share > 0.12, "{count} colours fly on {share:.3} of seeds");
    }
    for t in Tincture::ALL {
        assert!(
            flags.iter().filter(|f| f.colours().contains(&t)).count() as u64 > SEEDS / 10,
            "{t} is rare"
        );
    }
    let distinct: HashSet<&Flag> = flags.iter().collect();
    assert!(
        distinct.len() as u64 > SEEDS / 2,
        "only {} distinct flags",
        distinct.len()
    );
}

#[test]
fn a_rival_is_never_taken_for_the_flag_it_faces() {
    for seed in 0..600 {
        let ours = Flag::generate(seed);
        let theirs = Flag::rival(seed.wrapping_mul(31) + 5, &ours);
        let apart = theirs.difference(&ours);
        assert!(
            apart >= MIN_DIFFERENCE,
            "{seed}: {ours} / {theirs}: {apart:.2}"
        );
        assert!((apart - ours.difference(&theirs)).abs() < 1e-4);
    }
    // The same colours in another arrangement are the same flag at a distance.
    let tricolour = Flag::new(Bands(3), vec![Navy, White, Scarlet]).unwrap();
    let upright = Flag::new(Pales(3), vec![Scarlet, White, Navy]).unwrap();
    assert!(tricolour.difference(&upright) < 0.01);
    assert!(Flag::rival(9, &tricolour).difference(&tricolour) >= MIN_DIFFERENCE);
}

#[test]
fn a_flag_the_player_picks_still_meets_a_rival() {
    // Any four colours quartered, readable or not, and a flag of one colour.
    let mut picks = vec![Flag::new(Bands(2), vec![Gold, Gold]).unwrap()];
    for a in 0..9 {
        for b in a + 1..9 {
            for c in b + 1..9 {
                for d in c + 1..9 {
                    let colours = [a, b, c, d].map(|i| Tincture::ALL[i]).to_vec();
                    picks.push(Flag::new(Quarters, colours).unwrap());
                }
            }
        }
    }
    for (seed, ours) in picks.iter().enumerate() {
        let theirs = Flag::rival(seed as u64, ours);
        assert!(
            theirs.difference(ours) >= MIN_DIFFERENCE,
            "{ours} / {theirs}"
        );
        assert!(theirs.readable());
    }
    assert!(!Flag::new(Bands(2), vec![Gold, White]).unwrap().readable());
}

#[test]
fn coverage_adds_up_and_follows_the_pattern() {
    let quartered = Flag::new(Quarters, vec![Gold, Black, Black, Gold]).unwrap();
    let cover = quartered.coverage();
    assert!((cover.iter().sum::<f32>() - 1.).abs() < 1e-3);
    assert!((cover[Gold as usize] - 0.5).abs() < 1e-3);
    assert!((cover[Black as usize] - 0.5).abs() < 1e-3);
    let canton = Flag::new(Canton(1), vec![White, Navy]).unwrap();
    let (depth, length) = Canton(1).canton();
    assert!((canton.coverage()[White as usize] - depth * length).abs() < 0.01);
    // A slash covers its width along the whole diagonal, less the two corners.
    let slash = Flag::new(Slashes(3), vec![Green, Gold, Green]).unwrap();
    let across = SLASH / 2.;
    let corners = (1. - across) * (1. - across);
    assert!((slash.coverage()[Gold as usize] - (1. - corners)).abs() < 0.01);
}

#[test]
fn painting_lays_each_slot_in_its_own_colour() {
    let (width, height) = (112, 40);
    let flags = (0..60).map(Flag::generate).chain([
        Flag::new(Saltire, vec![Green, Black, Gold]).unwrap(),
        Flag::new(Slashes(4), vec![Navy, White, Scarlet, Sky]).unwrap(),
    ]);
    for flag in flags {
        let pixels = flag.paint(width, height);
        let slot = flag.paint_slot(1, [40, 40, 40], width, height);
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        assert_eq!(slot.len(), pixels.len());
        // Away from the edges every pixel is exactly its slot's colour.
        for y in (2..height - 2).step_by(3) {
            for x in (2..width - 2).step_by(5) {
                let at = |dx: f32, dy: f32| {
                    flag.pattern().region(
                        (x as f32 + dx) / width as f32,
                        (y as f32 + dy) / height as f32,
                    )
                };
                let centre = at(0.5, 0.5);
                // Sample the pixel's corners and edges: the X is not convex.
                let inside = [(0., 0.), (1., 0.), (0., 1.), (1., 1.), (0.5, 0.), (0., 0.5)]
                    .iter()
                    .all(|&(dx, dy)| at(dx, dy) == centre);
                if inside {
                    let i = ((y * width + x) * 4) as usize;
                    let [r, g, b] = flag.colours()[centre].srgb();
                    assert_eq!(&pixels[i..i + 4], &[r, g, b, 255], "{flag} at {x},{y}");
                    let shown = if centre == 1 { [r, g, b] } else { [40, 40, 40] };
                    assert_eq!(&slot[i..i + 3], &shown, "slot 1 of {flag} at {x},{y}");
                }
            }
        }
    }
}

#[test]
fn a_flag_survives_its_code() {
    for seed in 0..500 {
        let flag = Flag::generate(seed);
        assert_eq!(Flag::parse(&flag.code()), Some(flag));
    }
    for pattern in Pattern::ALL {
        let flag = Flag::new(pattern, vec![Sky; pattern.slots()]).unwrap();
        assert_eq!(Flag::parse(&flag.code()), Some(flag));
    }
    assert_eq!(
        Flag::new(Canton(2), vec![Gold, Scarlet, Black])
            .unwrap()
            .code(),
        "canton2 gold scarlet black"
    );
    assert_eq!(
        Flag::parse("  saltire  green black\tgold\n"),
        Flag::new(Saltire, vec![Green, Black, Gold])
    );
    for junk in [
        "",
        "bands3 navy white",
        "bands3 navy white scarlet gold",
        "bands9 navy white",
        "stripes navy white",
        "bands2 navy teal",
    ] {
        assert_eq!(Flag::parse(junk), None, "{junk:?}");
    }
}

#[test]
fn flags_describe_themselves() {
    let flag =
        |pattern, colours: &[Tincture]| Flag::new(pattern, colours.to_vec()).unwrap().to_string();
    assert_eq!(
        flag(Bands(3), &[Navy, White, Scarlet]),
        "navy over white over scarlet"
    );
    assert_eq!(
        flag(Pales(2), &[Green, White]),
        "upright bands of green and white from the hoist"
    );
    assert_eq!(flag(Canton(1), &[Gold, Navy]), "navy with a gold canton");
    assert_eq!(
        flag(Canton(2), &[Gold, Scarlet, Black]),
        "scarlet over black with a gold canton"
    );
    assert_eq!(
        flag(Quarters, &[White, Scarlet, Scarlet, White]),
        "quartered white, scarlet, scarlet and white"
    );
    assert_eq!(
        flag(Slashes(2), &[Navy, Gold]),
        "navy over gold on the diagonal"
    );
    assert_eq!(
        flag(Slashes(3), &[Green, Gold, Sky]),
        "green and sky blue with a gold slash"
    );
    assert_eq!(
        flag(Slashes(4), &[Green, Gold, Black, Sky]),
        "green and sky blue with gold and black slashes"
    );
    assert_eq!(
        flag(Saltire, &[Green, Black, Gold]),
        "a gold saltire, green at the head and foot, black at the hoist and fly"
    );
    assert_eq!(
        flag(Saltire, &[Navy, Navy, White]),
        "a white saltire on navy"
    );
}
