use super::Component::*;
use super::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}
fn berth(module: usize, index: usize) -> Berth {
    Berth { module, index }
}
/// Breakwater's hull with a stock crew, so the times in a test are the repair
/// mechanics' own, not her crew's slower pace.
fn stock_crew() -> Design {
    let mut design = Design::breakwater();
    design.repair = 1.;
    design
}

#[test]
fn her_crew_mends_and_pumps_at_two_thirds_of_a_stock_crews_pace() {
    let mut ship = Ship::new(Design::breakwater());
    let mut stock = Ship::new(stock_crew());
    for ship in [&mut ship, &mut stock] {
        ship.strike_hull(7.5, 10.);
        ship.tend(60.);
    }
    let mended = |ship: &Ship| ship.compartments()[1].structure() - 8.;
    assert!(near(mended(&stock), 60. * 0.002 * 18.));
    assert!(near(
        mended(&ship),
        Design::BREAKWATER_REPAIR * mended(&stock)
    ));
}

#[test]
fn moving_a_compartment_moves_its_gun_damage_and_repair_party() {
    let mut ship = Ship::new(Design::breakwater());
    ship.strike_fitting(berth(1, 0), 4.);
    assert!(ship.assign(Some(Task::Fitting(berth(1, 0)))));
    ship.strike_hull(7.5, 4.);
    let before = ship.compartments()[1].structure();
    ship.move_module(1, 3).unwrap();
    assert_eq!(ship.party(), Some(Task::Fitting(berth(3, 0))));
    assert_eq!(ship.design().component(berth(3, 0)), Some(MainGun));
    assert_eq!(ship.compartments()[3].structure(), before);
    let new_slot = ship
        .design()
        .gun_stations()
        .into_iter()
        .find(|g| g.berth.module == 3)
        .unwrap();
    assert_eq!(ship.design().module_at(16. - new_slot.fore), 3);
    ship.move_module(3, 1).unwrap();
    assert_eq!(ship.party(), Some(Task::Fitting(berth(1, 0))));
    assert_eq!(ship.compartments()[1].structure(), before);
}

#[test]
fn each_turret_stays_inside_its_compartment_after_a_move_and_the_bridge_masks_its_bearing() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut design = Design::breakwater();
    let stock = design.gun_stations()[0];
    assert!(stock.bears(135_f32.to_radians()) && !stock.bears(145_f32.to_radians()));
    design.insert(2, Module::new(Frame::Straight, [])).unwrap();
    assert!(design.gun_stations()[0].bears(145_f32.to_radians()));
    design
        .replace_module(5, Module::new(Frame::BatteryGallery, [MainGun, MainGun]))
        .unwrap();
    for from in 1..design.modules().len() - 1 {
        for to in 1..design.modules().len() - 1 {
            let mut trial = design.clone();
            trial.move_module(from, to).unwrap();
            let guns = trial.gun_stations();
            for g in guns {
                assert_eq!(g.berth.module, trial.module_at(16. - g.fore));
                if g.paired() {
                    assert!(g.bears(g.abeam.signum() * FRAC_PI_2));
                    assert!(
                        !g.bears(0.) && !g.bears(PI) && !g.bears(-g.abeam.signum() * FRAC_PI_2)
                    );
                }
            }
        }
    }
    // Sandwich the centre gun between the bridge and the engine. Both ends are masked.
    design.move_module(1, 3).unwrap();
    let enclosed = design.gun_stations()[0];
    assert!(enclosed.bears(FRAC_PI_2) && enclosed.bears(-FRAC_PI_2));
    assert!(!enclosed.bears(0.) && !enclosed.bears(PI));

    // Put both guns ahead of the bridge. Only the front gun may shoot over the bow.
    let mut tandem = Design::breakwater();
    tandem.move_module(4, 2).unwrap();
    let guns = tandem.gun_stations();
    assert!(guns[0].bears(0.));
    assert!(!guns[1].bears(0.) && guns[1].bears(FRAC_PI_2));
    tandem
        .replace_module(1, Module::new(Frame::BatteryGallery, [MainGun, MainGun]))
        .unwrap();
    assert!(tandem.gun_stations()[2].bears(0.));
}
/// Run repairs for a number of seconds and collect what they brought back.
fn tend(ship: &mut Ship, seconds: usize) -> Vec<Mend> {
    (0..seconds).flat_map(|_| ship.tend(1.)).collect()
}

#[test]
fn stock_designs_come_out_at_the_figures_the_game_already_uses() {
    let cruiser = Design::breakwater().sheet();
    assert!(near(cruiser.length, 30.) && near(cruiser.beam, 5.5));
    assert!(near(cruiser.mass, 8.) && near(cruiser.structure, 100.));
    assert!(near(cruiser.top_speed, 13.6));
    assert!(near(Design::breakwater().turn_rate(), 0.38));
    assert_eq!(cruiser.deck_mounts, 10);

    let boat = Design::patrol_boat().sheet();
    assert!(near(boat.length, 15.) && near(boat.beam, 5.));
    assert!(near(boat.mass, 1.) && near(boat.structure, 4.));

    let capital = Design::battleship();
    assert!(near(capital.length(), 51.) && near(capital.structure(), 129.5));
    assert_eq!(capital.citadel_span(), Some(1..=5));
    assert_eq!(capital.armor(0), ArmorGrade::Light);
    assert_eq!(capital.armor(3), ArmorGrade::Capital);
    assert_eq!(capital.armor(6), ArmorGrade::Light);
    assert_eq!(Design::breakwater().citadel_span(), None);
    for (design, length, structure) in [
        (Design::freighter(), 30.1, 100. * 4. / 18.),
        (Design::cruiser(), 30.1, 100. * 5. / 18.),
        (Design::flagship(), 81., 172.),
    ] {
        assert!(near(design.length(), length) && near(design.structure(), structure));
    }
}

#[test]
fn each_enemy_gun_finds_a_berth_on_the_module_under_its_deck_station() {
    // Stations are metres aft of the bow, from each class's present gun positions.
    let guns = |design: Design, stations: &[f32]| design.berths_for(MainGun, stations);
    assert_eq!(guns(Design::patrol_boat(), &[3.8]), Some(vec![berth(0, 0)]));
    assert_eq!(
        guns(Design::cruiser(), &[7.3, 24.5]),
        Some(vec![berth(1, 0), berth(4, 0)])
    );
    assert_eq!(guns(Design::cruiser(), &[15.]), None);
    assert_eq!(
        guns(Design::battleship(), &[9.5, 18., 43.]),
        Some(vec![berth(1, 0), berth(2, 0), berth(5, 0)])
    );
    assert_eq!(
        Design::battleship().berths_for(Director, &[27.]),
        Some(vec![berth(3, 0)])
    );
    let turrets = guns(
        Design::flagship(),
        &[12., 61., 36., 25., 50., 72., 25., 50., 61., 36.],
    )
    .unwrap();
    for (i, a) in turrets.iter().enumerate() {
        assert!(turrets[i + 1..].iter().all(|b| a != b), "one turret each");
    }
    assert_eq!((turrets[0], turrets[5]), (berth(1, 0), berth(7, 0)));

    // The flagship's turrets take 7 points each whatever her hull is worth: four
    // stock rounds' worth once they are through the plate, and three of the heavier ones.
    let mut flagship = Ship::new(Design::flagship());
    for _ in 0..3 {
        assert_eq!(flagship.strike_fitting(turrets[3], 2.), []);
    }
    assert_eq!(
        flagship.strike_fitting(turrets[3], 2.),
        [Loss::Knocked(turrets[3])]
    );
}

#[test]
fn fittings_stand_on_their_modules_with_side_mounts_paired_across_the_hull() {
    let design = Design::breakwater();
    assert_eq!(design.span(3), (15., 20.));
    assert_eq!(
        [-3., 7.5, 29.9, 400.].map(|aft| design.module_at(aft)),
        [0, 1, 5, 5]
    );
    let at = |b: Berth| {
        let fitting = design
            .fittings()
            .into_iter()
            .find(|f| f.berth == b)
            .unwrap();
        (fitting.component, fitting.aft, fitting.abeam)
    };
    assert_eq!(at(berth(1, 0)), (MainGun, 7.5, 0.));
    assert_eq!(at(berth(4, 0)), (MainGun, 22.5, 0.));
    assert_eq!(at(berth(2, 0)), (Director, 12.5, 0.));
    let (port, starboard) = (at(berth(2, 1)), at(berth(2, 2)));
    assert_eq!((port.0, port.1), (CloseGun, 12.5));
    assert!(near(port.2, -2.475) && near(starboard.2, 2.475));
    assert_eq!(at(berth(3, 2)), (Engine, 17.5, 0.));
}

#[test]
fn the_dock_inserts_and_removes_modules_and_refuses_what_cannot_be_built() {
    let stock = Design::breakwater();
    let mut design = stock.clone();
    design
        .insert(3, Module::new(Frame::Straight, [MainGun]))
        .unwrap();
    let (before, after) = (stock.sheet(), design.sheet());
    assert!(near(after.length, before.length + 5.));
    assert_eq!(after.deck_mounts, before.deck_mounts + 1);
    assert!(near(after.structure, 118.));
    assert!(after.top_speed < before.top_speed && after.top_speed > 12.5);
    assert!(after.turn_radius > before.turn_radius);
    assert_eq!(design.count(MainGun), 3);
    assert_eq!(design.remove(3).unwrap().fit, [MainGun]);
    assert_eq!(design, stock);

    // A second engine in a free bay buys a quarter more speed.
    let added = design.fit(1, Engine).unwrap();
    assert_eq!(added, berth(1, 1));
    assert!(design.top_speed() > 16.);
    assert_eq!(design.strip(added), Ok(Engine));
    assert_eq!(design, stock);

    assert_eq!(design.remove(0), Err(Fault::Ends));
    assert_eq!(design.remove(5), Err(Fault::Ends));
    assert_eq!(design.remove(3), Err(Fault::NoEngine));
    assert_eq!(design.remove(9), Err(Fault::Missing));
    assert_eq!(
        design.insert(2, Module::new(Frame::Bow, [])),
        Err(Fault::Ends)
    );
    assert_eq!(
        design.fit(1, MainGun),
        Err(Fault::Crowded {
            module: 1,
            slot: Slot::Centre
        })
    );
    assert_eq!(
        design.fit(1, CloseGun).unwrap_err(),
        Fault::Crowded {
            module: 1,
            slot: Slot::Side
        }
    );
    assert_eq!(design, stock, "a refused change leaves the design alone");
    assert_eq!(Design::new("Raft", vec![]).unwrap_err(), Fault::Ends);
    assert_eq!(
        Design::new("Hulk", vec![Module::new(Frame::Boat, [MainGun])]).unwrap_err(),
        Fault::NoEngine
    );
}

#[test]
fn a_round_reaches_the_engine_only_through_the_middle_of_the_machinery_space() {
    let engine = berth(3, 2);
    // Twelve is enough to knock her engine out, and the machinery space runs from 15 m
    // to 20 m aft of the bow with her engine in its middle.
    let struck = |aft: f32, abeam: f32, reach: Reach| {
        let mut ship = Ship::new(Design::breakwater());
        let hull = ship.hull();
        let losses = ship.strike_into(aft, abeam, 12., None, reach);
        assert!(near(ship.hull(), hull - 12.), "the module takes the blow");
        (losses, ship.state(engine))
    };
    let spared = (vec![], State::Working);
    let knocked = (vec![Loss::Knocked(engine)], State::Out);
    // Burst in the funnel, above the deck.
    assert_eq!(struck(16.5, 0., Reach::Track(None)), spared);
    // Burst in the side passage before getting in to her.
    let passage = Reach::Track(Some([(17., 2.75), (17., 2.)]));
    assert_eq!(struck(17., 2.75, passage), spared);
    // Holed at the bulkhead, clear of her.
    let bulkhead = Reach::Track(Some([(15.2, 2.75), (15.2, -0.25)]));
    assert_eq!(struck(15.2, 2.75, bulkhead), spared);
    // Through the side and on into the engine room.
    let through = Reach::Track(Some([(17., 2.75), (17., -0.25)]));
    assert_eq!(struck(17., 2.75, through), knocked);
    // Plunging through the deck above her.
    let plunging = Reach::Track(Some([(16.8, 0.5), (17.2, 0.3)]));
    assert_eq!(struck(16.8, 0.5, plunging), knocked);
    // A ram, an iceberg or a bomb reaches every bay of the module it lands on.
    assert_eq!(struck(19.8, 2.7, Reach::Bays), knocked);
}

#[test]
fn breakwaters_stock_engine_takes_twelve_damage_to_knock_out() {
    let mut ship = Ship::new(Design::breakwater());
    let engine = berth(3, 2);
    assert_eq!(ship.strike_hull(17., 5.), []);
    assert_eq!(ship.state(engine), State::Working);
    assert_eq!(ship.strike_hull(17., 5.), []);
    assert_eq!(ship.state(engine), State::Damaged);
    assert!(near(ship.propulsion(), 0.5));
    assert!(!ship.on_reserve_power());
    assert_eq!(ship.strike_hull(17., 2.), [Loss::Knocked(engine)]);
    assert_eq!(ship.propulsion(), Ship::RESERVE_SPEED);
    assert!(ship.afloat() && !ship.compartments()[3].wrecked());
    assert!(near(ship.hull(), 88.));
    // The guns stand on other modules and fight on.
    assert!(
        ship.fitted(MainGun)
            .all(|(_, state)| state == State::Working)
    );
    assert_eq!(ship.state(berth(3, 0)), State::Working);

    // Wrecking the module takes the racks on its deck with it.
    assert_eq!(ship.strike_hull(17., 6.), [Loss::Wrecked(3)]);
    assert_eq!(ship.state(berth(3, 0)), State::Out);
    assert!(ship.on_reserve_power());
}

#[test]
fn a_wrecked_module_soaks_up_fire_and_passes_half_to_its_neighbours() {
    let mut ship = Ship::new(Design::breakwater());
    ship.strike_hull(17., 30.);
    assert!(ship.compartments()[3].wrecked());
    assert!(near(ship.hull(), 82.), "the excess is wasted");
    assert_eq!(ship.strike_hull(17., 10.), []);
    let left = |ship: &Ship, module: usize| ship.compartments()[module].structure();
    assert!(near(left(&ship, 2), 15.5) && near(left(&ship, 4), 15.5));
    // At the end of the hull there is only one neighbour to take it.
    ship.strike_hull(1., 14.);
    ship.strike_hull(1., 10.);
    assert!(near(left(&ship, 1), 13.));
}

/// A round wider than Breakwater's stock plate.
const SIX_INCH: Option<f32> = Some(6.);

/// Hits of `damage` on module 1 of a fresh Breakwater, cycling through `points` as
/// (metres aft, metres abeam), until it floods. Returns how many it took.
fn hits_to_flood(points: &[(f32, f32)], damage: f32) -> usize {
    let mut ship = Ship::new(Design::breakwater());
    (1..100)
        .find(|hit| {
            let (aft, abeam) = points[(hit - 1) % points.len()];
            ship.strike_side(aft, abeam, damage, SIX_INCH);
            ship.compartments()[1].wrecked()
        })
        .unwrap()
}

#[test]
fn hammering_one_stretch_of_plating_floods_a_module_sooner_than_spreading_the_same_fire() {
    let ship = Ship::new(Design::breakwater());
    // Module 1 runs from 5 to 10 metres aft: its fore half, then its after half.
    assert_eq!(ship.panel_at(6., -2.), (1, Panel::new(Side::Port, true)));
    assert_eq!(
        ship.panel_at(9., 2.),
        (1, Panel::new(Side::Starboard, false))
    );
    let focused = hits_to_flood(&[(6., -2.)], 2.);
    let spread = hits_to_flood(&[(6., -2.), (9., -2.), (6., 2.), (9., 2.)], 2.);
    // On sound plate, 18 structure goes in nine hits of 2.
    assert_eq!((focused, spread), (6, 9));
}

#[test]
fn turning_the_other_beam_brings_fresh_plating_to_bear() {
    let mut ship = Ship::new(Design::breakwater());
    let port = Panel::new(Side::Port, true);
    let lost = |ship: &mut Ship, abeam: f32| {
        let before = ship.compartments()[1].structure();
        ship.strike_side(6., abeam, 3., SIX_INCH);
        before - ship.compartments()[1].structure()
    };
    // Sound plate holds a shell to its damage, and is worn by all of it.
    assert!(near(lost(&mut ship, -2.), 3.));
    assert!(near(ship.compartments()[1].plating(port), 2. / 3.));
    assert!(near(lost(&mut ship, -2.), 4.));
    let battered = lost(&mut ship, -2.);
    assert!(near(battered, 5.));
    assert_eq!(ship.compartments()[1].plating(port), 0.);
    for panel in &Panel::ALL[1..] {
        assert_eq!(ship.compartments()[1].plating(*panel), 1.);
    }
    assert!(
        near(lost(&mut ship, 2.), 3.),
        "the starboard plate is sound"
    );
}

#[test]
fn plating_mends_with_the_crew_and_the_dock_replates_her() {
    let mut ship = Ship::new(stock_crew());
    let port = Panel::new(Side::Port, true);
    for _ in 0..3 {
        ship.strike_side(6., -2., 3., SIX_INCH);
    }
    assert_eq!(ship.compartments()[1].plating(port), 0.);
    assert!(!ship.compartments()[1].wrecked());
    // The crew restores 0.2% of the plate each second, the pace it mends the hull at.
    ship.tend(50.);
    assert!(near(ship.compartments()[1].plating(port), 0.1));
    ship.dock(0.);
    assert_eq!(ship.compartments()[1].plating(port), 1.);
}

#[test]
fn breakwater_founders_with_four_of_her_six_modules_wrecked() {
    let mut ship = Ship::new(Design::breakwater());
    assert_eq!(ship.strike_hull(1., 14.), [Loss::Wrecked(0)]);
    assert_eq!(ship.strike_hull(7.5, 18.), [Loss::Wrecked(1)]);
    assert!(ship.afloat());
    assert_eq!(ship.flooding(), (2, 4));
    assert!(!ship.in_peril());
    assert_eq!(Ship::new(Design::battleship()).flooding(), (0, 3));
    assert_eq!(Ship::new(Design::patrol_boat()).flooding(), (0, 1));
    assert!(
        near(ship.propulsion(), 1. - 1.8 * 10. / 30.),
        "flooding drags"
    );
    assert_eq!(ship.strike_hull(12.5, 18.), [Loss::Wrecked(2)]);
    assert!(
        ship.afloat() && ship.in_peril(),
        "one more module sinks her"
    );
    assert_eq!(
        ship.strike_hull(27., 14.),
        [Loss::Wrecked(5), Loss::Foundered]
    );
    assert!(!ship.afloat());
    assert_eq!(ship.propulsion(), 0.);
    assert!(!ship.on_reserve_power());
    assert_eq!(ship.gun_train_rate(), 0.);
    assert_eq!(ship.strike_hull(22., 50.), [], "she is already gone");
    assert!(ship.tend(60.).is_empty() && !ship.assign(Some(Task::Module(4))));
}

#[test]
fn a_citadel_ship_shrugs_off_wrecked_ends_and_dies_by_her_vitals() {
    let mut ship = Ship::new(Design::battleship());
    assert_eq!(ship.strike_hull(1., 50.), [Loss::Wrecked(0)]);
    assert_eq!(ship.strike_hull(50., 50.), [Loss::Wrecked(6)]);
    assert!(ship.afloat(), "the ends are outside the citadel");

    let (fore, aft) = (berth(3, 1), berth(4, 0));
    assert_eq!(ship.strike_hull(25., 12.), [Loss::Knocked(fore)]);
    // One engine of two, dragging her flooded ends.
    let drag = 1. - 1.8 * ship.water_aboard();
    assert!(near(ship.propulsion(), 0.5 * drag) && drag > 0.4, "{drag}");
    assert_eq!(ship.strike_hull(25., 12.), [Loss::Wrecked(3)]);
    assert_eq!(
        ship.strike_hull(32., 22.),
        [Loss::Knocked(aft), Loss::Wrecked(4)]
    );
    assert!(ship.afloat() && ship.propulsion() == 0.);
    // The third of her five citadel modules is more than she can lose.
    assert_eq!(
        ship.strike_hull(18., 22.),
        [Loss::Wrecked(2), Loss::Foundered]
    );
}

#[test]
fn a_magazine_hit_wrecks_its_module_and_blasts_the_ones_beside_it() {
    let mut ship = Ship::new(Design::battleship());
    assert_eq!(
        ship.strike_hull(10., 13.),
        [
            Loss::Knocked(berth(1, 1)),
            Loss::Detonated(1),
            Loss::Wrecked(1),
        ]
    );
    let holds = ship.compartments();
    let (end, wide) = (holds[0].capacity(), holds[1].capacity());
    assert!(holds[1].wrecked() && holds[1].structure() == 0.);
    assert!(near(holds[0].structure(), end - 0.5 * wide));
    assert!(near(holds[2].structure(), 0.5 * wide));
    assert_eq!(ship.state(berth(1, 0)), State::Out, "the turret above it");
    assert!(ship.afloat());
}

#[test]
fn deck_fittings_are_knocked_out_by_direct_hits_that_spare_the_hull() {
    let mut ship = Ship::new(Design::breakwater());
    let (gun, director) = (berth(1, 0), berth(2, 0));
    assert_eq!(ship.strike_fitting(gun, 6.), []);
    assert_eq!(ship.state(gun), State::Damaged);
    assert_eq!(ship.strike_fitting(gun, 6.), [Loss::Knocked(gun)]);
    assert_eq!(ship.strike_fitting(gun, 6.), []);
    assert_eq!(ship.hull(), 100.);
    let states: Vec<State> = ship.fitted(MainGun).map(|(_, state)| state).collect();
    assert_eq!(states, [State::Out, State::Working]);

    assert_eq!(ship.reload_factor(), 1.);
    ship.strike_fitting(director, 5.);
    assert_eq!(ship.reload_factor(), 1.3);
    ship.strike_fitting(director, 5.);
    assert_eq!(ship.reload_factor(), 1.65);
    let boat = Ship::new(Design::patrol_boat());
    assert_eq!(boat.reload_factor(), 1., "no director to lose");
}

#[test]
fn a_patrol_boat_is_adrift_after_one_hit_and_sunk_by_the_next() {
    let mut boat = Ship::new(Design::patrol_boat());
    assert_eq!(boat.strike_hull(7., 2.), [Loss::Knocked(berth(0, 1))]);
    assert_eq!(boat.propulsion(), 0.);
    assert!(boat.afloat());
    assert_eq!(
        boat.state(berth(0, 0)),
        State::Working,
        "its gun still fires"
    );
    // Nobody aboard can restart the engine.
    assert!(boat.triage().is_none() && !boat.assign(Some(Task::Fitting(berth(0, 1)))));
    assert!(tend(&mut boat, 600).is_empty());
    assert_eq!(boat.propulsion(), 0.);
    assert_eq!(
        boat.strike_hull(7., 2.),
        [Loss::Wrecked(0), Loss::Foundered]
    );
}

#[test]
fn the_crew_mends_slowly_and_only_up_to_what_the_damage_left_sound() {
    let mut ship = Ship::new(stock_crew());
    ship.strike_hull(7.5, 10.);
    let hold = |ship: &Ship| {
        let hold = &ship.compartments()[1];
        (hold.structure(), hold.ceiling())
    };
    assert_eq!(hold(&ship), (8., 14.5));
    tend(&mut ship, 60);
    assert!(near(hold(&ship).0, 8. + 60. * 0.002 * 18.));
    tend(&mut ship, 600);
    assert!(near(hold(&ship).0, 14.5), "the rest waits for stores");
    assert!(near(ship.hull(), 96.5));

    // Stores are new material: they lift the ceiling with the structure.
    assert!(ship.patch(2.).is_empty());
    assert_eq!(hold(&ship), (16.5, 16.5));
    ship.patch(50.);
    assert_eq!(ship.hull(), 100.);
}

#[test]
fn a_battered_module_fills_unless_it_is_pumped() {
    // Five sixths of the forward gun's module shot away: it leaks faster than the crew
    // can pump, though nothing is wrecked yet.
    let battered = || {
        let mut ship = Ship::new(stock_crew());
        assert!(ship.strike_hull(7.5, 15.).is_empty());
        ship
    };
    let water = |ship: &Ship| ship.compartments()[1].water();
    let mut ship = battered();
    assert!(ship.compartments()[1].inflow() > PUMP);
    assert!(
        ship.compartments()[3].inflow() == 0.,
        "a sound module stays dry"
    );
    tend(&mut ship, 10);
    assert!(water(&ship) > 0.25 && water(&ship) < 1., "{}", water(&ship));
    assert!(ship.propulsion() < 1., "water aboard drags on her");
    // The party goes to it before it floods.
    assert_eq!(ship.triage(), Some(Task::Module(1)));
    // Left alone it floods within the minute, though the crew's own repairs slow the
    // leak, and the gun on it falls silent.
    tend(&mut ship, 50);
    assert!(ship.compartments()[1].wrecked());
    assert_eq!(ship.state(berth(1, 0)), State::Out);

    // Sent in time, the party pumps it out and patches it.
    let mut ship = battered();
    tend(&mut ship, 10);
    assert!(ship.assign(Some(Task::Module(1))));
    tend(&mut ship, 20);
    assert!(!ship.compartments()[1].wrecked());
    assert!(water(&ship) < 0.05, "{}", water(&ship));

    // Pumps twice as good keep it dry on their own.
    let mut ship = battered();
    ship.fit_pumps(2.);
    tend(&mut ship, 60);
    assert!(water(&ship) == 0., "{}", water(&ship));
}

#[test]
fn a_flooded_module_drags_on_her_until_the_dock_pumps_her_dry() {
    let mut ship = Ship::new(Design::breakwater());
    assert_eq!(ship.strike_hull(7.5, 18.), [Loss::Wrecked(1)]);
    assert_eq!(ship.compartments()[1].water(), 1.);
    assert!(near(ship.water_aboard(), 5. / 30.));
    assert!(near(ship.propulsion(), 1. - 1.8 * 5. / 30.));
    // Holed to nothing, it leaks faster than the crew alone can pump.
    assert!(ship.compartments()[1].inflow() > PUMP);
    assert_eq!(ship.dock(40.), [Mend::Refloated(1)]);
    assert_eq!(ship.compartments()[1].water(), 0.);
    assert_eq!(ship.propulsion(), 1.);
}

#[test]
fn a_torpedo_lets_the_sea_in_and_slows_her_until_it_is_pumped_out() {
    let mut ship = Ship::new(Design::breakwater());
    let water = |ship: &Ship| ship.compartments()[1].water();
    ship.breach(7.5);
    assert!(ship.compartments()[1].breached() && water(&ship) == 0.);
    tend(&mut ship, 3);
    // The sea comes in faster than the pumps can clear it, and she is slower for it.
    assert!(water(&ship) > 0.6, "{}", water(&ship));
    assert!(ship.propulsion() < 0.9, "{}", ship.propulsion());
    assert_eq!(ship.triage(), Some(Task::Module(1)));
    // Once it is shored her pumps clear it, and she has her speed again.
    tend(&mut ship, 2);
    assert!(!ship.compartments()[1].breached());
    let peak = water(&ship);
    assert!(peak < BREACH && !ship.compartments()[1].wrecked(), "{peak}");
    tend(&mut ship, 30);
    assert_eq!(water(&ship), 0.);
    assert_eq!(ship.propulsion(), 1.);

    // A second torpedo into a module not yet pumped out floods it.
    ship.breach(7.5);
    tend(&mut ship, 1);
    ship.breach(7.5);
    tend(&mut ship, 5);
    assert!(ship.compartments()[1].wrecked());

    // A ship that does not leak takes no water from it.
    let mut boat = Ship::new(Design::patrol_boat());
    boat.breach(1.);
    tend(&mut boat, 3);
    assert_eq!(boat.compartments()[0].water(), 0.);
}

#[test]
fn a_knocked_out_engine_waits_for_the_repair_party() {
    let mut ship = Ship::new(stock_crew());
    let engine = berth(3, 2);
    // A hit on the engine alone, so the machinery space stays tight.
    ship.strike_fitting(engine, 12.);
    assert!(tend(&mut ship, 300).is_empty());
    assert_eq!(ship.state(engine), State::Out);
    assert_eq!(ship.propulsion(), Ship::RESERVE_SPEED);

    assert_eq!(ship.triage(), Some(Task::Fitting(engine)));
    assert!(ship.assign(ship.triage()));
    assert!(tend(&mut ship, 9).is_empty());
    assert_eq!(tend(&mut ship, 2), [Mend::Restored(engine)]);
    assert_eq!(ship.state(engine), State::Damaged);
    assert!(near(ship.propulsion(), 0.5));
    assert!(!ship.on_reserve_power());
    assert_eq!(ship.gun_train_rate(), 1.);
    // The party stays until the job is finished, then stands down.
    tend(&mut ship, 30);
    assert_eq!(
        (ship.state(engine), ship.propulsion()),
        (State::Working, 1.)
    );
    assert_eq!(ship.party(), None);
    assert_eq!(ship.triage(), None, "nothing left that the crew can mend");
}

#[test]
fn the_repair_party_refloats_a_wrecked_module_and_its_fittings_return() {
    let mut ship = Ship::new(stock_crew());
    let gun = berth(1, 0);
    ship.strike_hull(7.5, 18.);
    assert_eq!(ship.state(gun), State::Out);
    assert!(tend(&mut ship, 120).is_empty());
    assert_eq!(ship.compartments()[1].structure(), 0.);

    assert!(!ship.assign(Some(Task::Fitting(gun))), "it is under water");
    assert!(!ship.assign(Some(Task::Module(4))), "nothing to do there");
    assert_eq!(ship.party(), None);
    assert_eq!(ship.triage(), Some(Task::Module(1)));
    assert!(ship.assign(Some(Task::Module(1))));
    // The party patches the hole until it leaks slower than her pumps clear it, and
    // she is pumped down to half full within a quarter of a minute or so.
    let refloated = (1..=30).find(|_| tend(&mut ship, 1) == [Mend::Refloated(1)]);
    assert!(
        refloated.is_some_and(|s| (10..=25).contains(&s)),
        "{refloated:?}"
    );
    assert_eq!(ship.state(gun), State::Working);
    tend(&mut ship, 60);
    let hold = &ship.compartments()[1];
    assert!(near(hold.structure(), 18. - 0.35 * 18.) && !hold.wrecked());
    assert_eq!(ship.party(), None);
}

#[test]
fn the_party_keeps_her_afloat_then_moving_then_fighting() {
    let mut ship = Ship::new(Design::battleship());
    let (gun, engine) = (berth(2, 0), berth(4, 0));
    ship.strike_hull(1., 50.);
    assert_eq!(ship.triage(), Some(Task::Module(0)));
    ship.strike_fitting(gun, 50.);
    assert_eq!(ship.triage(), Some(Task::Fitting(gun)));
    ship.strike_hull(32., 12.);
    assert_eq!(ship.triage(), Some(Task::Fitting(engine)));
    ship.strike_hull(32., 12.);
    assert_eq!(
        ship.triage(),
        Some(Task::Module(4)),
        "the citadel is flooding"
    );

    // Working down the list leaves only what cannot be mended at sea.
    for _ in 0..2000 {
        if ship.party().is_none() {
            ship.assign(ship.triage());
        }
        ship.tend(1.);
    }
    assert!(ship.compartments().iter().all(|hold| !hold.wrecked()));
    assert_eq!(ship.state(engine), State::Working);
    assert_eq!(ship.state(gun), State::Working);
    assert!(ship.integrity() < 1. && ship.triage().is_none());
}

#[test]
fn reserve_power_lasts_until_either_engine_is_back_in_service() {
    let mut ship = Ship::new(stock_crew());
    ship.insert_module(4, Module::new(Frame::Straight, [Engine]))
        .unwrap();
    let (first, second) = (berth(3, 2), berth(4, 0));
    ship.strike_fitting(first, 12.);
    assert!(!ship.on_reserve_power());
    assert_eq!((ship.propulsion(), ship.gun_train_rate()), (0.5, 1.));
    ship.strike_fitting(second, 12.);
    ship.strike_fitting(berth(1, 0), 12.);
    for elapsed in [0, 300] {
        tend(&mut ship, elapsed);
        assert!(ship.on_reserve_power());
        assert_eq!((ship.propulsion(), ship.gun_train_rate()), (0.15, 0.30));
        assert!(!ship.ready(berth(1, 0)) && ship.ready(berth(5, 0)));
    }
    assert!(ship.assign(Some(Task::Fitting(second))));
    tend(&mut ship, 12);
    assert_eq!(ship.state(first), State::Out);
    assert_eq!(ship.state(second), State::Damaged);
    assert!(!ship.on_reserve_power());
    assert_eq!((ship.propulsion(), ship.gun_train_rate()), (0.25, 1.));
}

#[test]
fn an_enemy_boats_battery_runs_out_after_twenty_seconds() {
    let mut boat = Ship::new(Design::patrol_boat());
    boat.strike_hull(7., 2.);
    tend(&mut boat, 19);
    assert!(boat.ready(berth(0, 0)));
    assert_eq!((boat.propulsion(), boat.gun_train_rate()), (0., 1.));
    tend(&mut boat, 2);
    assert!(!boat.ready(berth(0, 0)) && boat.afloat());
    assert_eq!(boat.reserve(), 0.);
}

#[test]
fn the_dock_repairs_what_its_budget_reaches_and_she_sails_with_the_rest() {
    let mut ship = Ship::new(Design::breakwater());
    let engine = berth(3, 2);
    ship.strike_hull(17., 12.);
    ship.strike_hull(7.5, 18.);
    assert!(near(ship.hull(), 70.));
    tend(&mut ship, 30);
    assert!(ship.on_reserve_power());

    // New plate goes to the wrecked module first, and fittings come back with it.
    assert_eq!(ship.dock(20.), [Mend::Refloated(1), Mend::Restored(engine)]);
    assert_eq!(ship.state(engine), State::Working);
    assert!(!ship.on_reserve_power() && ship.powered());
    let holds = ship.compartments();
    assert_eq!((holds[1].structure(), holds[1].ceiling()), (18., 18.));
    assert!(
        holds[3].structure() < holds[3].ceiling(),
        "the crew can finish this"
    );
    assert!(ship.integrity() < 0.95);

    // A ship that is really banged up leaves with a module still under water.
    let mut wreck = Ship::new(Design::breakwater());
    wreck.strike_hull(7.5, 18.);
    wreck.strike_hull(22.5, 18.);
    assert_eq!(wreck.dock(6.), [Mend::Refloated(1)]);
    assert_eq!(wreck.state(berth(1, 0)), State::Working);
    assert_eq!(wreck.state(berth(4, 0)), State::Out);
    assert!(wreck.compartments()[4].wrecked());
}

#[test]
fn extending_a_live_hull_preserves_damage_and_the_repair_partys_berth() {
    let stock = std::sync::Arc::new(Design::breakwater());
    let mut ship = Ship::new(stock.clone());
    ship.strike_hull(22., 5.);
    ship.strike_fitting(berth(4, 0), 12.);
    assert!(ship.assign(Some(Task::Fitting(berth(4, 0)))));
    let structure = ship.compartments()[4].structure();
    let ceiling = ship.compartments()[4].ceiling();
    let engine = || Module::new(Frame::Straight, [Engine]);
    assert_eq!(ship.insert_module(0, engine()), Err(Fault::Ends));
    assert_eq!(ship.compartments().len(), 6);
    assert_eq!(ship.party(), Some(Task::Fitting(berth(4, 0))));
    ship.insert_module(4, engine()).unwrap();
    assert_eq!(stock.modules().len(), 6);
    assert_eq!(ship.party(), Some(Task::Fitting(berth(5, 0))));
    assert_eq!(ship.compartments()[5].structure(), structure);
    assert_eq!(ship.compartments()[5].ceiling(), ceiling);
    assert_eq!(ship.state(berth(5, 0)), State::Out);
    assert_eq!(ship.state(berth(4, 0)), State::Working);
    assert_eq!(ship.compartments()[4].soundness(), 1.);
    ship.strike_hull(22., 12.);
    assert_eq!(ship.state(berth(4, 0)), State::Out);
    assert!(near(ship.propulsion(), 0.5));
}

#[test]
fn thick_plate_turns_aside_light_rounds_until_heavy_ones_wear_it_thin() {
    let armoured = Design::breakwater().protected(Protection {
        plating: 2.,
        thickness: 7.,
        ..Protection::BREAKWATER
    });
    let mut ship = Ship::new(armoured.clone());
    let (module, panel) = ship.panel_at(7.5, 2.);
    assert!(near(
        ship.design().plate_strength(module),
        2. * Design::breakwater().plate_strength(module)
    ));
    let lost = |ship: &mut Ship, abeam: f32, calibre: Option<f32>| {
        let before = ship.compartments()[module].structure();
        ship.strike_side(7.5, abeam, 4., calibre);
        before - ship.compartments()[module].structure()
    };
    let left = |ship: &Ship| ship.compartments()[module].plating(panel);
    // Seven inches of sound plate turns a destroyer's six-inch round and is scuffed.
    assert_eq!(lost(&mut ship, 2., Some(5.9)), 0.);
    assert!(near(left(&ship), 17. / 18.));
    // A battleship's twelve-inch round goes through and wears it.
    assert!(near(lost(&mut ship, 2., Some(12.)), 4. * (1. + 1. / 18.)));
    assert!(near(left(&ship), 13. / 18.));
    // What is left still turns a patrol boat's round, but no longer the destroyer's.
    assert_eq!(lost(&mut ship, 2., Some(4.7)), 0.);
    assert!(near(lost(&mut ship, 2., Some(5.9)), 4. * (1. + 6. / 18.)));
    // The other beam is sound and turns it, and nothing turns a ram.
    assert_eq!(lost(&mut ship, -2., Some(5.9)), 0.);
    assert!(near(lost(&mut Ship::new(armoured), 2., None), 4.));
    // Stock plate turns nothing that is fired at her.
    let mut stock = Ship::new(Design::breakwater());
    assert!(near(lost(&mut stock, 2., Some(2.8)), 4.));
}

#[test]
fn her_log_says_what_each_round_met_and_what_got_through_until_it_is_read() {
    let mut ship = Ship::new(Design::breakwater().protected(Protection {
        thickness: 7.,
        ..Protection::BREAKWATER
    }));
    ship.strike_side(7.5, 2., 4., Some(5.9));
    ship.strike_side(7.5, 2., 4., Some(12.));
    // A round the pressure dial took all the damage from still struck her.
    ship.strike_side(7.5, -2., 0., Some(12.));
    let log = ship.take_struck();
    assert_eq!(log.len(), 3);
    let (turned, through, spent) = (&log[0], &log[1], &log[2]);
    assert!(turned.turned && turned.dealt == 0. && turned.losses.is_empty());
    assert!(near(turned.plate, 7.) && turned.calibre == Some(5.9));
    // The first round scuffed the plate, so the second met a little less of it.
    assert!(!through.turned && through.plate < 7. && through.dealt > 4.);
    assert!(near(through.aft, 7.5) && near(through.abeam, 2.));
    assert!(!spent.turned && spent.dealt == 0. && near(spent.plate, 7.));
    assert!(ship.take_struck().is_empty(), "read once");
    // Nobody reading it keeps it from growing without end.
    for _ in 0..100 {
        ship.strike_side(1., 1., 0., Some(3.));
    }
    assert_eq!(ship.take_struck().len(), ship::STRUCK_KEPT);
}

#[test]
fn hardening_strengthens_engine_and_fittings_and_keeps_damage_as_a_share() {
    let mut ship = Ship::new(Design::breakwater());
    let engine = berth(3, 2);
    let gun = ship.fitted(MainGun).next().unwrap().0;
    assert_eq!(ship.design().strength(Engine), Some(12.));
    ship.strike_hull(17., 6.);
    ship.strike_fitting(gun, 6.);
    assert!(near(ship.health(engine), 0.5) && near(ship.health(gun), 0.5));
    ship.protect(Protection {
        engine: 1.,
        fittings: 1.5,
        ..Protection::BREAKWATER
    });
    assert_eq!(ship.design().strength(Engine), Some(18.));
    assert_eq!(ship.design().strength(MainGun), Some(18.));
    // Each keeps the share of strength it had left.
    assert!(near(ship.health(engine), 0.5) && near(ship.health(gun), 0.5));
    // A fully hardened engine lasts as long as its compartment.
    let mut fresh = Ship::new(Design::breakwater());
    fresh.protect(Protection {
        engine: 1.,
        ..Protection::BREAKWATER
    });
    assert_eq!(fresh.strike_hull(17., 17.), []);
    assert_eq!(fresh.state(engine), State::Damaged);
}

#[test]
fn double_turrets_stand_abreast_and_train_round_their_own_side_to_bow_and_stern() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let mut design = Design::breakwater();
    design
        .insert(4, Module::new(Frame::DoubleBattery, [MainGun, MainGun]))
        .unwrap();
    let guns = design.gun_stations();
    let pair: Vec<_> = guns
        .iter()
        .filter(|g| g.mounting == Mounting::Double)
        .collect();
    assert_eq!(pair.len(), 2);
    assert_eq!(pair[0].berth.module, 4);
    assert!(pair[0].abeam < 0. && pair[1].abeam > 0.);
    assert!(near(pair[0].fore, pair[1].fore));
    for g in pair {
        // Full-size turrets, not a light wing pair.
        assert!(!g.paired());
        let own = g.abeam.signum() * FRAC_PI_2;
        assert!(g.bears(own) && g.bears(0.) && g.bears(PI));
        assert!(!g.bears(-own));
    }
    // A centreline turret beside them is masked across them, as by any centreline turret.
    let after = guns.iter().find(|g| g.berth.module == 5).unwrap();
    assert_eq!(after.mounting, Mounting::Centre);
    assert!(!after.bears(0.));
}
