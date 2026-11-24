//! Breakwater's captain on the forecastle and at her post during a run.
mod prow;
use crate::state::{Campaign, Encounter, PlayerHelm};
use crate::surfaces::painted_image;
use bevy::prelude::*;
use shipvivor_encounter::Events;

#[derive(Resource)]
pub struct CaptainArt {
    faces: Handle<Image>,
    blink: Handle<Image>,
    cell: f32,
}

pub fn setup(
    mut cmd: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    hull: Single<Entity, With<crate::scene::Ship>>,
) {
    let faces = painted_image(include_bytes!("captain/watch.png"));
    let cell = faces.width() as f32 * 0.5;
    cmd.insert_resource(CaptainArt {
        faces: images.add(faces),
        cell,
        blink: images.add(painted_image(include_bytes!("captain/blink.png"))),
    });
    let figure = painted_image(include_bytes!("captain/prow.png"));
    let aspect = figure.width() as f32 / figure.height() as f32;
    prow::spawn(
        &mut cmd,
        *hull,
        &mut meshes,
        &mut materials,
        images.add(figure),
        aspect,
    );
}

#[cfg(test)]
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

#[derive(Default)]
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
    cmd.spawn((
        Face,
        ImageNode {
            image: art.faces.clone(),
            rect: Some(Rect::new(0., 0., art.cell, art.cell)),
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
