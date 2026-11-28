//! Breakwater's captain on the forecastle and at her post during a run.
mod prow;
use crate::state::{Campaign, Encounter, PlayerHelm};
use crate::surfaces::painted_image;
use bevy::prelude::*;
use shipvivor_encounter::Events;

/// Each captain's paintings, by their id in the crew data: the watch sheet of four
/// faces (calm, startled, determined, delighted), the calm face blinking, and the
/// figure on the prow. The first stands in for a captain with none of their own.
const PAINTED: [(&str, &[u8], &[u8], &[u8]); 4] = [
    (
        "calloway",
        include_bytes!("captain/calloway/watch.png"),
        include_bytes!("captain/calloway/blink.png"),
        include_bytes!("captain/calloway/prow.png"),
    ),
    (
        "marrack",
        include_bytes!("captain/marrack/watch.png"),
        include_bytes!("captain/marrack/blink.png"),
        include_bytes!("captain/marrack/prow.png"),
    ),
    (
        "ashby",
        include_bytes!("captain/ashby/watch.png"),
        include_bytes!("captain/ashby/blink.png"),
        include_bytes!("captain/ashby/prow.png"),
    ),
    (
        "thorne",
        include_bytes!("captain/thorne/watch.png"),
        include_bytes!("captain/thorne/blink.png"),
        include_bytes!("captain/thorne/prow.png"),
    ),
];

/// One captain's paintings, loaded.
struct Painted {
    id: &'static str,
    faces: Handle<Image>,
    blink: Handle<Image>,
    figure: Handle<Image>,
    /// Pixels on a side of one face on the watch sheet.
    cell: f32,
}

#[derive(Resource)]
pub struct CaptainArt {
    painted: Vec<Painted>,
}

impl CaptainArt {
    /// The paintings of the captain with this id, or the stand-in's.
    fn of(&self, id: Option<&str>) -> &Painted {
        (self.painted.iter())
            .find(|p| Some(p.id) == id)
            .unwrap_or(&self.painted[0])
    }

    /// A captain's face at rest, for a portrait: the sheet and the cell to show.
    pub fn portrait(&self, id: &str) -> (Handle<Image>, Rect) {
        let painted = self.of(Some(id));
        (
            painted.faces.clone(),
            Rect::new(0., 0., painted.cell, painted.cell),
        )
    }
}

/// The captain to show on the prow and at the watch: the one previewed on the career
/// page while it is open, else the one in command of this run, else the one the saved
/// career is under.
pub fn shown(
    run: &Campaign,
    book: Option<&crate::career::CareerBook>,
    crew: Option<&crate::crew_book::CrewBook>,
) -> Option<String> {
    if run.screen() == crate::state::Screen::Career
        && let (Some(book), Some(crew)) = (book, crew)
    {
        return (crew.wardroom.captains.get(book.captain)).map(|c| c.id.clone());
    }
    (run.captain.as_ref().map(|c| c.id.clone()))
        .or_else(|| book.and_then(|b| b.roster.last.clone()))
}

pub fn setup(
    mut cmd: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    hull: Single<Entity, With<crate::scene::Ship>>,
) {
    let mut aspect = 1.;
    let painted = PAINTED
        .map(|(id, watch, blink, prow)| {
            let faces = painted_image(watch);
            let figure = painted_image(prow);
            // Every figure is painted to the same proportions.
            aspect = figure.width() as f32 / figure.height() as f32;
            Painted {
                id,
                cell: faces.width() as f32 * 0.5,
                faces: images.add(faces),
                blink: images.add(painted_image(blink)),
                figure: images.add(figure),
            }
        })
        .into();
    let art = CaptainArt { painted };
    prow::spawn(
        &mut cmd,
        *hull,
        &mut meshes,
        &mut materials,
        art.of(None).figure.clone(),
        aspect,
    );
    cmd.insert_resource(art);
}

/// Dress the figure on the prow as the captain [`shown`].
pub fn dress(
    run: Res<Campaign>,
    book: Option<Res<crate::career::CareerBook>>,
    crew: Option<Res<crate::crew_book::CrewBook>>,
    art: Res<CaptainArt>,
    figure: Single<&MeshMaterial3d<StandardMaterial>, With<ProwCaptain>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let id = shown(&run, book.as_deref(), crew.as_deref());
    let wanted = &art.of(id.as_deref()).figure;
    if let Some(mut material) = materials.get_mut(&figure.0)
        && material.base_color_texture.as_ref() != Some(wanted)
    {
        material.base_color_texture = Some(wanted.clone());
    }
}

pub(crate) use prow::ProwCaptain;
pub use prow::wind;

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) enum Expression {
    #[default]
    Calm,
    Startled,
    Determined,
    Delighted,
}

#[derive(Clone, Default)]
pub struct Watch {
    startle_left: f32,
    delight_left: f32,
}

impl Watch {
    /// Age reactions on the simulation clock, including hits between rendered frames.
    pub fn observe(&mut self, dt: f32, ev: &Events) {
        self.startle_left = (self.startle_left - dt).max(0.);
        self.delight_left = (self.delight_left - dt).max(0.);
        if ev.damage > 0 {
            self.startle_left = 1.1;
            self.delight_left = 0.;
        } else if ev.sunk > 0 {
            self.delight_left = 2.;
        }
    }

    fn expression(&self, strained: bool, spotted: bool) -> Expression {
        if self.startle_left > 0. || spotted {
            Expression::Startled
        } else if strained {
            Expression::Determined
        } else if self.delight_left > 0. {
            Expression::Delighted
        } else {
            Expression::Calm
        }
    }
}

#[derive(Component)]
pub(crate) struct Face;

/// The watch camera sits beside the helm readout, inside the same corner panel.
pub fn portrait(cmd: &mut Commands, parent: Entity, art: &CaptainArt) {
    let frame = cmd
        .spawn((
            Node {
                width: px(82),
                height: px(82),
                flex_shrink: 0.,
                border: UiRect::all(px(1)),
                overflow: Overflow::clip(),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(Color::srgb_u8(152, 131, 87)),
            ChildOf(parent),
        ))
        .id();
    // The captain in command takes the frame on the first `react`.
    let stand_in = art.of(None);
    cmd.spawn((
        Face,
        ImageNode {
            image: stand_in.faces.clone(),
            rect: Some(Rect::new(0., 0., stand_in.cell, stand_in.cell)),
            ..default()
        },
        Node {
            width: px(88),
            height: px(88),
            flex_shrink: 0.,
            ..default()
        },
        ChildOf(frame),
    ));
}

pub fn react(
    run: Res<Campaign>,
    battle: Res<Encounter>,
    helm: Res<PlayerHelm>,
    time: Res<Time>,
    art: Res<CaptainArt>,
    mut portraits: Query<(&mut ImageNode, &mut UiTransform), With<Face>>,
) {
    let Some((mut image, mut pose)) = portraits.iter_mut().next() else {
        return;
    };
    let art = art.of(run.captain.as_ref().map(|c| c.id.as_str()));
    let strained = run.ship.on_reserve_power() || run.ship.in_peril() || helm.rudder.abs() > 0.85;
    let spotted = battle.spotters().iter().any(|p| p.alert_left > 0.);
    let expression = run.watch.expression(strained, spotted);
    let t = time.elapsed_secs();
    let blink = expression == Expression::Calm && (t + 0.32 * (t * 0.71).sin()) % 4.7 > 4.54;
    let (texture, rect) = if blink {
        (&art.blink, None)
    } else {
        let (x, y) = match expression {
            Expression::Calm => (0., 0.),
            Expression::Startled => (1., 0.),
            Expression::Determined => (0., 1.),
            Expression::Delighted => (1., 1.),
        };
        let origin = Vec2::new(x, y) * art.cell;
        (
            &art.faces,
            Some(Rect::from_corners(origin, origin + Vec2::splat(art.cell))),
        )
    };
    if &image.image != texture || image.rect != rect {
        image.image = texture.clone();
        image.rect = rect;
    }
    let flinch = run.watch.startle_left.powi(2);
    pose.rotation = Rot2::degrees(-1.5 * helm.rudder + flinch * (t * 31.).sin());
    pose.translation = Val2::px(0., (t * 1.6).sin() * 0.65 - flinch);
}

#[cfg(test)]
mod tests {
    use super::PAINTED;

    #[test]
    fn every_captain_in_the_crew_data_is_painted() {
        let wardroom = shipvivor_campaign::crew::Wardroom::stock();
        for captain in &wardroom.captains {
            assert!(
                PAINTED.iter().any(|(id, ..)| *id == captain.id),
                "{} has no paintings in src/captain/",
                captain.id
            );
        }
    }
}
