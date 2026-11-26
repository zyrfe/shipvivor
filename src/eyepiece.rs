//! The scope's eyepiece. As Shift narrows the lens, a round field stop closes in to 80%
//! of the screen's width across, with a soft edge, and colour fringes come up toward its
//! rim. Through it, the HUD's marks over ships, guns and bases show only inside the field.
use crate::{
    camera::{FollowCamera, ShipView},
    state::{Campaign, Mode, Screen},
};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    post_process::effect_stack::ChromaticAberration,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    ui::UiGlobalTransform,
};

/// The black around the field, over the scene and under the whole HUD.
#[derive(Component)]
pub struct FieldStop;

/// Across the field, as a share of the screen's width.
const FIELD: f32 = 0.8;
/// Half the width of the field's soft edge, as a share of its radius.
const EDGE: f32 = 0.12;
/// How dark the field shades toward its edge before the edge itself.
const SHADE: f32 = 0.22;
/// Colour fringing through the scope, as a share of each point's distance from the
/// middle of the screen: nothing at the reticle, strongest at the rim.
const FRINGE: f32 = 0.026;
/// Texels across the field stop's image, which spans two field radii either side of the
/// middle so that it covers the screen past the field.
const TEXELS: u32 = 1024;

/// The HUD's marks over things in the world. Through the scope they show only inside
/// the field.
type WorldMark = Or<(
    With<crate::threat_ring::Badge>,
    With<crate::threat_ring::QuarryName>,
    With<crate::flagship_hud::Tag>,
    With<crate::mission_hud::WorldFlag>,
    With<crate::mission_hud::SpotterAlert>,
    With<crate::slip_badges::SlipBadge>,
    With<crate::fire_control::Mark>,
    With<crate::gun_sight::GunCircle>,
    With<crate::gun_sight::RangeNote>,
)>;

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// How dark the field stop is `u` field radii from the middle: clear, shading a little
/// toward the edge, then softly into black.
fn darkness(u: f32) -> f32 {
    let edge = smoothstep(1. - EDGE, 1. + EDGE, u);
    let shade = SHADE * smoothstep(0.45, 1., u).powi(2);
    1. - (1. - edge) * (1. - shade)
}

fn field_image() -> Image {
    let side = TEXELS as usize;
    let mut data = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let p = (Vec2::new(x as f32, y as f32) + 0.5) / side as f32 * 4. - 2.;
            data.extend_from_slice(&[0, 0, 0, (darkness(p.length()) * 255.).round() as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: TEXELS,
            height: TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Stretched over the screen, the soft edge must blend between texels.
    image.sampler = ImageSampler::linear();
    image
}

pub fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        FieldStop,
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        ImageNode::new(images.add(field_image())),
        // Over the scene and under every panel and mark of the HUD.
        GlobalZIndex(-1),
        Visibility::Hidden,
    ));
}

/// How far the lens is through the scope, from 0 (the cruising lens) to 1. Only the
/// chase camera has a scope.
fn through_scope(run: &Campaign, view: &ShipView) -> f32 {
    let chase =
        !matches!(run.screen(), Screen::Drydock(_) | Screen::Customise) && run.mode != Mode::Title;
    if chase { view.zoom } else { 0. }
}

/// The field's radius on a screen of `size`: standing open past the corners with the
/// scope down, and closing to its full size as the lens narrows.
fn field_radius(size: Vec2, scope: f32) -> f32 {
    size.length().lerp(0.5 * FIELD * size.x, scope)
}

/// Close the field stop and bring up the fringes as the lens narrows.
pub fn field_stop(
    sight: (Res<Campaign>, Res<ShipView>),
    window: Single<&Window>,
    scale: Res<UiScale>,
    mut stop: Single<(&mut Node, &mut ImageNode, &mut Visibility), With<FieldStop>>,
    mut fringe: Single<&mut ChromaticAberration, With<FollowCamera>>,
) {
    let (run, view) = sight;
    let scope = through_scope(&run, &view);
    // At nothing, Bevy skips the pass altogether.
    if fringe.intensity != FRINGE * scope {
        fringe.intensity = FRINGE * scope;
    }
    let (node, image, visibility) = &mut *stop;
    let shown = if scope > 0. {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if **visibility != shown {
        **visibility = shown;
    }
    if scope == 0. {
        return;
    }
    let size = window.size() / scale.0;
    let radius = field_radius(size, scope);
    node.width = px(4. * radius);
    node.height = px(4. * radius);
    node.left = px(size.x * 0.5 - 2. * radius);
    node.top = px(size.y * 0.5 - 2. * radius);
    let tint = Color::WHITE.with_alpha(scope);
    if image.color != tint {
        image.color = tint;
    }
}

/// Hide each world mark whose middle lies outside the field, once the HUD is laid out
/// and before visibility passes down to its parts.
pub fn clip(
    sight: (Res<Campaign>, Res<ShipView>),
    window: Single<&Window>,
    mut marks: Query<(&mut Visibility, &UiGlobalTransform), (WorldMark, Without<FieldStop>)>,
) {
    let (run, view) = sight;
    let scope = through_scope(&run, &view);
    // Marks are laid out in physical pixels.
    let size = window.physical_size().as_vec2();
    let reach = field_radius(size, scope);
    for (mut visibility, at) in &mut marks {
        let outside = scope > 0. && at.translation.distance(size * 0.5) > reach;
        let wanted = if outside {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_clear_in_the_middle_and_softens_to_black_past_its_rim() {
        assert_eq!(darkness(0.), 0.);
        assert!(darkness(0.6) < 0.02);
        assert!((0.08..0.25).contains(&darkness(0.85)));
        assert!((0.45..0.65).contains(&darkness(1.)));
        assert!(darkness(1. + EDGE) > 0.999);
        // No step anywhere across the edge.
        let steps = (0..400).map(|i| darkness(0.8 + i as f32 * 0.001));
        let mut last = darkness(0.8);
        for d in steps {
            assert!((0. ..0.01).contains(&(d - last)), "{last} to {d}");
            last = d;
        }
        // Closed, the field spans 80% of a wide screen; open, it clears the corners.
        let screen = Vec2::new(1600., 1000.);
        assert_eq!(field_radius(screen, 1.), 640.);
        assert!(field_radius(screen, 0.) * (1. - EDGE) > screen.length() * 0.5);
    }
}
