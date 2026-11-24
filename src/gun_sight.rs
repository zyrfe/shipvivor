//! The bow and stern rings follow the deck mounts, with one load pip per fitted barrel.
use crate::{
    camera::ShipView,
    state::{Campaign, Encounter, PlayerHelm},
};
use bevy::prelude::*;
use shipvivor_encounter::{MountStatus, Station};

#[derive(Component)]
pub struct GunCircle(Station);
#[derive(Component)]
pub struct GunLabel(&'static str);
#[derive(Component)]
pub struct GunPip(usize);

fn radius(station: Station) -> f32 {
    match station {
        Station::Bow => 10.,
        Station::Stern => 15.,
        Station::Third => 20.,
        Station::Fourth => 25.,
    }
}
pub(crate) fn ink(status: MountStatus) -> Color {
    match status {
        MountStatus::Ready => Color::srgb_u8(105, 251, 180),
        MountStatus::Training => Color::srgb_u8(255, 195, 71),
        MountStatus::Loading => Color::srgb_u8(128, 209, 255),
        MountStatus::Disabled => Color::srgb_u8(234, 125, 98),
        MountStatus::Masked => Color::srgb_u8(183, 181, 174),
    }
}
pub fn spawn(cmd: &mut Commands, root: Entity, run: &Campaign) {
    let positions = run.ship.design().gun_stations();
    for (&station, position) in Station::SLOTS.iter().zip(&positions) {
        let layout = run.fittings.for_station(position).turrets;
        let radius = radius(station);
        let ring = cmd
            .spawn((
                GunCircle(station),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(2. * radius),
                    height: px(2. * radius),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(ink(MountStatus::Training)),
                ChildOf(root),
            ))
            .id();
        cmd.spawn((
            GunLabel(station.label(&positions)),
            Text::new(station.label(&positions)),
            TextFont::from_font_size(11.),
            TextColor(Color::WHITE),
            BackgroundColor(Color::srgba_u8(14, 34, 40, 218)),
            Node {
                position_type: PositionType::Absolute,
                left: px(radius - 7.),
                top: px(if station == Station::Bow {
                    -18.
                } else {
                    2. * radius + 1.
                }),
                padding: UiRect::horizontal(px(3)),
                ..default()
            },
            ChildOf(ring),
        ));
        for tube in 0..layout.count() {
            cmd.spawn((
                GunPip(tube),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(2. * radius + 3.),
                    top: px(radius - 2.5 + 7. * (tube as f32 - (layout.count() - 1) as f32 * 0.5)),
                    width: px(5),
                    height: px(5),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(ink(MountStatus::Loading)),
                ChildOf(ring),
            ));
        }
    }
}

pub fn update(
    sight: (Res<ShipView>, Res<PlayerHelm>),
    encounter: Res<Encounter>,
    window: Single<&Window>,
    controls: (Res<UiScale>, Res<crate::state::Guns>),
    mut circles: Query<(&GunCircle, &Children, &mut Node, &mut BorderColor)>,
    mut labels: Query<(&GunLabel, &mut Text, &mut TextColor)>,
    mut pips: Query<
        (&GunPip, &mut Node, &mut BackgroundColor, &mut BorderColor),
        Without<GunCircle>,
    >,
) {
    let (view, helm) = sight;
    let (scale, guns) = controls;
    let size = window.size() / scale.0;
    if size.min_element() < 1. {
        return;
    }
    for (circle, children, mut node, mut border) in &mut circles {
        node.display = if guns.torpedo_mode {
            Display::None
        } else {
            Display::Flex
        };
        let station = circle.0;
        let mount = encounter.main_mount(station);
        let point = mount.aim_mark(helm.position, helm.heading);
        let (center, edge) = view.project_mark(helm.position, point, size);
        let radius = radius(station);
        node.left = px(center.x - radius);
        node.top = px(center.y - radius);
        let color = ink(mount.status());
        *border = BorderColor::all(if edge.is_empty() { color } else { Color::NONE });
        for child in children.iter() {
            if let Ok((pip, mut node, mut fill, mut rim)) = pips.get_mut(child) {
                node.display = if edge.is_empty() {
                    Display::Flex
                } else {
                    Display::None
                };
                let (inside, outline) = if mount.loaded(pip.0) {
                    (color, color)
                } else {
                    (Color::NONE, ink(MountStatus::Loading))
                };
                fill.0 = inside;
                *rim = BorderColor::all(outline);
            }
            if let Ok((label, mut text, mut tint)) = labels.get_mut(child) {
                let caption = format!("{}{edge}", label.0);
                if text.0 != caption {
                    text.0 = caption;
                }
                if tint.0 != color {
                    tint.0 = color;
                }
            }
        }
    }
}
