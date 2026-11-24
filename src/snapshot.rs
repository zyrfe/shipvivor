//! Dev builds only: the backquote key (` and ~) saves the window as a PNG in
//! `work/screenshots`, along with a Markdown note on the game state, so a screen can be
//! discussed with an agent without first describing it.
use crate::camera::ShipView;
use crate::state::{Campaign, Encounter, Guns, PlayerHelm, Screen};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::PrimaryWindow;
use std::fmt::Write as _;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The capture folder inside the project, wherever the game is launched from.
const FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/work/screenshots");
/// How long the confirmation stays on screen, in real seconds.
const TOAST_SECS: f32 = 2.5;

pub struct SnapshotPlugin;
impl Plugin for SnapshotPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(debug_assertions) {
            app.add_systems(Update, (capture, fade_toasts));
        }
    }
}

#[derive(Component)]
struct Toast(f32);

fn capture(
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    game: (
        Res<Campaign>,
        Res<PlayerHelm>,
        Res<ShipView>,
        Res<Guns>,
        Res<Encounter>,
    ),
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Backquote) {
        return;
    }
    let folder = Path::new(FOLDER);
    if let Err(err) = std::fs::create_dir_all(folder) {
        warn!("Couldn't create {FOLDER}: {err}");
        return;
    }
    let (run, helm, view, guns, battle) = game;
    let (stamp, when) = timestamp(SystemTime::now());
    let note = describe(&when, &window, &run, &helm, &view, &guns, &battle);
    let png = folder.join(format!("{stamp}.png"));
    // The note goes first so it is there by the time the image appears.
    if let Err(err) = std::fs::write(folder.join(format!("{stamp}.md")), note) {
        warn!("Couldn't write the capture note: {err}");
    }
    // Downscale a high-DPI window to its logical size, which is what the UI is laid out in.
    let scale = window.scale_factor().round().max(1.) as u32;
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut commands: Commands| {
            let image = shot.image.clone();
            let path = png.clone();
            // Encoding takes a moment in a debug build. Keep it off the frame.
            std::thread::spawn(move || {
                if let Err(err) = save_png(image, scale, &path) {
                    warn!("Couldn't save {}: {err}", path.display());
                }
            });
            let name = png
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into());
            show_toast(&mut commands, format!("Captured  work/screenshots/{name}"));
        },
    );
}

fn save_png(image: Image, scale: u32, path: &Path) -> Result<(), String> {
    let rgb = image
        .try_into_dynamic()
        .map_err(|e| e.to_string())?
        .to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());
    let (pixels, width, height) = shrink(&rgb.into_raw(), width, height, scale);
    // Write under a temporary name so a reader never sees half a file.
    let partial = path.with_extension("png.partial");
    let file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())
}

/// Average each `scale`×`scale` block of an RGB image into one pixel.
fn shrink(rgb: &[u8], width: u32, height: u32, scale: u32) -> (Vec<u8>, u32, u32) {
    if scale <= 1 {
        return (rgb.to_vec(), width, height);
    }
    let (w, h) = (width / scale, height / scale);
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            for channel in 0..3 {
                let mut sum = 0;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (sx, sy) = (x * scale + dx, y * scale + dy);
                        sum += u32::from(rgb[((sy * width + sx) * 3 + channel) as usize]);
                    }
                }
                out.push((sum / (scale * scale)) as u8);
            }
        }
    }
    (out, w, h)
}

/// A note along the bottom centre, clear of the HUD panels in the corners.
fn show_toast(commands: &mut Commands, message: String) {
    commands.spawn((
        Toast(TOAST_SECS),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(0),
            right: px(0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        GlobalZIndex(i32::MAX),
        children![(
            Node {
                padding: UiRect::axes(px(10), px(6)),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.7)),
            Text::new(message),
            TextFont::from_font_size(14.),
            TextColor(Color::WHITE),
        )],
    ));
}

fn fade_toasts(
    time: Res<Time<Real>>,
    mut toasts: Query<(Entity, &mut Toast)>,
    mut commands: Commands,
) {
    for (entity, mut toast) in &mut toasts {
        toast.0 -= time.delta_secs();
        if toast.0 <= 0. {
            commands.entity(entity).despawn();
        }
    }
}

/// A sortable file stem and a readable time, both in UTC.
fn timestamp(now: SystemTime) -> (String, String) {
    let since = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (year, month, day) = civil_date(secs / 86_400);
    let (h, m, s) = (secs / 3600 % 24, secs / 60 % 60, secs % 60);
    let ms = since.subsec_millis();
    (
        format!("{year:04}-{month:02}-{day:02}_{h:02}-{m:02}-{s:02}-{ms:03}"),
        format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC"),
    )
}

/// The proleptic Gregorian date of a day counted from 1970-01-01.
fn civil_date(days: u64) -> (u64, u64, u64) {
    // Howard Hinnant's civil_from_days, shifted so each year starts on 1 March.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

fn degrees(radians: f32) -> i32 {
    (radians.to_degrees().round() as i32).rem_euclid(360)
}

/// What was on screen, in the terms the HUD uses.
fn describe(
    when: &str,
    window: &Window,
    run: &Campaign,
    helm: &PlayerHelm,
    view: &ShipView,
    guns: &Guns,
    battle: &Encounter,
) -> String {
    let mut s = String::new();
    let screen = run.screen();
    let _ = writeln!(s, "# Shipvivor capture, {when}\n");
    let _ = writeln!(
        s,
        "- Screen: {screen:?} (mode {:?}), leg {:?}, {:.0} s elapsed, {:.0} s left",
        run.mode,
        run.stage,
        run.elapsed,
        run.remaining()
    );
    // `SHIPVIVOR_SEED` with this value charts the same waters again.
    let _ = writeln!(s, "- Run seed: {}", run.seed);
    let set = battle.currents().set().flow;
    let here = battle.current(helm.position);
    let _ = writeln!(
        s,
        "- Current: open water {:.2} m/s toward {:03} deg, here {:.2} m/s toward {:03} deg",
        set.length(),
        degrees(shipvivor_compass::heading(set)),
        here.length(),
        degrees(shipvivor_compass::heading(here))
    );
    let nearest_ice = battle.icebergs().iter().min_by(|a, b| {
        let near = |i: &shipvivor_encounter::Iceberg| helm.position.distance(i.body.pos);
        near(a).total_cmp(&near(b))
    });
    if let Some(ice) = nearest_ice {
        let _ = writeln!(
            s,
            "- Ice: {} pieces adrift. Nearest a {:?} {:.0} m across at {:.0} m, bearing {:03} deg",
            battle.icebergs().len(),
            ice.shape,
            ice.radius * 2.,
            helm.position.distance(ice.body.pos),
            degrees(shipvivor_compass::heading(ice.body.pos - helm.position))
        );
    }
    let _ = writeln!(
        s,
        "- Window: {:.0}x{:.0} logical, scale factor {}",
        window.width(),
        window.height(),
        window.scale_factor()
    );
    let _ = writeln!(
        s,
        "- Ship: hull {}%{}, level {}, XP {} / {}, hits {}, sunk {}",
        run.hull_percent(),
        if run.ship.in_peril() {
            " (one more wrecked module sinks her)"
        } else {
            ""
        },
        run.level,
        run.experience,
        run.next_level_xp(),
        run.hits,
        run.sunk
    );
    if let Screen::Drydock(_) = screen {
        let _ = writeln!(
            s,
            "- Drydock: section {}, option {:?} = {:?}, armed {}",
            run.dock_module,
            run.dock_option,
            run.dock_job(),
            run.dock_armed
        );
    }
    let _ = writeln!(
        s,
        "- Helm: {} {:+.1} kn, heading {:03} deg, rudder {:+.2}, position ({:.0}, {:.0})",
        helm.order_label(),
        helm.speed * 1.94384,
        degrees(helm.heading),
        helm.rudder,
        helm.position.x,
        helm.position.y
    );
    let weapon = if guns.torpedo_mode {
        "torpedoes".to_string()
    } else {
        format!("guns, loading {:?}", guns.ammo)
    };
    let _ = writeln!(
        s,
        "- Sight: bearing {:03} deg, zoom {:.2}, {weapon}",
        degrees(view.bearing),
        view.zoom
    );
    if let Some(target) = view.target(battle) {
        let _ = writeln!(
            s,
            "- {}: {:?} #{} at {:.0} m, bearing {:03} deg",
            if view.locked() { "Pinned" } else { "Marked" },
            target.class,
            target.body.id,
            helm.position.distance(target.body.pos),
            degrees(shipvivor_compass::heading(target.body.pos - helm.position))
        );
    }
    let mut afloat: Vec<_> = battle.raiders().iter().filter(|r| r.afloat()).collect();
    afloat.sort_by(|a, b| {
        let near = |r: &&shipvivor_encounter::Raider| helm.position.distance(r.body.pos);
        near(a).total_cmp(&near(b))
    });
    if matches!(screen, Screen::Sailing | Screen::Paused | Screen::Secured) {
        let _ = writeln!(s, "- Contacts afloat: {}", afloat.len());
        for r in afloat.iter().take(5) {
            let _ = writeln!(
                s,
                "  - {:?} #{} at {:.0} m, bearing {:03} deg",
                r.class,
                r.body.id,
                helm.position.distance(r.body.pos),
                degrees(shipvivor_compass::heading(r.body.pos - helm.position))
            );
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stamps_sort_and_read_in_utc() {
        let t = UNIX_EPOCH + Duration::from_millis(1_795_467_857_042);
        let (stem, when) = timestamp(t);
        assert_eq!(stem, "2026-11-23_21-04-17-042");
        assert_eq!(when, "2026-11-23 21:04:17 UTC");
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
    }

    #[test]
    fn shrink_averages_blocks() {
        let rgb = [
            0, 0, 0, 100, 100, 100, //
            200, 200, 200, 100, 100, 100,
        ];
        assert_eq!(shrink(&rgb, 2, 2, 2), (vec![100, 100, 100], 1, 1));
        assert_eq!(shrink(&rgb, 2, 2, 1).0, rgb.to_vec());
    }
}
