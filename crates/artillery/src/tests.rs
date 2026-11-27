use super::*;
use ArmorGrade::{Capital, Light, Reinforced};
use ContactAngle::{Direct, Glancing};
use Impact::{Bounced, Embedded, Overpenetrated, PassedThrough, Penetrated};

fn round(calibre: f32, penetration: f32) -> Round {
    Round {
        calibre,
        penetration,
    }
}

#[test]
fn a_round_opens_plate_no_thicker_than_its_penetration_and_holes_it() {
    for (penetration, grade, want) in [
        (4., Light, Penetrated),
        (4., Reinforced, Embedded),
        (6., Reinforced, Penetrated),
        (11., Capital, Embedded),
        (12., Capital, Penetrated),
    ] {
        let mut panel = ArmorPanel::new(grade, 3, 2);
        let strike = panel.strike(round(4., penetration), Direct, (1, 0), 0.99);
        assert_eq!(
            strike.map(|s| s.impact),
            Ok(want),
            "{penetration} on {grade:?}"
        );
        assert_eq!(panel.is_breached((1, 0)), want == Penetrated);
        // Every later round goes in through the hole, however light.
        if want == Penetrated {
            let after = panel.strike(round(1., 0.5), Glancing, (1, 0), 0.);
            assert_eq!(
                after,
                Ok(Strike {
                    impact: PassedThrough,
                    share: 1.
                })
            );
        }
    }
}

#[test]
fn plate_that_holds_a_round_still_lets_some_of_it_through_and_wears() {
    let mut panel = ArmorPanel::new(Capital, 4, 3);
    // A four-inch round on capital plate does a little, and one that nearly opens it
    // does much more.
    let light = panel.strike(round(4., 4.), Direct, (0, 0), 0.).unwrap();
    let near = ArmorPanel::new(Capital, 4, 3)
        .strike(round(11., 11.5), Direct, (0, 0), 0.)
        .unwrap();
    assert_eq!((light.impact, near.impact), (Embedded, Embedded));
    let (least, _) = HELD_SHARE;
    assert!(light.share > least && light.share < 0.2, "{}", light.share);
    assert!(near.share > 0.5 && near.share < 0.6, "{}", near.share);
    // The plate there is thinner now, and so is the plate beside it, by half as much.
    let worn = panel.worn((0, 0));
    assert!(worn > 0.05 && worn < 0.1, "{worn}");
    assert!((panel.worn((1, 0)) - 0.5 * worn).abs() < 1e-6);
    assert!((panel.worn((0, 1)) - 0.5 * worn).abs() < 1e-6);
    assert_eq!(panel.worn((1, 1)), 0.);
    // Keep at the same place and the plate gives way. Each round does a little more
    // than the one before as it thins.
    let mut shares = vec![light.share];
    let mut rounds = 1;
    while !panel.is_breached((0, 0)) {
        let s = panel.strike(round(4., 4.), Direct, (0, 0), 0.).unwrap();
        shares.push(s.share);
        rounds += 1;
        assert!(rounds < 20, "the plate never gave way");
    }
    assert!(
        rounds > 5,
        "capital plate fell to {rounds} four-inch rounds"
    );
    assert!(shares.windows(2).all(|w| w[1] >= w[0]), "{shares:?}");
    assert_eq!(*shares.last().unwrap(), 1.);
}

#[test]
fn a_shallow_round_skips_off_unless_it_is_wide_enough_to_bite() {
    let mut panel = ArmorPanel::new(Reinforced, 3, 2);
    let skip = panel.strike(round(6., 8.), Glancing, (1, 1), 0.).unwrap();
    assert_eq!(skip.impact, Bounced);
    assert!((skip.share - RICOCHET_SHARE).abs() < 1e-6);
    assert!(!panel.is_breached((1, 1)) && panel.worn((1, 1)) > 0.);
    // A twelve-inch round overmatches six inches of plate and bites at any angle.
    let bite = panel
        .strike(round(12., 12.), Glancing, (0, 0), 0.99)
        .unwrap();
    assert_eq!(bite.impact, Penetrated);
    // Every main-battery round overmatches light plate.
    let light = ArmorPanel::new(Light, 2, 2).strike(round(4., 3.), Glancing, (0, 0), 0.99);
    assert_eq!(light.map(|s| s.impact), Ok(Penetrated));
}

#[test]
fn only_a_round_with_plenty_to_spare_may_go_clean_through() {
    let through = |penetration, grade, luck| {
        ArmorPanel::new(grade, 2, 2)
            .strike(round(12., penetration), Direct, (0, 0), luck)
            .unwrap()
    };
    // Not enough to spare: it bursts behind the plate however lucky.
    assert_eq!(through(5.9, Light, 0.).impact, Penetrated);
    assert_eq!(through(20., Capital, 0.).impact, Penetrated);
    // With plenty, a lucky one goes clean through and does less behind it.
    let lucky = through(12., Light, 0.);
    assert_eq!(lucky.impact, Overpenetrated);
    assert_eq!(lucky.share, OVERPENETRATED_SHARE);
    assert_eq!(through(12., Light, 0.9).impact, Penetrated);
    assert!(Overpenetrated.opened());
    // What is left after going in and out of the plate.
    let left = round(12., 12.).beyond(Light.inches());
    assert_eq!(left.penetration, 8.);
}

#[test]
fn a_contact_outside_the_panel_leaves_it_untouched() {
    let mut panel = ArmorPanel::new(Light, 3, 2);
    assert_eq!(
        panel
            .strike(round(4., 4.), Direct, (2, 1), 0.9)
            .map(|s| s.impact),
        Ok(Penetrated)
    );
    for outside in [(3, 0), (0, 2), (u16::MAX, u16::MAX)] {
        assert_eq!(
            panel.strike(round(4., 4.), Direct, outside, 0.9),
            Err(OutsidePanel(outside))
        );
        assert!(!panel.is_breached((1, 1)));
        assert!(panel.is_breached((2, 1)));
    }
}

#[test]
fn bore_caps_and_velocity_drive_a_round_through_heavier_plate_and_range_costs_it() {
    let stock = MainShell::STOCK;
    assert_eq!(stock.penetration(), 4.);
    let eight = MainShell { bore: 4, ..stock };
    let capped = MainShell { caps: 2, ..stock };
    let fast = MainShell {
        velocity: 3,
        ..stock
    };
    assert_eq!(eight.penetration(), 8.);
    assert_eq!(capped.penetration(), 6.);
    assert!(fast.penetration() > 5. && fast.penetration() < 5.5);
    // Every round loses some of it as it flies, and a faster round gets there sooner,
    // so it keeps more at the same distance.
    assert!(stock.penetration_at(1.) < stock.penetration());
    let distance = 500.;
    let at =
        |shell: MainShell| shell.penetration_at(distance / shell.speed()) / shell.penetration();
    assert!(at(fast) > at(stock));
    // Stock rounds open light plate out to their reach, and nothing heavier.
    let far = stock.round_at(stock.reach() / stock.speed());
    assert!(far.opens(Light) && !stock.round_at(0.).opens(Reinforced));
    assert!(eight.round_at(0.5).opens(Reinforced));
    let eighteen = MainShell { bore: 14, ..stock };
    assert!(eighteen.round_at(1.).opens(Capital));
}

#[test]
fn a_heavier_gunhouse_trains_more_slowly_and_training_gear_wins_it_back() {
    let rates: Vec<f32> = [4., 6., 8., 12., 15., 18.]
        .into_iter()
        .map(|inches| training_rate(inches, 0))
        .collect();
    assert!((rates[0] - 1.).abs() < 1e-6, "stock four-inch mount");
    assert!(rates.windows(2).all(|pair| pair[1] < pair[0]));
    // Four ranks bring a sixteen-inch mount back to the stock pace, eight take it half
    // again as fast, and no more ranks fit.
    assert!((training_rate(16., 4) - 1.).abs() < 1e-6);
    assert!((training_rate(16., MOST_TRAINING_GEAR) - 1.5).abs() < 1e-6);
    assert_eq!(
        training_rate(8., MOST_TRAINING_GEAR + 3),
        training_rate(8., MOST_TRAINING_GEAR)
    );
}

#[test]
fn a_faster_round_carries_farther() {
    let stock = MainShell::STOCK;
    assert!((stock.reach() - 780.).abs() < 1e-3);
    let reaches: Vec<f32> = (0..5)
        .map(|velocity| MainShell { velocity, ..stock }.reach())
        .collect();
    assert!(reaches.windows(2).all(|pair| pair[1] > pair[0]));
    // A heavier bore alone does not change the reach.
    assert_eq!(MainShell { bore: 8, ..stock }.reach(), stock.reach());
}
