use super::Component::*;
use super::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}
fn berth(module: usize, index: usize) -> Berth {
    Berth { module, index }
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
        (Design::flagship(), 81., 21.5),
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

    // The flagship's turrets take 3 points each whatever her hull is worth.
    let mut flagship = Ship::new(Design::flagship());
    assert_eq!(flagship.strike_fitting(turrets[3], 2.), []);
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
        near(ship.propulsion(), 1. - 0.5 * 10. / 30.),
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
    assert!(ship.propulsion() > 0.3 && ship.propulsion() < 0.5);
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
    let mut ship = Ship::new(Design::breakwater());
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
fn a_knocked_out_engine_waits_for_the_repair_party() {
    let mut ship = Ship::new(Design::breakwater());
    let engine = berth(3, 2);
    ship.strike_hull(17., 12.);
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
    let mut ship = Ship::new(Design::breakwater());
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
    assert!(tend(&mut ship, 20).is_empty());
    assert_eq!(tend(&mut ship, 2), [Mend::Refloated(1)]);
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
    let mut ship = Ship::new(Design::breakwater());
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
