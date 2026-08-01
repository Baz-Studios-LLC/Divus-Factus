//! The game's look, carried in as data.
//!
//! The palette arrives from the game via `data/palette.json` — written by
//! the game's own `export_palette_for_atelier` test — so nothing here can
//! drift from what the game actually draws. If the file is missing the
//! Atelier still opens, in bone and gold, and says so.

use bevy::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct PaletteFile {
    ramps: Vec<RampEntry>,
}

#[derive(Deserialize)]
struct RampEntry {
    name: String,
    steps: [[u8; 3]; 5],
}

/// The game's ramps, by name.
#[derive(Resource, Default)]
pub struct Palette {
    ramps: Vec<(String, [[u8; 3]; 5])>,
}

impl Palette {
    pub fn ramp(&self, name: &str) -> Option<&[[u8; 3]; 5]> {
        self.ramps
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, ramp)| ramp)
    }

    /// A step from a named ramp, 0 shadow to 1 bright — the game's own
    /// `palette::shade`, re-derived from the data.
    pub fn shade(&self, name: &str, t: f32) -> Color {
        let Some(ramp) = self.ramp(name) else {
            return Color::srgb(0.8, 0.2, 0.8); // the classic missing-colour
        };
        let idx = ((t.clamp(0.0, 1.0) * 4.0).round() as usize).min(4);
        let [r, g, b] = ramp[idx];
        Color::srgb_u8(r, g, b)
    }

    #[allow(dead_code)] // the Builder bench's ramp picker reads this
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.ramps.iter().map(|(n, _)| n.as_str())
    }
}

/// The Atelier wears the codex's colours.
pub mod theme {
    use super::Palette;
    use bevy::prelude::*;

    pub fn panel_bg() -> Color {
        Color::srgb(0.045, 0.05, 0.062).with_alpha(0.985)
    }

    pub fn panel_border(palette: &Palette) -> Color {
        palette.shade("cloth-gold", 0.55).with_alpha(0.35)
    }

    pub fn accent(palette: &Palette) -> Color {
        palette.shade("cloth-gold", 0.85)
    }

    #[allow(dead_code)] // body text, once the benches carry prose
    pub fn text(palette: &Palette) -> Color {
        palette.shade("bone", 0.97)
    }

    pub fn text_dim(palette: &Palette) -> Color {
        palette.shade("bone", 0.78)
    }
}

/// A pane the wheel scrolls when the cursor is over it.
#[derive(Component)]
pub struct Scrollable;

/// Wheel over a scrollable pane walks it, and says so, so the camera
/// knows to keep its hands off the zoom.
pub fn scroll_panes(
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    windows: Query<&Window>,
    mut panes: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &mut ScrollPosition,
        ),
        With<Scrollable>,
    >,
    mut over_pane: ResMut<OverPane>,
) {
    let cursor = windows
        .iter()
        .next()
        .and_then(|window| window.cursor_position());
    over_pane.0 = false;
    let Some(cursor) = cursor else {
        return;
    };
    // Hit-tested by geometry, not by hover: a button inside the pane
    // would otherwise swallow the wheel.
    for (computed, transform, visibility, mut scroll) in &mut panes {
        if !visibility.get() {
            continue;
        }
        let scale = computed.inverse_scale_factor();
        let centre = Vec2::new(transform.translation.x, transform.translation.y) * scale;
        let half = computed.size() * scale * 0.5;
        if (cursor.x - centre.x).abs() <= half.x && (cursor.y - centre.y).abs() <= half.y {
            over_pane.0 = true;
            if wheel.delta.y != 0.0 {
                // A line's worth per notch, and trackpads send small
                // fractions of one - both read the same speed.
                let notches = match wheel.unit {
                    bevy::input::mouse::MouseScrollUnit::Line => wheel.delta.y * 3.0,
                    bevy::input::mouse::MouseScrollUnit::Pixel => wheel.delta.y * 0.14,
                };
                scroll.0.y -= notches * 22.0;
            }
        }
    }
}

/// Whether the cursor rests over a scrollable pane - the camera reads
/// this and leaves the wheel alone.
#[derive(Resource, Default)]
pub struct OverPane(pub bool);

/// Display and body faces, the game's own.
#[derive(Resource)]
pub struct Fonts {
    pub display: Handle<Font>,
    pub text: Handle<Font>,
}

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(load_palette())
            .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.05)))
            .init_resource::<OverPane>()
            .add_systems(PreStartup, load_fonts)
            .add_systems(Update, scroll_panes);
    }
}

fn load_fonts(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Fonts {
        display: assets.load("fonts/Cinzel.ttf"),
        text: assets.load("fonts/EBGaramond.ttf"),
    });
}

/// The palette, for the bake to resolve colours with.
#[cfg(test)]
pub fn load_palette_for_bake() -> Palette {
    load_palette()
}

/// Reads the palette relative to the crate first (so `cargo run` works from
/// anywhere), then the working directory, then gives up gracefully.
fn load_palette() -> Palette {
    let mut roads = Vec::new();
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        roads.push(std::path::PathBuf::from(manifest).join("data/palette.json"));
    }
    roads.push("data/palette.json".into());
    roads.push("atelier/data/palette.json".into());

    for road in roads {
        if let Ok(text) = std::fs::read_to_string(&road)
            && let Ok(file) = serde_json::from_str::<PaletteFile>(&text)
        {
            info!(
                "palette: {} ramps from {}",
                file.ramps.len(),
                road.display()
            );
            return Palette {
                ramps: file
                    .ramps
                    .into_iter()
                    .map(|entry| (entry.name, entry.steps))
                    .collect(),
            };
        }
    }
    warn!(
        "no data/palette.json - run the game's export first: \
         cargo test export_palette_for_atelier -- --ignored"
    );
    Palette {
        ramps: vec![
            (
                "bone".to_string(),
                [
                    [60, 55, 48],
                    [105, 97, 84],
                    [152, 142, 124],
                    [199, 188, 166],
                    [237, 227, 205],
                ],
            ),
            (
                "cloth-gold".to_string(),
                [
                    [66, 48, 16],
                    [110, 82, 28],
                    [158, 121, 43],
                    [205, 163, 66],
                    [240, 205, 103],
                ],
            ),
        ],
    }
}
