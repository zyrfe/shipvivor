//! Where the window opens. It fills the screen it was last on, which is kept between
//! launches in `window.txt` in Shipvivor's settings folder, or in the file
//! `SHIPVIVOR_WINDOW_FILE` names. On the first launch, or when that screen has gone, it
//! fills the screen the system opened it on.
use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, OnMonitor, PrimaryWindow, RawHandleWrapper};
use std::path::{Path, PathBuf};

/// How far in from a screen's corner the window is placed before it fills that screen,
/// in points, so it lands well inside the screen.
const INSET: f64 = 64.;

/// Frames to wait for the screens to be listed before showing the window regardless.
const PATIENCE: u32 = 30;

pub struct BerthPlugin;

impl Plugin for BerthPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Berths {
            kept: kept(),
            settled: false,
        })
        .add_systems(Update, (settle, remember).chain());
    }
}

/// The game's window. It opens hidden, and `settle` shows it once it fills its screen,
/// so it never flashes up small or on the wrong screen first.
pub fn window() -> Window {
    Window {
        title: "Shipvivor".into(),
        resolution: (1280, 760).into(),
        visible: false,
        ..default()
    }
}

/// A screen, known by its name and where its top-left corner sits among the others.
#[derive(Clone, Debug, PartialEq)]
struct Berth {
    origin: IVec2,
    name: String,
}

impl Berth {
    fn of(monitor: &Monitor) -> Self {
        Self {
            origin: monitor.physical_position,
            name: monitor.name.clone().unwrap_or_default(),
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().splitn(3, ' ');
        let x = parts.next()?.parse().ok()?;
        let y = parts.next()?.parse().ok()?;
        let name = parts.next().unwrap_or_default().trim().to_string();
        Some(Self {
            origin: IVec2::new(x, y),
            name,
        })
    }

    fn code(&self) -> String {
        let Self { origin, name } = self;
        format!("{} {} {name}", origin.x, origin.y)
            .trim_end()
            .to_string()
    }
}

/// The screen among `screens` that `berth` names: the one with its name and corner, or
/// failing that the only one with its name, since rearranging the screens moves corners.
fn find<T: Copy>(berth: &Berth, screens: &[(T, Berth)]) -> Option<T> {
    if let Some((screen, _)) = screens.iter().find(|(_, other)| other == berth) {
        return Some(*screen);
    }
    let mut named = screens.iter().filter(|(_, other)| other.name == berth.name);
    match (named.next(), named.next()) {
        (Some((screen, _)), None) => Some(*screen),
        _ => None,
    }
}

/// Where to put the window's corner, in the physical pixels Bevy positions it by, so it
/// lands just inside a screen whose corner is `origin` at `screen_scale`. macOS places
/// windows in points, and winit turns the physical position into points at the window's
/// own scale rather than the target screen's, so on a Mac the corner is first taken back
/// to points. Elsewhere screens and windows share one physical space.
fn corner(origin: IVec2, screen_scale: f64, window_scale: f64) -> IVec2 {
    let (x, y) = if cfg!(target_os = "macos") {
        let point = |at: i32| (at as f64 / screen_scale + INSET) * window_scale;
        (point(origin.x), point(origin.y))
    } else {
        let inset = INSET * screen_scale;
        (origin.x as f64 + inset, origin.y as f64 + inset)
    };
    IVec2::new(x.round() as i32, y.round() as i32)
}

#[derive(Resource)]
struct Berths {
    /// The screen the window was last on.
    kept: Option<Berth>,
    /// Whether the window has been placed and shown.
    settled: bool,
}

/// Once the window is up, move it to the screen it was last on, fill that screen and
/// show it. Bevy applies the move before the fill, and the fill before the showing.
fn settle(
    mut waited: Local<u32>,
    mut berths: ResMut<Berths>,
    mut window: Query<&mut Window, (With<PrimaryWindow>, With<RawHandleWrapper>)>,
    monitors: Query<(Entity, &Monitor)>,
) {
    if berths.settled {
        return;
    }
    let Ok(mut window) = window.single_mut() else {
        return;
    };
    if monitors.is_empty() && *waited < PATIENCE {
        *waited += 1;
        return;
    }
    berths.settled = true;
    let screens: Vec<_> = monitors.iter().map(|(e, m)| (e, Berth::of(m))).collect();
    if let Some(screen) = berths.kept.as_ref().and_then(|kept| find(kept, &screens))
        && let Ok((_, monitor)) = monitors.get(screen)
    {
        let at = corner(
            monitor.physical_position,
            monitor.scale_factor,
            window.resolution.base_scale_factor() as f64,
        );
        window.position = WindowPosition::At(at);
    } else if berths.kept.is_some() {
        // The screen it was last on has gone, so it may have opened off every screen.
        window.position = WindowPosition::Centered(MonitorSelection::Primary);
    }
    window.set_maximized(true);
    window.visible = true;
}

/// Keep the screen the window is on whenever it moves to another one.
fn remember(
    mut berths: ResMut<Berths>,
    window: Query<Ref<OnMonitor>, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
) {
    let Ok(on) = window.single() else {
        return;
    };
    if !berths.settled || !on.is_changed() {
        return;
    }
    let Ok(monitor) = monitors.get(on.0) else {
        return;
    };
    let berth = Berth::of(monitor);
    if berths.kept.as_ref() != Some(&berth) {
        stow(&berth);
        berths.kept = Some(berth);
    }
}

/// Where the window's screen is kept between launches.
fn locker() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SHIPVIVOR_WINDOW_FILE") {
        return Some(path.into());
    }
    crate::ensign::settings_folder().map(|folder| folder.join("window.txt"))
}

/// The screen the window was last on. Tests open on none.
fn kept() -> Option<Berth> {
    if cfg!(test) {
        return None;
    }
    stowed(&locker()?)
}

fn stowed(path: &Path) -> Option<Berth> {
    Berth::parse(&std::fs::read_to_string(path).ok()?)
}

fn stow_at(path: &Path, berth: &Berth) -> std::io::Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, format!("{}\n", berth.code()))
}

/// Keep the window's screen for the next launch. Tests keep nothing.
fn stow(berth: &Berth) {
    if cfg!(test) {
        return;
    }
    let Some(path) = locker() else {
        warn!("Nowhere to keep the window's screen: no settings folder");
        return;
    };
    if let Err(err) = stow_at(&path, berth) {
        warn!(
            "Could not keep the window's screen in {}: {err}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berth(x: i32, y: i32, name: &str) -> Berth {
        Berth {
            origin: IVec2::new(x, y),
            name: name.into(),
        }
    }

    #[test]
    fn the_screen_is_kept_between_launches() {
        let folder = std::env::temp_dir().join(format!("shipvivor-window-{}", std::process::id()));
        let path = folder.join("deeper").join("window.txt");
        assert_eq!(stowed(&path), None);
        for kept in [
            berth(-3840, -400, "DELL U2720Q (2)"),
            berth(0, 0, "Built-in Retina Display"),
            berth(2560, 0, ""),
        ] {
            stow_at(&path, &kept).unwrap();
            assert_eq!(stowed(&path), Some(kept));
        }
        std::fs::write(&path, "left screen").unwrap();
        assert_eq!(stowed(&path), None);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn the_window_finds_its_screen_by_name_once_the_screens_are_rearranged() {
        let laptop = berth(0, 0, "Built-in Retina Display");
        let screens = [(1, laptop.clone()), (2, berth(3024, -200, "LG HDR 4K"))];
        assert_eq!(find(&laptop, &screens), Some(1));
        assert_eq!(find(&berth(-3840, 0, "LG HDR 4K"), &screens), Some(2));
        assert_eq!(find(&berth(3024, -200, "Studio Display"), &screens), None);

        // Two screens of one model are told apart only by where they sit.
        let twins = [(1, berth(0, 0, "U2720Q")), (2, berth(2560, 0, "U2720Q"))];
        assert_eq!(find(&berth(2560, 0, "U2720Q"), &twins), Some(2));
        assert_eq!(find(&berth(-2560, 0, "U2720Q"), &twins), None);
    }

    #[test]
    fn the_window_lands_inside_a_screen_of_another_scale() {
        // A 1x screen to the right of a 2x laptop, whose 1512 points wide are 3024 pixels.
        let at = corner(IVec2::new(1512, 0), 1., 2.);
        if cfg!(target_os = "macos") {
            // Taken back to points at the window's 2x, it sits 64 points into that screen.
            assert_eq!(at, IVec2::new((1512 + 64) * 2, 64 * 2));
        } else {
            assert_eq!(at, IVec2::new(1512 + 64, 64));
        }
        assert_eq!(corner(IVec2::new(-6048, 0), 2., 2.).x, (-3024 + 64) * 2);
    }
}
