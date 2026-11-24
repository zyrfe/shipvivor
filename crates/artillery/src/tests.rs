use super::*;
use ArmorGrade::{Light, Reinforced};
use ContactAngle::{Direct, Glancing};
use Impact::{Bounced, Embedded, PassedThrough, Penetrated};
use ShellClass::{Breaker, Standard};

#[test]
fn intact_plate_outcomes_cover_each_shell_grade_and_angle() {
    for (shell, armor, angle, expected) in [
        (Standard, Light, Direct, Penetrated),
        (Breaker, Light, Direct, Penetrated),
        (Standard, Reinforced, Direct, Embedded),
        (Breaker, Reinforced, Direct, Penetrated),
        (Standard, Light, Glancing, Bounced),
        (Breaker, Light, Glancing, Bounced),
        (Standard, Reinforced, Glancing, Bounced),
        (Breaker, Reinforced, Glancing, Bounced),
    ] {
        let mut panel = ArmorPanel::new(armor, 3, 2);
        let contact = [(1, 0), (2, 1)];
        assert_eq!(
            panel.resolve_hit(shell, angle, &contact),
            Ok(expected),
            "{shell:?} / {armor:?} / {angle:?}"
        );
        for row in 0..2 {
            for column in 0..3 {
                let cell = (column, row);
                assert_eq!(
                    panel.is_breached(cell),
                    expected == Penetrated && contact.contains(&cell)
                );
            }
        }
    }
}

#[test]
fn follow_up_requires_clearance_for_the_whole_footprint() {
    let mut panel = ArmorPanel::new(Reinforced, 4, 1);
    assert_eq!(
        panel.resolve_hit(Breaker, Direct, &[(0, 0), (1, 0)]),
        Ok(Penetrated)
    );
    assert_eq!(
        panel.resolve_hit(Standard, Glancing, &[(1, 0), (0, 0)]),
        Ok(PassedThrough)
    );

    let rim = [(1, 0), (2, 0)];
    assert_eq!(panel.resolve_hit(Standard, Direct, &rim), Ok(Embedded));
    assert_eq!(panel.resolve_hit(Breaker, Glancing, &rim), Ok(Bounced));
    assert!(!panel.is_breached((2, 0)));
    assert!(panel.is_breached((1, 0)));
    assert_eq!(panel.resolve_hit(Breaker, Direct, &rim), Ok(Penetrated));
    assert_eq!(
        panel.resolve_hit(Standard, Direct, &[(2, 0), (1, 0), (0, 0)]),
        Ok(PassedThrough)
    );
    assert!(!panel.is_breached((3, 0)));
}

#[test]
fn invalid_contact_preserves_existing_breaches_without_opening_new_cells() {
    let mut panel = ArmorPanel::new(Light, 3, 2);
    assert_eq!(
        panel.resolve_hit(Breaker, Direct, &[(2, 1)]),
        Ok(Penetrated)
    );
    assert_eq!(
        panel.resolve_hit(Breaker, Direct, &[]),
        Err(ContactError::EmptyFootprint)
    );
    for outside in [(3, 0), (0, 2), (u16::MAX, u16::MAX)] {
        assert_eq!(
            panel.resolve_hit(Breaker, Direct, &[(2, 1), (1, 1), outside]),
            Err(ContactError::OutsidePanel(outside))
        );
        assert!(!panel.is_breached((1, 1)));
        assert!(panel.is_breached((2, 1)));
    }
}

#[test]
fn contact_order_and_duplicate_cells_do_not_change_the_breach() {
    let original = ArmorPanel::new(Light, 3, 2);
    let mut first = original.clone();
    let mut second = original.clone();
    assert_eq!(
        first.resolve_hit(Standard, Direct, &[(1, 0), (2, 1), (1, 0)]),
        Ok(Penetrated)
    );
    assert_eq!(
        second.resolve_hit(Standard, Direct, &[(2, 1), (1, 0)]),
        Ok(Penetrated)
    );
    for row in 0..2 {
        for column in 0..3 {
            let cell = (column, row);
            assert_eq!(first.is_breached(cell), second.is_breached(cell));
            assert!(!original.is_breached(cell));
        }
    }
}

#[test]
fn heavy_plate_requires_a_matching_grade_and_grazing_still_skips() {
    use ArmorGrade::Capital;
    for (round, angle, want) in [
        (Standard, Direct, Embedded),
        (Breaker, Direct, Embedded),
        (ShellClass::Heavy, Direct, Penetrated),
        (ShellClass::Heavy, Glancing, Bounced),
    ] {
        assert_eq!(
            ArmorPanel::new(Capital, 2, 2).resolve_hit(round, angle, &[(1, 0)]),
            Ok(want)
        );
    }
}

#[test]
fn larger_round_must_open_its_own_clearance() {
    let mut p = ArmorPanel::new(ArmorGrade::Capital, 3, 3);
    assert_eq!(
        p.resolve_round(ShellClass::Heavy, Direct, &[(1, 1)], 0),
        Ok(Penetrated)
    );
    assert_eq!(
        p.resolve_round(Standard, Direct, &[(1, 1)], 2),
        Ok(Embedded)
    );
    assert_eq!(
        p.resolve_round(Standard, Glancing, &[(1, 1)], 0),
        Ok(PassedThrough)
    );
    assert_eq!(
        p.resolve_round(ShellClass::Heavy, Direct, &[(1, 1)], 2),
        Ok(Penetrated)
    );
    assert_eq!(
        p.resolve_round(Standard, Glancing, &[(1, 1)], 1),
        Ok(PassedThrough)
    );
}

#[test]
fn he_bursts_instead_of_glancing_and_ap_goes_through_light_plate() {
    use ArmorGrade::Capital;
    for (ammo, shell, armor, angle, want) in [
        (Ammo::He, Standard, Light, Glancing, Penetrated),
        (Ammo::He, Standard, Light, Direct, Penetrated),
        (Ammo::He, Standard, Reinforced, Glancing, Embedded),
        (Ammo::He, Standard, Capital, Direct, Embedded),
        (Ammo::Ap, Breaker, Light, Direct, Impact::Overpenetrated),
        (Ammo::Ap, Breaker, Light, Glancing, Bounced),
        (Ammo::Ap, Breaker, Reinforced, Direct, Penetrated),
        (Ammo::Ap, Breaker, Capital, Direct, Embedded),
        (Ammo::Ap, ShellClass::Heavy, Capital, Direct, Penetrated),
    ] {
        let mut panel = ArmorPanel::new(armor, 3, 2);
        assert_eq!(
            panel.resolve_ammo(ammo, shell, angle, &[(1, 0)], 0),
            Ok(want),
            "{ammo:?} {shell:?} / {armor:?} / {angle:?}"
        );
        assert_eq!(panel.is_breached((1, 0)), want.opened());
    }
    assert_eq!(Impact::Penetrated.dealt(5), 5);
    assert_eq!(Impact::Overpenetrated.dealt(5), 3);
    assert_eq!(Impact::Embedded.dealt(5), 0);
}

#[test]
fn he_stays_light_whatever_the_bore_and_ap_starts_a_grade_heavier() {
    let he = MainShell {
        bore: 4,
        caps: 3,
        ..MainShell::STOCK
    };
    assert_eq!(he.penetration(), Standard);
    let ap = MainShell {
        ammo: Ammo::Ap,
        ..MainShell::STOCK
    };
    assert_eq!(ap.penetration(), Breaker);
    assert_eq!(ap.at_contact(1.), Standard);
    let capped = MainShell { caps: 1, ..ap };
    assert_eq!(capped.penetration(), ShellClass::Heavy);
    assert_eq!(capped.at_contact(1.), Breaker);
    assert_eq!(
        MainShell { caps: 2, ..ap }.at_contact(1.),
        ShellClass::Heavy
    );
}
