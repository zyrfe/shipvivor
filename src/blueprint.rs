//! Work drawn up in the yard and not yet built is drawn on the ship as a blueprint. A
//! spliced section is blueprint along its stretch of hull, and a turret, a section's
//! gear or a gun mount the plan adds or refits is blueprint all over. When the refit is
//! built a sweep runs along her from bow to stern and the blueprint gives way to paint.
//!
//! Each part keeps its own material: while it is drafted it wears a copy extended with
//! the blueprint, and it gets its own back once it is built.
use crate::{
    dockyard::BERTH,
    scene::{AaPosition, DeckFitting, RangeTower, SectionModel, Ship, Turret, WingPosition},
    state::{Campaign, Screen},
};
use bevy::{
    asset::{load_internal_asset, uuid_handle},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use shipvivor_campaign::{Plan, Run};
use shipvivor_encounter::Hull;

const SHADER: Handle<Shader> = uuid_handle!("b4d2a3c1-7f0e-4a51-9b6e-2c8d1e5f7a90");

/// A part's own material with the blueprint over it.
pub type Drafted = ExtendedMaterial<StandardMaterial, Draft>;

/// Seconds the reveal takes to sweep a hull of the stock length.
const REVEAL: f32 = 1.5;
/// The most stretches of hull one material draws as blueprint.
const SPANS: usize = 8;
/// Stands for every stretch of the ship at once.
const EVERYWHERE: Vec2 = Vec2::new(-1e6, 1e6);

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct DraftUniform {
    spans: [Vec4; SPANS],
    /// Spans in use, the z the reveal has reached, 1 while it runs, the band's glow.
    sweep: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct Draft {
    #[uniform(100)]
    plan: DraftUniform,
}
impl MaterialExtension for Draft {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct BlueprintPlugin;
impl Plugin for BlueprintPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<Drafted>::default());
        load_internal_asset!(app, SHADER, "blueprint.wesl", Shader::from_wesl);
    }
}

/// What a plan changes on the ship, as the parts the yard draws as blueprint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// Stretches of hull spliced in, as world z from and to.
    spans: Vec<Vec2>,
    /// Sections whose turrets and gear are new or refitted.
    sections: Vec<usize>,
    /// Main turrets, counted as the design lists them, that are new or refitted.
    turrets: Vec<usize>,
    /// The first wing gun the plan adds, or every one once they are rearmed.
    wings_from: Option<usize>,
    /// The first light AA mount the plan adds.
    aa_from: Option<usize>,
    /// The rangefinder tower is raised.
    tower: bool,
}
impl Changes {
    /// The parts of the planned ship that the plan adds or refits.
    pub fn of(plan: &Plan, run: &Run) -> Self {
        let design = run.ship.design();
        let built = plan.built_design();
        let bow = Hull::CRUISER.bow;
        let modules = 0..design.modules().len();
        // A broader beam rebuilds the whole hull.
        let spans = if design.beam() > built.beam() + 1e-3 {
            vec![EVERYWHERE]
        } else {
            modules
                .clone()
                .filter(|&at| plan.is_new(at))
                .map(|at| {
                    let (fore, aft) = design.span(at);
                    Vec2::new(fore - bow, aft - bow) + BERTH.z
                })
                .collect()
        };
        let (now, then) = (run.fittings, plan.built_fittings());
        // Gear refitted on every rack, or on the plane section, is drawn there.
        let (craft, craft_then) = (now.strike_craft(), then.strike_craft());
        let racks = craft.launchers != craft_then.launchers || craft.salvo != craft_then.salvo;
        let planes = craft.planes != craft_then.planes || craft.bombs != craft_then.bombs;
        let regeared = |at: usize| {
            let fit = &design.modules()[at].fit;
            (racks && fit.contains(&shipvivor_shipyard::Component::MissileRack))
                || (planes && fit.contains(&shipvivor_shipyard::Component::Catapult))
        };
        let sections: Vec<usize> = modules
            .filter(|&at| plan.is_new(at) || plan.is_refitted(at) || regeared(at))
            .collect();
        let turrets = design
            .gun_stations()
            .iter()
            .enumerate()
            .filter(|(_, g)| sections.contains(&g.berth.module))
            .map(|(i, _)| i)
            .collect();
        let (wings, wings_then) = (now.secondaries(), then.secondaries());
        let houses_changed = now.gunhouses() != then.gunhouses();
        let wings_from = if wings.barrels != wings_then.barrels
            || wings.bore != wings_then.bore
            || houses_changed
        {
            Some(0)
        } else {
            (wings.per_side > wings_then.per_side).then_some(2 * wings_then.per_side)
        };
        let aa = then.anti_air().mounts;
        let aa_from = (now.anti_air().mounts > aa).then_some(aa);
        Self {
            spans,
            sections,
            turrets,
            wings_from,
            aa_from,
            tower: now.wing_range > then.wing_range,
        }
    }
}

/// The refit just built, revealed from under its blueprint.
#[derive(Resource)]
pub struct Reveal {
    changes: Changes,
    seconds: f32,
}
impl Reveal {
    pub fn of(plan: &Plan, run: &Run) -> Self {
        Self {
            changes: Changes::of(plan, run),
            seconds: 0.,
        }
    }
}

/// A part wearing the blueprint, and the material it wears when built.
#[derive(Component)]
pub struct Original(Handle<StandardMaterial>);

/// The blueprint copies of each part's own material: over a stretch of hull, or all over.
#[derive(Default)]
pub struct Drafts {
    copies: HashMap<(AssetId<StandardMaterial>, bool), Handle<Drafted>>,
    shown: Changes,
}

/// Whether a part is drafted along its stretch of hull, or all over.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cover {
    Spans,
    Whole,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draft(
    mut cmd: Commands,
    run: Res<Campaign>,
    time: Res<Time>,
    reveal: Option<ResMut<Reveal>>,
    mut drafts: Local<Drafts>,
    mut copies: ResMut<Assets<Drafted>>,
    paints: Res<Assets<StandardMaterial>>,
    ship: Query<Entity, With<Ship>>,
    hull_gear: Query<(Entity, &DeckFitting)>,
    turrets: Query<(Entity, &Turret)>,
    sections: Query<(Entity, &SectionModel)>,
    // Wing guns, and the rangefinder tower, which has no place of its own.
    wings: Query<(Entity, Option<&WingPosition>), Or<(With<WingPosition>, With<RangeTower>)>>,
    tubs: Query<(Entity, &AaPosition)>,
    children: Query<&Children>,
    painted: Query<&MeshMaterial3d<StandardMaterial>>,
    drafted: Query<(Entity, &Original)>,
) {
    let docked = matches!(run.screen(), Screen::Drydock(_));
    let design = run.ship.design();
    let length = design.length();
    // The reveal runs on, and the plan is drawn while there is one.
    let (changes, sweep) = match (reveal, run.plan()) {
        (Some(mut reveal), _) if docked => {
            reveal.seconds += time.delta_secs();
            let stock = shipvivor_shipyard::Design::BREAKWATER_LENGTH;
            let duration = REVEAL * (length / stock).max(1.);
            let t = (reveal.seconds / duration).min(1.);
            if t >= 1. {
                cmd.remove_resource::<Reveal>();
            }
            let bow = BERTH.z - Hull::CRUISER.bow - 2.;
            let front = bow + (length + 4.) * (t * t * (3. - 2. * t));
            let glow = 1. - (t - 0.85).max(0.) / 0.15;
            (reveal.changes.clone(), Some((front, glow)))
        }
        (Some(_), _) => {
            cmd.remove_resource::<Reveal>();
            (Changes::default(), None)
        }
        (None, Some(plan)) if docked => (Changes::of(plan, &run), None),
        _ => (Changes::default(), None),
    };

    // Which parts wear which blueprint.
    let mut cover: HashMap<Entity, Cover> = HashMap::default();
    if !changes.spans.is_empty() {
        for root in &ship {
            cover.insert(root, Cover::Spans);
        }
        for (gear, fitting) in &hull_gear {
            if fitting.on_hull() {
                cover.insert(gear, Cover::Spans);
            }
        }
    }
    let whole = |root: Entity, cover: &mut HashMap<Entity, Cover>| {
        cover.insert(root, Cover::Whole);
        for part in children.iter_descendants(root) {
            cover.insert(part, Cover::Whole);
        }
    };
    for (root, turret) in &turrets {
        if changes.turrets.contains(&turret.index()) {
            whole(root, &mut cover);
        }
    }
    for (root, section) in &sections {
        if section
            .place(design)
            .is_some_and(|at| changes.sections.contains(&at))
        {
            whole(root, &mut cover);
        }
    }
    for (root, wing) in &wings {
        let drafted = match wing {
            Some(wing) => changes.wings_from.is_some_and(|from| wing.index() >= from),
            None => changes.tower,
        };
        if drafted {
            whole(root, &mut cover);
        }
    }
    for (root, tub) in &tubs {
        if changes.aa_from.is_some_and(|from| tub.index() >= from) {
            whole(root, &mut cover);
        }
    }

    // Parts no longer drafted get their own material back.
    for (part, original) in &drafted {
        if !cover.contains_key(&part) {
            cmd.entity(part)
                .remove::<(MeshMaterial3d<Drafted>, Original)>()
                .insert(MeshMaterial3d(original.0.clone()));
        }
    }
    if cover.is_empty() {
        drafts.copies.clear();
        drafts.shown = Changes::default();
        return;
    }

    let uniform = |kind: Cover| {
        let mut plan = DraftUniform::default();
        let spans = match kind {
            Cover::Whole => vec![EVERYWHERE],
            Cover::Spans => changes.spans.clone(),
        };
        for (slot, span) in plan.spans.iter_mut().zip(spans.iter().take(SPANS)) {
            *slot = span.extend(0.).extend(0.);
        }
        let (front, glow) = sweep.unwrap_or((0., 0.));
        plan.sweep = Vec4::new(
            spans.len().min(SPANS) as f32,
            front,
            if sweep.is_some() { 1. } else { 0. },
            glow,
        );
        plan
    };
    // Every copy follows the plan and the sweep.
    let restyle = sweep.is_some() || drafts.shown != changes;
    if restyle {
        for (&(_, spans), handle) in &drafts.copies {
            if let Some(mut copy) = copies.get_mut(handle) {
                copy.extension.plan = uniform(if spans { Cover::Spans } else { Cover::Whole });
            }
        }
        drafts.shown = changes.clone();
    }
    // Parts newly drafted put on a blueprint copy of their own material.
    for (&part, &kind) in &cover {
        let Ok(paint) = painted.get(part) else {
            continue;
        };
        let key = (paint.0.id(), kind == Cover::Spans);
        let copy = match drafts.copies.get(&key) {
            Some(copy) => copy.clone(),
            None => {
                let Some(base) = paints.get(&paint.0).cloned() else {
                    continue;
                };
                let copy = copies.add(Drafted {
                    base,
                    extension: Draft {
                        plan: uniform(kind),
                    },
                });
                drafts.copies.insert(key, copy.clone());
                copy
            }
        };
        cmd.entity(part)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((Original(paint.0.clone()), MeshMaterial3d(copy)));
    }
}
