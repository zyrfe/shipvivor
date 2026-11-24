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
fn borders_are_the_edges_the_cloth_shows() {
    // Neighbouring cells of a fine grid that fall in different regions. Pairs seen only
    // a cell or two at a time meet at a point, as opposite quarters do.
    const COLUMNS: usize = 560;
    const ROWS: usize = 200;
    for layout in Layout::ALL {
        let at = |c: usize, r: usize| {
            layout.region(
                (c as f32 + 0.5) / COLUMNS as f32,
                (r as f32 + 0.5) / ROWS as f32,
            )
        };
        let mut seen = std::collections::BTreeMap::<(usize, usize), u32>::new();
        for r in 0..ROWS {
            for c in 0..COLUMNS {
                let here = at(c, r);
                for there in [
                    (c + 1 < COLUMNS).then(|| at(c + 1, r)),
                    (r + 1 < ROWS).then(|| at(c, r + 1)),
                ]
                .into_iter()
                .flatten()
                {
                    if here != there {
                        *seen.entry((here.min(there), here.max(there))).or_default() += 1;
                    }
                }
            }
        }
        let edges: BTreeSet<(usize, usize)> = seen
            .into_iter()
            .filter(|&(_, count)| count > 4)
            .map(|(pair, _)| pair)
            .collect();
        let declared: BTreeSet<(usize, usize)> = layout.borders().into_iter().collect();
        assert_eq!(edges, declared, "{layout:?}");
        let regions: BTreeSet<usize> = (0..ROWS)
            .flat_map(|r| (0..COLUMNS).map(move |c| (c, r)))
            .map(|(c, r)| at(c, r))
            .collect();
        assert_eq!(regions, (0..layout.regions()).collect(), "{layout:?}");
    }
}

#[test]
fn every_flag_shows_two_to_four_colours_that_read_where_they_meet() {
    for seed in 0..SEEDS {
        let flag = Flag::generate(seed);
        let layout = flag.layout();
        assert_eq!(flag.fills().len(), layout.regions(), "{seed}: {flag}");
        assert!((2..=4).contains(&flag.scheme().len()), "{seed}: {flag}");
        for (a, b) in layout.borders() {
            let (a, b) = (flag.fills()[a], flag.fills()[b]);
            assert!(a.reads_against(b), "{seed}: {a} meets {b} in {flag}");
        }
        assert_eq!(Flag::new(layout, flag.fills().to_vec()), Some(flag));
    }
}

#[test]
fn seeds_cover_every_layout_colour_count_and_tincture() {
    let flags: Vec<Flag> = (0..SEEDS).map(Flag::generate).collect();
    for layout in Layout::ALL {
        let share = flags.iter().filter(|f| f.layout() == layout).count() as f32 / SEEDS as f32;
        assert!(share > 0.02, "{layout:?} flies on {share:.3} of seeds");
    }
    for count in 2..=4 {
        let share =
            flags.iter().filter(|f| f.scheme().len() == count).count() as f32 / SEEDS as f32;
        assert!(share > 0.12, "{count} colours fly on {share:.3} of seeds");
    }
    for t in Tincture::ALL {
        assert!(
            flags.iter().filter(|f| f.fills().contains(&t)).count() as u64 > SEEDS / 10,
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
fn coverage_adds_up_and_follows_the_layout() {
    let quartered = Flag::new(Quarters, vec![Gold, Black, Black, Gold]).unwrap();
    let cover = quartered.coverage();
    assert!((cover.iter().sum::<f32>() - 1.).abs() < 1e-3);
    assert!((cover[Gold as usize] - 0.5).abs() < 1e-3);
    assert!((cover[Black as usize] - 0.5).abs() < 1e-3);
    let canton = Flag::new(Canton(1), vec![White, Navy]).unwrap();
    let (depth, length) = Canton(1).canton();
    assert!((canton.coverage()[White as usize] - depth * length).abs() < 0.01);
}

#[test]
fn painting_lays_each_region_in_its_own_colour() {
    let (width, height) = (112, 40);
    for seed in 0..40 {
        let flag = Flag::generate(seed);
        let pixels = flag.paint(width, height);
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        // Away from the edges every pixel is exactly its region's colour.
        for y in (2..height - 2).step_by(3) {
            for x in (2..width - 2).step_by(5) {
                let at = |dx: f32, dy: f32| {
                    flag.tincture(
                        (x as f32 + dx) / width as f32,
                        (y as f32 + dy) / height as f32,
                    )
                };
                // Every region is convex, so a pixel whose corners agree lies inside one.
                let centre = at(0.5, 0.5);
                if [at(0., 0.), at(1., 0.), at(0., 1.), at(1., 1.)]
                    .iter()
                    .all(|&t| t == centre)
                {
                    let i = ((y * width + x) * 4) as usize;
                    let [r, g, b] = centre.srgb();
                    assert_eq!(
                        &pixels[i..i + 4],
                        &[r, g, b, 255],
                        "{seed}: {flag} at {x},{y}"
                    );
                }
            }
        }
    }
}

#[test]
fn flags_describe_themselves() {
    let flag = |layout, fills: &[Tincture]| Flag::new(layout, fills.to_vec()).unwrap().to_string();
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
}
