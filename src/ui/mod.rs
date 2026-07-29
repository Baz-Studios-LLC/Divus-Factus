//! The interface kit.
//!
//! Every panel, label and readout in the game goes through here, for two reasons.
//! The practical one: the HUD, the inspector, the coming prayer queue and doctrine
//! panels all want the same frame, the same type, the same colours, and copying
//! that styling around means it drifts. The aesthetic one: the interface is part of
//! the world's look, so its colours come from the same master palette as the
//! terrain and the villagers' clothes — the UI is lit by the same art direction,
//! even though it is flat.
//!
//! The other thing this module owns is the boundary between world and interface.
//! [`PointerContext`] knows, each frame, whether the cursor is over a panel or over
//! the world. The Divine Hand reads it to become the UI cursor — gliding over
//! panels in a pointing pose instead of reaching into the terrain behind them —
//! and the picking systems read it so a click on a panel never grabs a villager
//! who happens to be rendered underneath.

use bevy::prelude::*;

use crate::palette;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerContext>()
            .init_resource::<WindowDrag>()
            .add_message::<Notice>()
            .add_message::<Say>()
            .add_systems(Startup, spawn_toast_shelf)
            .add_systems(PreUpdate, track_pointer.after(bevy::ui::UiSystems::Focus))
            .add_systems(
                Update,
                (
                    show_notices,
                    age_toasts,
                    style_buttons,
                    drag_windows,
                    close_windows,
                    scroll_regions,
                    focus_windows,
                    switch_tabs,
                    speak,
                    float_bubbles,
                ),
            );
    }
}

/// A line said or thought in the world: shown as a small bubble floating
/// over the speaker's head. Thoughts are for the god's eyes only — the
/// voyeur's reward for watching the right person at the right moment.
#[derive(Message)]
pub struct Say {
    pub speaker: Entity,
    pub text: String,
    pub thought: bool,
}

/// A live bubble, following its speaker until it fades.
#[derive(Component)]
struct Bubble {
    speaker: Entity,
    until: f32,
    /// How far above the head the box floats — a thought's circle trail
    /// needs more room than a speech tail.
    lift: f32,
}

/// At most this many bubbles at once: sparse is the point. If everyone
/// talks over each other, nobody is worth watching.
const BUBBLE_CAP: usize = 7;

/// Spawns a bubble per Say, skipping speakers who already have one.
fn speak(
    mut commands: Commands,
    // Real time: a bubble is for the player's eyes, and must stay
    // readable however hard the world is hasted.
    time: Res<Time<Real>>,
    mut messages: MessageReader<Say>,
    names: Query<&crate::villager::Person>,
    live: Query<&Bubble>,
) {
    for say in messages.read() {
        if live.iter().count() >= BUBBLE_CAP || live.iter().any(|b| b.speaker == say.speaker) {
            continue;
        }
        // Speech wears the gold border everything divine-adjacent wears;
        // thoughts wear a soft blue and dimmer text — readable as "inner"
        // at a glance, with no punctuation dressing.
        let border = if say.thought {
            palette::shade(&palette::CLOTH_BLUE, 0.7).with_alpha(0.9)
        } else {
            theme::panel_border()
        };
        let bubble = commands
            .spawn((
                Bubble {
                    speaker: say.speaker,
                    until: time.elapsed_secs() + 4.5,
                    lift: if say.thought { 26.0 } else { 8.0 },
                },
                // Under all interface chrome: a window dragged over a bubble
                // must cover it.
                GlobalZIndex(-10),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-1000),
                    top: px(-1000),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1),
                    // Thoughts breathe: the billowed rim eats into the box,
                    // so the words stand further from the edge than in a
                    // straight-walled speech bubble.
                    padding: if say.thought {
                        UiRect::axes(px(15), px(9))
                    } else {
                        UiRect::axes(px(8), px(4))
                    },
                    // A thought's rim is drawn entirely by its lobes: the
                    // box itself has no border, so no straight line can
                    // ever show between the round parts.
                    border: UiRect::all(if say.thought { px(0) } else { px(1) }),
                    border_radius: BorderRadius::all(if say.thought { px(14) } else { px(8) }),
                    max_width: px(230),
                    ..default()
                },
                // Opaque, so the trimmings below can weld on seamlessly -
                // and for thoughts, FULLY opaque: the cloud is layered
                // discs, and any alpha darkens every overlap.
                BackgroundColor(if say.thought {
                    theme::panel_bg().with_alpha(1.0)
                } else {
                    theme::panel_bg()
                }),
                BorderColor::all(border),
            ))
            .id();
        if say.thought {
            // A thought is a cloud: big soft lobes touching the whole way
            // round. The outline is built in two layers - first every lobe
            // as a solid border-colour disc a ring wider, all beneath,
            // then every lobe again in the bubble's own fill. Where bumps
            // overlap, the fill covers the neighbour's ring, so the line
            // survives only on the OUTSIDE silhouette and never breaks
            // into the cloud.
            let auto = Val::Auto;
            // A modest count of lobes, sizes JITTERED neighbour to
            // neighbour - a billow beside a bump beside a billow, the way
            // a cloud actually piles up. A smooth size ramp reads as
            // uniform; only real jumps read as cloud.
            const TOP: [f32; 7] = [30.0, 22.0, 34.0, 24.0, 28.0, 21.0, 32.0];
            const UNDER: [f32; 7] = [24.0, 32.0, 22.0, 30.0, 21.0, 33.0, 26.0];
            let mut rim: Vec<(Val, Val, Val, Val, f32)> = Vec::new();
            for (i, (&size, &under)) in TOP.iter().zip(UNDER.iter()).enumerate() {
                let pc = i as f32 * 14.4 - 2.0;
                rim.push((percent(pc), px(-size * 0.40), auto, auto, size));
                rim.push((percent(pc + 7.0), auto, auto, px(-under * 0.40), under));
            }
            for (pc, size) in [(12.0, 24.0), (52.0, 27.0)] {
                rim.push((px(-size * 0.40), percent(pc), auto, auto, size));
                rim.push((auto, percent(pc + 14.0), px(-size * 0.40), auto, size - 4.0));
            }
            for pass in 0..2 {
                for (left, top, right, bottom, size) in &rim {
                    let grow = if pass == 0 { 2.6 } else { 0.0 };
                    // The bigger under-disc stays concentric with its
                    // fill: nudged back toward whichever edges anchor it.
                    let dx = if *left != Val::Auto {
                        -grow * 0.5
                    } else {
                        grow * 0.5
                    };
                    let dy = if *top != Val::Auto {
                        -grow * 0.5
                    } else {
                        grow * 0.5
                    };
                    commands.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: *left,
                            top: *top,
                            right: *right,
                            bottom: *bottom,
                            width: px(size + grow),
                            height: px(size + grow),
                            border_radius: BorderRadius::all(percent(50)),
                            ..default()
                        },
                        UiTransform {
                            translation: Val2::new(px(dx), px(dy)),
                            ..default()
                        },
                        // Fully opaque, both layers: any alpha at all and
                        // every overlap darkens like a Venn diagram.
                        BackgroundColor(if pass == 0 {
                            border.with_alpha(1.0)
                        } else {
                            theme::panel_bg().with_alpha(1.0)
                        }),
                        ChildOf(bubble),
                    ));
                }
            }
            // The cover: the box's own fill, laid over every lobe's
            // inner half. Spawned after the lobes, before the words.
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(1),
                    top: px(1),
                    right: px(1),
                    bottom: px(1),
                    border_radius: BorderRadius::all(px(13)),
                    ..default()
                },
                BackgroundColor(theme::panel_bg().with_alpha(1.0)),
                ChildOf(bubble),
            ));
            // The trail: detached circles shrinking down toward whoever
            // is thinking it. These keep their full rings.
            for (pc, hang, size) in [(46.0, 12.0, 10.0), (43.0, 22.0, 6.0)] {
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(pc),
                        bottom: px(-hang),
                        width: px(size),
                        height: px(size),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(percent(50)),
                        ..default()
                    },
                    BackgroundColor(theme::panel_bg()),
                    BorderColor::all(border),
                    ChildOf(bubble),
                ));
            }
        } else {
            // The tail: a square in the bubble's own fill, turned 45
            // degrees and hung half out of the bottom edge. Only its two
            // lower edges wear the border, so what shows is a bordered
            // triangle pointing at whoever is talking.
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(50),
                    bottom: px(-4),
                    width: px(10),
                    height: px(10),
                    border: UiRect {
                        right: px(1),
                        bottom: px(1),
                        ..default()
                    },
                    ..default()
                },
                UiTransform {
                    translation: Val2::new(percent(-50), px(0)),
                    rotation: Rot2::degrees(45.0),
                    ..default()
                },
                BackgroundColor(theme::panel_bg()),
                BorderColor {
                    right: border,
                    bottom: border,
                    ..default()
                },
                ChildOf(bubble),
            ));
        }
        // The name over the words, so a crowd's chatter has owners.
        if let Ok(person) = names.get(say.speaker) {
            commands.spawn((
                Text::new(person.name.clone()),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(theme::accent().with_alpha(0.9)),
                ChildOf(bubble),
            ));
        }
        commands.spawn((
            Text::new(say.text.clone()),
            // The text itself must know the wrap width: a shrink-wrapped
            // bubble measures its text at full one-line width, and the
            // late wrap spills lines out the bottom of the border. The
            // width is the box minus ITS OWN padding - a thought pads
            // wider, so its text wraps sooner.
            Node {
                max_width: px(if say.thought { 200.0 } else { 212.0 }),
                ..default()
            },
            TextFont {
                font_size: FontSize::Px(theme::SMALL_SIZE),
                ..default()
            },
            TextColor(if say.thought {
                theme::text_dim()
            } else {
                theme::text()
            }),
            ChildOf(bubble),
        ));
    }
}

/// Bubbles ride above their speakers' heads and fade off with them.
fn float_bubbles(
    mut commands: Commands,
    time: Res<Time<Real>>,
    cameras: Query<(&bevy::camera::Camera, &GlobalTransform)>,
    speakers: Query<&GlobalTransform, Without<Bubble>>,
    mut bubbles: Query<(Entity, &Bubble, &mut Node, &ComputedNode, &mut Visibility)>,
) {
    let Some((camera, camera_at)) = cameras
        .iter()
        .find(|(camera, _)| camera.order == 0 && camera.is_active)
    else {
        return;
    };
    // Bubbles already settled this frame, so later ones can stack clear.
    let mut placed: Vec<Rect> = Vec::new();
    for (entity, bubble, mut node, computed, mut visibility) in &mut bubbles {
        if time.elapsed_secs() > bubble.until {
            commands.entity(entity).despawn();
            continue;
        }
        let Ok(speaker) = speakers.get(bubble.speaker) else {
            commands.entity(entity).despawn();
            continue;
        };
        let overhead = speaker.translation() + Vec3::Y * 2.3;
        match camera.world_to_viewport(camera_at, overhead) {
            Ok(at) => {
                let size = computed.size() * computed.inverse_scale_factor();
                // Lifted enough that the tail's point, not the box, meets
                // the top of the speaker's head.
                let mut pos = Vec2::new(at.x - size.x * 0.5, at.y - size.y - bubble.lift);
                // Two people talking shoulder to shoulder must not talk
                // over each other's words: a bubble that would land on an
                // earlier one climbs until it sits clear above it.
                // The footprint counts the trimmings hanging under the box
                // — a thought's circle trail reaches well below it, and
                // must not drift into the bubble stacked beneath.
                let footprint = size + Vec2::new(0.0, bubble.lift);
                let mut guard = 0;
                loop {
                    let rect = Rect::from_corners(pos, pos + footprint).inflate(3.0);
                    let Some(hit) = placed.iter().find(|p| !p.intersect(rect).is_empty()) else {
                        break;
                    };
                    pos.y = hit.min.y - footprint.y - 8.0;
                    guard += 1;
                    if guard > 8 {
                        break;
                    }
                }
                placed.push(Rect::from_corners(pos, pos + footprint));
                node.left = px(pos.x);
                node.top = px(pos.y);
                *visibility = Visibility::Visible;
            }
            Err(_) => {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// Something worth telling the player, sent from anywhere in the simulation.
/// Ordinary notices pass quietly through the bottom-right; fanfares are for
/// the moments a village remembers — a founding, a naming.
#[derive(Message)]
pub struct Notice {
    pub text: String,
    pub fanfare: bool,
}

impl Notice {
    pub fn new(text: impl Into<String>) -> Self {
        Notice {
            text: text.into(),
            fanfare: false,
        }
    }

    pub fn fanfare(text: impl Into<String>) -> Self {
        Notice {
            text: text.into(),
            fanfare: true,
        }
    }
}

/// The bottom-right column toasts stack into, newest at the bottom.
#[derive(Component)]
struct ToastShelf;

/// One visible notice, counting down to its exit.
#[derive(Component)]
struct Toast {
    remaining: f32,
    /// Border alpha when fully present, so the fade knows where it started.
    border_alpha: f32,
    fanfare: bool,
}

/// How many notices may be on screen before the oldest is pushed out.
const TOAST_CAP: usize = 6;

fn spawn_toast_shelf(mut commands: Commands) {
    commands.spawn((
        Name::new("Notices"),
        ToastShelf,
        Node {
            position_type: PositionType::Absolute,
            right: px(theme::MARGIN),
            bottom: px(theme::MARGIN),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(6),
            ..default()
        },
    ));
}

/// Turns queued notices into toasts. Deliberately *not* kit panels: a toast
/// must never catch the pointer or block a grab happening under it.
fn show_notices(
    mut commands: Commands,
    mut notices: MessageReader<Notice>,
    shelf: Query<Entity, With<ToastShelf>>,
    toasts: Query<(Entity, &Toast)>,
) {
    let Ok(shelf) = shelf.single() else {
        return;
    };

    for notice in notices.read() {
        // Push the stalest out to make room.
        if toasts.iter().count() >= TOAST_CAP
            && let Some((oldest, _)) = toasts
                .iter()
                .min_by(|a, b| a.1.remaining.total_cmp(&b.1.remaining))
        {
            commands.entity(oldest).despawn();
        }

        let border = if notice.fanfare {
            theme::accent().with_alpha(0.75)
        } else {
            theme::panel_border()
        };
        let toast = commands
            .spawn((
                Toast {
                    remaining: if notice.fanfare { 9.0 } else { 6.0 },
                    border_alpha: if notice.fanfare { 0.75 } else { 0.35 },
                    fanfare: notice.fanfare,
                },
                Node {
                    padding: UiRect::axes(px(12), px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(5)),
                    max_width: px(360),
                    ..default()
                },
                BackgroundColor(theme::panel_bg()),
                BorderColor::all(border),
                ChildOf(shelf),
            ))
            .id();

        let line = if notice.fanfare {
            let text = commands.spawn(heading(notice.text.clone())).id();
            text
        } else {
            commands.spawn(body(notice.text.clone())).id()
        };
        commands.entity(line).insert(ChildOf(toast));
    }
}

/// Toasts count down and fade out over their last moments rather than popping.
fn age_toasts(
    mut commands: Commands,
    // Real time: notices are for the player, not the world.
    time: Res<Time<Real>>,
    mut toasts: Query<(
        Entity,
        &mut Toast,
        &mut BackgroundColor,
        &mut BorderColor,
        &Children,
    )>,
    mut lines: Query<&mut TextColor>,
) {
    for (entity, mut toast, mut bg, mut border, children) in &mut toasts {
        toast.remaining -= time.delta_secs();
        if toast.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        let fade = (toast.remaining / 1.6).clamp(0.0, 1.0);
        bg.0 = theme::panel_bg().with_alpha(0.94 * fade);
        let border_color = if toast.fanfare {
            theme::accent()
        } else {
            theme::panel_border()
        };
        *border = BorderColor::all(border_color.with_alpha(toast.border_alpha * fade));
        for child in children {
            if let Ok(mut line) = lines.get_mut(*child) {
                let base = if toast.fanfare {
                    theme::accent()
                } else {
                    theme::text()
                };
                line.0 = base.with_alpha(fade);
            }
        }
    }
}

/// Colours and metrics, all derived from the master palette.
///
/// Nothing in the interface picks its own colour; it picks a *role* — panel,
/// heading, body, dim, accent — and the role resolves here, once.
pub mod theme {
    use super::*;

    /// Panel background: near-black with the palette's cool cast. Nearly
    /// opaque — a bright landscape bleeding through the panel is what makes
    /// long dim text unreadable over grass.
    pub fn panel_bg() -> Color {
        palette::shade(&palette::STONE, 0.05).with_alpha(0.94)
    }

    /// Title-bar fill: a shade lighter than the panel, so the chrome reads
    /// as a distinct part the way real windows do.
    pub fn title_bg() -> Color {
        palette::shade(&palette::STONE, 0.13).with_alpha(0.98)
    }

    /// The warm card: a parchment-dark fill for detail panes, so content
    /// areas read as two materials - dark wells beside warm boards - the
    /// way a built interface does, without leaving the palette.
    pub fn card_bg() -> Color {
        Color::srgb(0.16, 0.13, 0.095)
    }

    /// The card's stronger golden edge.
    pub fn card_border() -> Color {
        palette::shade(&palette::CLOTH_GOLD, 0.6).with_alpha(0.55)
    }

    /// Panel edge: a whisper of the gold that marks everything divine.
    pub fn panel_border() -> Color {
        palette::shade(&palette::CLOTH_GOLD, 0.55).with_alpha(0.35)
    }

    /// Primary text.
    pub fn text() -> Color {
        palette::shade(&palette::BONE, 0.97)
    }

    /// Secondary text: labels, hints, anything the eye should find second —
    /// but still *found*. Warm bone rather than cold stone: the stone ramp
    /// never gets bright enough to read over a dark panel.
    pub fn text_dim() -> Color {
        palette::shade(&palette::BONE, 0.78)
    }

    /// Emphasis: titles and the occasional word that matters.
    pub fn accent() -> Color {
        palette::shade(&palette::CLOTH_GOLD, 0.85)
    }

    pub const TITLE_SIZE: f32 = 13.0;
    pub const BODY_SIZE: f32 = 13.0;
    pub const SMALL_SIZE: f32 = 12.0;

    /// Inner padding of a panel.
    pub const PAD: f32 = 12.0;
    /// Gap between rows inside a panel.
    pub const GAP: f32 = 5.0;
    /// Distance from the window edge to a panel.
    pub const MARGIN: f32 = 10.0;
    /// Width of the label column in a stat row. One width everywhere is what
    /// makes stacked rows read as a table instead of a list of sentences.
    pub const LABEL_WIDTH: f32 = 112.0;
}

/// Where a panel sits on screen. Panels anchor to window corners rather than
/// being positioned freely — a god game's interface is furniture, not windows
/// to be dragged about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopRight,
    /// Claimed by the prayer queue in the next milestone.
    #[allow(dead_code)]
    BottomLeft,
    /// Claimed by the doctrine panel in the next milestone.
    #[allow(dead_code)]
    BottomRight,
}

impl Anchor {
    /// Layout node placing a panel in its corner.
    fn node(self) -> Node {
        let auto = Val::Auto;
        let m = px(theme::MARGIN);
        let (top, right, bottom, left) = match self {
            Anchor::TopLeft => (m, auto, auto, m),
            Anchor::TopRight => (m, m, auto, auto),
            Anchor::BottomLeft => (auto, auto, m, m),
            Anchor::BottomRight => (auto, m, m, auto),
        };
        Node {
            position_type: PositionType::Absolute,
            top,
            right,
            bottom,
            left,
            flex_direction: FlexDirection::Column,
            row_gap: px(theme::GAP),
            padding: UiRect::all(px(theme::PAD)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(5)),
            ..default()
        }
    }
}

/// Marks a kit-made panel. [`track_pointer`] watches these to know when the
/// cursor has left the world for the interface.
#[derive(Component)]
pub struct Panel;

/// The pieces of a spawned panel a caller may want to reach.
pub struct PanelHandles {
    /// The panel itself; content goes here as children.
    pub root: Entity,
    /// The title bar, if the panel has one. A flex row — anything added to it
    /// after the title lands on its right edge, which is where a panel puts a
    /// live readout that belongs to the whole panel (the HUD's fps, say).
    pub title_bar: Option<Entity>,
}

/// Spawns a standard panel: anchored frame, translucent background, hairline
/// border, an optional title bar with a divider, and an optional minimum width
/// so panels with changing content hold their shape instead of breathing.
pub fn panel(
    commands: &mut Commands,
    anchor: Anchor,
    title: Option<&str>,
    min_width: Option<f32>,
) -> PanelHandles {
    let mut node = anchor.node();
    if let Some(width) = min_width {
        node.min_width = px(width);
    }

    let root = commands
        .spawn((
            Panel,
            node,
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border()),
            // Lets the UI focus pass track hover, which is how the pointer knows
            // it has left the world.
            Interaction::default(),
        ))
        .id();

    let title_bar = title.map(|title| {
        let bar = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Baseline,
                    column_gap: px(16),
                    padding: UiRect::bottom(px(6)),
                    margin: UiRect::bottom(px(2)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::panel_border()),
                ChildOf(root),
            ))
            .id();
        let text = commands.spawn(heading(title)).id();
        commands.entity(text).insert(ChildOf(bar));
        bar
    });

    PanelHandles { root, title_bar }
}

/// A movable, closable window's root. Open/closed is its `Visibility`.
#[derive(Component)]
pub struct UiWindow;

/// A window's drag handle; points at the window root it moves.
#[derive(Component)]
pub struct DragHandle(pub Entity);

/// A window's close button; points at the window root it hides.
#[derive(Component)]
pub struct CloseButton(pub Entity);

/// The drag in progress, if any: which window, and where inside it the
/// pointer took hold.
#[derive(Resource, Default)]
pub struct WindowDrag {
    active: Option<(Entity, Vec2)>,
}

/// The pieces of a window a caller may want: the root (show/hide), the title
/// bar, and the body container its content goes into.
pub struct WindowHandles {
    pub root: Entity,
    #[allow(dead_code)]
    pub title_bar: Entity,
    pub body: Entity,
}

/// A real window: a titled panel with a drag handle for a title bar and a
/// close button in its corner. Content goes in `body`, so callers can clear
/// and rebuild it without touching the chrome.
///
/// Windows open centred on the left side of the screen, then go wherever
/// they are dragged. The full-screen strip only does the opening layout: it
/// catches no pointer, and an absolutely-positioned (dragged) window ignores
/// it entirely.
pub fn window(commands: &mut Commands, title: &str, min_width: f32) -> WindowHandles {
    window_impl(commands, title, min_width, false)
}

/// A window that opens dead centre — for the pages big enough to *be* the
/// screen for a moment, like the village ledger.
pub fn big_window(commands: &mut Commands, title: &str, min_width: f32) -> WindowHandles {
    window_impl(commands, title, min_width, true)
}

fn window_impl(
    commands: &mut Commands,
    title: &str,
    min_width: f32,
    centred: bool,
) -> WindowHandles {
    let strip = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            right: px(0),
            bottom: px(0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: if centred {
                AlignItems::Center
            } else {
                AlignItems::FlexStart
            },
            padding: if centred {
                UiRect::default()
            } else {
                UiRect::left(px(theme::MARGIN))
            },
            ..default()
        })
        .id();

    // The outer rim: a near-black plate with the deep shadow. Inside it, a
    // warm bezel frames the content - two materials, like a built thing.
    let root = commands
        .spawn((
            Panel,
            UiWindow,
            Node {
                min_width: px(min_width),
                flex_direction: FlexDirection::Column,
                padding: px(5).into(),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.045, 0.04)),
            BorderColor::all(Color::BLACK.with_alpha(0.8)),
            BoxShadow::new(Color::BLACK.with_alpha(0.65), px(4), px(9), px(2), px(26)),
            Interaction::default(),
            ChildOf(strip),
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(9)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::card_border()),
            ChildOf(root),
        ))
        .id();

    // The banner: a tall title plate with the name writ large.
    let title_bar = commands
        .spawn((
            DragHandle(root),
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: px(16),
                padding: UiRect::axes(px(theme::PAD + 6.0), px(12)),
                ..default()
            },
            BackgroundColor(theme::title_bg()),
            Interaction::default(),
            ChildOf(frame),
        ))
        .id();
    commands.spawn((
        Text::new(title),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(theme::accent()),
        ChildOf(title_bar),
    ));
    let close = commands
        .spawn((
            CloseButton(root),
            UiButton,
            Node {
                width: px(26),
                height: px(26),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BorderColor::all(theme::card_border()),
            Interaction::default(),
            ChildOf(title_bar),
        ))
        .id();
    commands.spawn((body("x"), ChildOf(close)));

    // The gold thread under the banner.
    commands.spawn((
        Node {
            width: percent(100),
            height: px(3),
            ..default()
        },
        BackgroundColor(theme::accent().with_alpha(0.45)),
        ChildOf(frame),
    ));

    let content = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                padding: px(theme::PAD).into(),
                ..default()
            },
            ChildOf(frame),
        ))
        .id();

    // Corner studs on the rim.
    for (left, top) in [(false, false), (true, false), (false, true), (true, true)] {
        let auto = Val::Auto;
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: if left { px(4) } else { auto },
                right: if left { auto } else { px(4) },
                top: if top { px(4) } else { auto },
                bottom: if top { auto } else { px(4) },
                width: px(5),
                height: px(5),
                border_radius: BorderRadius::all(px(2)),
                ..default()
            },
            BackgroundColor(theme::accent().with_alpha(0.6)),
            ChildOf(root),
        ));
    }

    WindowHandles {
        root,
        title_bar,
        body: content,
    }
}

/// A scrollable region: the wheel moves it while the pointer is over it.
#[derive(Component)]
pub struct Scrollable;

/// The wheel scrolls whatever scrollable region it is over. Bevy clamps the
/// position to the content, so this only has to push.
/// Clicking a window brings it to the front, the way windows anywhere
/// work. Hit-tested by geometry (clicks land on child buttons, never the
/// window root, so Interaction cannot carry this) and applied to the
/// window's full-screen strip, which is what actually stacks.
pub fn focus_windows(
    mut commands: Commands,
    buttons: Res<ButtonInput<bevy::input::mouse::MouseButton>>,
    primary: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    panels: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &ChildOf,
        ),
        With<UiWindow>,
    >,
    strips: Query<Option<&GlobalZIndex>>,
    mut stack: Local<i32>,
) {
    if !buttons.just_pressed(bevy::input::mouse::MouseButton::Left) {
        return;
    }
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    // Of every visible window under the cursor, the one already highest
    // is the one the player sees themself clicking.
    let mut hit: Option<(i32, Entity)> = None;
    for (computed, transform, visibility, strip) in &panels {
        if !visibility.get() {
            continue;
        }
        let scale = computed.inverse_scale_factor();
        let centre = Vec2::new(transform.translation.x, transform.translation.y) * scale;
        let half = computed.size() * scale * 0.5;
        if (cursor.x - centre.x).abs() > half.x || (cursor.y - centre.y).abs() > half.y {
            continue;
        }
        let z = strips.get(strip.parent()).ok().flatten().map_or(0, |z| z.0);
        if hit.is_none_or(|(top, _)| z >= top) {
            hit = Some((z, strip.parent()));
        }
    }
    let Some((current, strip)) = hit else {
        return;
    };
    // Already on top of the stack: nothing to raise.
    if current == *stack && *stack != 0 {
        return;
    }
    // Windows stack between the world (0) and the overlays (250+).
    *stack += 1;
    if *stack > 200 {
        *stack = 1;
    }
    commands.entity(strip).insert(GlobalZIndex(10 + *stack));
}

pub fn scroll_regions(
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    primary: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut regions: Query<
        (
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            &mut ScrollPosition,
        ),
        With<Scrollable>,
    >,
) {
    if mouse_scroll.delta.y == 0.0 {
        return;
    }
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    // Hit-test by geometry, not by Interaction: hovering a button INSIDE
    // a scrollable pane captures the hover and starved the pane of wheel
    // events — scroll that worked or not depending on which pixel the
    // cursor happened to rest on.
    for (computed, transform, visibility, mut scroll) in &mut regions {
        if !visibility.get() {
            continue;
        }
        let scale = computed.inverse_scale_factor();
        let centre = Vec2::new(transform.translation.x, transform.translation.y) * scale;
        let half = computed.size() * scale * 0.5;
        if (cursor.x - centre.x).abs() <= half.x && (cursor.y - centre.y).abs() <= half.y {
            scroll.0.y -= mouse_scroll.delta.y * 18.0;
        }
    }
}

/// A hint the cursor summons: hover anything carrying one and the card
/// in the corner names it - what it is, what it costs, what it does.
#[derive(Component)]
pub struct HoverHint {
    pub title: String,
    pub line: String,
}

impl HoverHint {
    pub fn new(title: impl Into<String>, line: impl Into<String>) -> Self {
        HoverHint {
            title: title.into(),
            line: line.into(),
        }
    }
}

/// A dark inset well: content that sits INTO the panel.
pub fn inset_well(commands: &mut Commands, parent: Entity) -> Entity {
    commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                padding: px(8).into(),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.35)),
            ChildOf(parent),
        ))
        .id()
}

/// A warm bordered card: the parchment board detail content sits ON.
pub fn detail_card(commands: &mut Commands, parent: Entity) -> Entity {
    commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                padding: px(theme::PAD).into(),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(theme::card_bg()),
            BorderColor::all(theme::card_border()),
            ChildOf(parent),
        ))
        .id()
}

/// A square content tile - for miracle slots, item cells, icon grids. Lit
/// tiles wear the gold edge; dim ones sit back.
pub fn tile(commands: &mut Commands, parent: Entity, size: f32, lit: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: px(size),
                height: px(size),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(2),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(if lit {
                theme::title_bg()
            } else {
                Color::BLACK.with_alpha(0.3)
            }),
            BorderColor::all(if lit {
                theme::card_border()
            } else {
                theme::panel_border().with_alpha(0.2)
            }),
            ChildOf(parent),
        ))
        .id()
}

/// The panes of a split view: the window chrome, the list on the left, and
/// the detail pane on the right. Any window that pairs a roster with a
/// close-up builds on this.
pub struct SplitView {
    pub window: WindowHandles,
    pub list: Entity,
    pub detail: Entity,
}

/// A two-pane window: a fixed-width list column beside a detail pane, split
/// by a hairline. The caller fills both sides; the chrome is shared.
pub fn split_view(commands: &mut Commands, title: &str, list_width: f32, height: f32) -> SplitView {
    // A generous FIXED frame: content must never resize the window - a
    // panel that breathes with every text change is seasickness, not UI.
    let window = self::window(commands, title, list_width + 500.0);
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(height),
                flex_direction: FlexDirection::Row,
                column_gap: px(theme::PAD),
                ..default()
            },
            ChildOf(window.body),
        ))
        .id();
    let list = commands
        .spawn((
            Node {
                width: px(list_width),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(px(6)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            // An inset well, a step darker than the panel: the roster reads
            // as content *inside* the window, not text floating on it.
            BackgroundColor(Color::BLACK.with_alpha(0.35)),
            Scrollable,
            ScrollPosition::DEFAULT,
            Interaction::default(),
            ChildOf(row),
        ))
        .id();
    let detail = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                // Flexbox lets long text push its container wide unless the
                // minimum is pinned; zero it so words wrap instead.
                min_width: px(0),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                padding: px(theme::PAD).into(),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(8)),
                // Content never bleeds past the window - it scrolls.
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme::card_bg()),
            BorderColor::all(theme::card_border()),
            Scrollable,
            ScrollPosition::DEFAULT,
            Interaction::default(),
            ChildOf(row),
        ))
        .id();
    SplitView {
        window,
        list,
        detail,
    }
}

/// Windows move by their title bars, like windows anywhere.
pub fn drag_windows(
    primary: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut drag: ResMut<WindowDrag>,
    handles: Query<(&DragHandle, &Interaction)>,
    mut roots: Query<(&mut Node, &ComputedNode, &UiGlobalTransform)>,
) {
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };

    if buttons.just_pressed(MouseButton::Left) {
        for (handle, interaction) in &handles {
            if *interaction != Interaction::None
                && let Ok((_, computed, transform)) = roots.get(handle.0)
            {
                let scale = computed.inverse_scale_factor();
                let centre = Vec2::new(transform.translation.x, transform.translation.y) * scale;
                let top_left = centre - computed.size() * scale * 0.5;
                drag.active = Some((handle.0, cursor - top_left));
            }
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        drag.active = None;
        return;
    }
    let Some((window, grip)) = drag.active else {
        return;
    };
    if let Ok((mut node, _, _)) = roots.get_mut(window) {
        node.position_type = PositionType::Absolute;
        node.left = px(cursor.x - grip.x);
        node.top = px(cursor.y - grip.y);
        node.right = Val::Auto;
        node.bottom = Val::Auto;
    }
}

/// The corner button does what corner buttons do.
pub fn close_windows(
    clicks: Query<(&CloseButton, &Interaction), Changed<Interaction>>,
    mut visibility: Query<&mut Visibility>,
) {
    for (close, interaction) in &clicks {
        if *interaction == Interaction::Pressed
            && let Ok(mut visibility) = visibility.get_mut(close.0)
        {
            *visibility = Visibility::Hidden;
        }
    }
}

/// The handles of a gauge row: the fill to widen and the value to write.
pub struct Gauge {
    pub fill: Entity,
    pub value: Entity,
}

/// A labelled bar gauge. The number is present but small; the bar does the
/// talking — this is what keeps a stats page from being a wall of text.
pub fn gauge_row(commands: &mut Commands, parent: Entity, label_text: &str, color: Color) -> Gauge {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        label(label_text),
        Node {
            width: px(theme::LABEL_WIDTH),
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(row),
    ));
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: px(12),
                border_radius: BorderRadius::all(px(6)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.45)),
            ChildOf(row),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                bottom: px(0),
                width: percent(0),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(color),
            ChildOf(track),
        ))
        .id();
    let value = commands
        .spawn((
            dim(""),
            Node {
                width: px(58),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    Gauge { fill, value }
}

/// One tab button in a bar; clicking it shows its page and hides its
/// siblings' pages.
#[derive(Component)]
pub struct TabButton {
    pub bar: Entity,
    pub page: Entity,
}

/// A tabbed row plus one content page per label. The first tab starts
/// active. Pages are plain columns the caller fills.
pub fn tab_bar(commands: &mut Commands, parent: Entity, labels: &[&str]) -> Vec<Entity> {
    let bar = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(4),
                margin: UiRect::bottom(px(4)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let mut pages = Vec::with_capacity(labels.len());
    for (index, label_text) in labels.iter().enumerate() {
        let page = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme::GAP),
                    display: if index == 0 {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                ChildOf(parent),
            ))
            .id();
        let button = commands
            .spawn((
                TabButton { bar, page },
                UiButton,
                Node {
                    padding: UiRect::axes(px(18), px(7)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::top(px(8)),
                    ..default()
                },
                BackgroundColor(if index == 0 {
                    theme::card_bg()
                } else {
                    Color::BLACK.with_alpha(0.25)
                }),
                BorderColor::all(if index == 0 {
                    theme::card_border()
                } else {
                    theme::panel_border().with_alpha(0.4)
                }),
                Interaction::default(),
                ChildOf(bar),
            ))
            .id();
        commands.spawn((
            Text::new(*label_text),
            TextFont {
                font_size: FontSize::Px(theme::SMALL_SIZE),
                ..default()
            },
            TextColor(theme::accent()),
            ChildOf(button),
        ));
        pages.push(page);
    }
    pages
}

/// Tab clicks swap the visible page within their bar.
#[allow(clippy::type_complexity)]
pub fn switch_tabs(
    clicked: Query<(Entity, &Interaction, &TabButton), Changed<Interaction>>,
    all_tabs: Query<(Entity, &TabButton)>,
    mut pages: Query<&mut Node, Without<TabButton>>,
    mut borders: Query<&mut BorderColor, With<TabButton>>,
) {
    for (pressed, interaction, tab) in &clicked {
        if *interaction != Interaction::Pressed {
            continue;
        }
        for (other, sibling) in &all_tabs {
            if sibling.bar != tab.bar {
                continue;
            }
            let active = other == pressed;
            if let Ok(mut node) = pages.get_mut(sibling.page) {
                node.display = if active { Display::Flex } else { Display::None };
            }
            if let Ok(mut border) = borders.get_mut(other) {
                *border = BorderColor::all(if active {
                    theme::card_border()
                } else {
                    theme::panel_border().with_alpha(0.4)
                });
            }
        }
    }
}

/// A ruled section header: a hairline, the label in gold, a hairline. The
/// difference between a debug dump and a page with sections.
pub fn section_header(commands: &mut Commands, parent: Entity, label_text: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8),
                margin: UiRect::top(px(4)),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let rule = |commands: &mut Commands| {
        commands
            .spawn(Node {
                flex_grow: 1.0,
                height: px(1),
                ..default()
            })
            .insert(BackgroundColor(theme::panel_border()))
            .id()
    };
    let left = rule(commands);
    commands.entity(left).insert(ChildOf(row));
    let text = commands
        .spawn((
            Text::new(label_text),
            TextFont {
                font_size: FontSize::Px(theme::SMALL_SIZE),
                ..default()
            },
            TextColor(theme::accent()),
            ChildOf(row),
        ))
        .id();
    let _ = text;
    let right = rule(commands);
    commands.entity(right).insert(ChildOf(row));
    row
}

/// The pieces of a stat row a caller may want to reach: the row itself (to show
/// and hide it) and the value text (to write to). The kit never writes values.
pub struct StatRow {
    pub row: Entity,
    pub value: Entity,
}

/// A label/value row. The label sits in a fixed-width dim column so values line
/// up down the panel; an optional dim hint (a key binding, usually) is pushed to
/// the row's right edge.
pub fn stat_row(
    commands: &mut Commands,
    panel: Entity,
    label_text: &str,
    hint: Option<&str>,
) -> StatRow {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Baseline,
                column_gap: px(10),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();

    let label_entity = commands
        .spawn((
            label(label_text),
            // Labels never wrap. A two-word label soft-wrapping inside its fixed
            // column makes the whole row two lines tall and breaks the table.
            TextLayout::linebreak(LineBreak::NoWrap),
            Node {
                width: px(theme::LABEL_WIDTH),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(label_entity).insert(ChildOf(row));

    let value = commands.spawn(body("")).id();
    commands.entity(value).insert(ChildOf(row));

    if let Some(hint) = hint {
        let hint_entity = commands
            .spawn((
                dim(hint),
                Node {
                    margin: UiRect::left(auto()),
                    ..default()
                },
            ))
            .id();
        commands.entity(hint_entity).insert(ChildOf(row));
    }

    StatRow { row, value }
}

/// A panel title or section heading.
pub fn heading(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(theme::TITLE_SIZE),
            ..default()
        },
        TextColor(theme::accent()),
    )
}

/// Ordinary readable text.
pub fn body(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(theme::BODY_SIZE),
            ..default()
        },
        TextColor(theme::text()),
    )
}

/// A row label: body-sized so it shares a baseline with its value, but dim so
/// the value is what the eye lands on.
pub fn label(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(theme::BODY_SIZE),
            ..default()
        },
        TextColor(theme::text_dim()),
    )
}

/// A section header inside a panel: small, dim, set off by a little air above.
pub fn section(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(theme::SMALL_SIZE),
            ..default()
        },
        TextColor(theme::text_dim()),
        Node {
            margin: UiRect::top(px(7)),
            ..default()
        },
    )
}

/// Quiet text: hints, key bindings, labels.
pub fn dim(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(theme::SMALL_SIZE),
            ..default()
        },
        TextColor(theme::text_dim()),
    )
}

/// A full-width, invisible strip that centres whatever is put in it. The
/// strip itself never catches the pointer; only its contents do.
pub fn centered_strip(commands: &mut Commands, top: Val, bottom: Val) -> Entity {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top,
            bottom,
            left: px(0),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id()
}

/// The top-centre toolbar: a short row of icon buttons. Centred by its strip,
/// however many buttons it grows.
pub fn toolbar(commands: &mut Commands) -> Entity {
    let strip = centered_strip(commands, px(theme::MARGIN), Val::Auto);
    let bar = commands
        .spawn((
            Name::new("Toolbar"),
            Panel,
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: px(6),
                padding: UiRect::all(px(5)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border()),
            Interaction::default(),
            ChildOf(strip),
        ))
        .id();
    bar
}

/// An icon button in the toolbar. Its icon is built from child nodes by the
/// caller — no icon fonts, no images, same as everything else in this game.
/// Returns the button entity; watch its `Interaction` for presses.
pub fn icon_button(commands: &mut Commands, toolbar: Entity) -> Entity {
    let button = commands
        .spawn((
            UiButton,
            Node {
                width: px(34),
                height: px(34),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(theme::panel_bg().with_alpha(0.4)),
            BorderColor::all(theme::panel_border()),
            Interaction::default(),
            ChildOf(toolbar),
        ))
        .id();
    button
}

/// Marks a kit button, for the shared hover/press styling.
#[derive(Component)]
pub struct UiButton;

/// Opts a button out of background restyling — for buttons whose face IS
/// their meaning, like colour swatches. They keep hover feedback via borders.
#[derive(Component)]
pub struct KeepFace;

/// One styling system for every button: brighten under the hand, dip on press.
fn style_buttons(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (With<UiButton>, Without<KeepFace>, Changed<Interaction>),
    >,
) {
    for (interaction, mut bg) in &mut buttons {
        bg.0 = match interaction {
            Interaction::None => theme::panel_bg().with_alpha(0.4),
            Interaction::Hovered => theme::accent().with_alpha(0.25),
            Interaction::Pressed => theme::accent().with_alpha(0.5),
        };
    }
}

/// Whether the pointer is over the interface or the world.
///
/// The one question every input system asks before acting: the Hand will not
/// grab through a panel, and a panel will not be clicked through by the world.
#[derive(Resource, Default)]
pub struct PointerContext {
    pub over_ui: bool,
}

/// Reads hover state off the panels after the UI focus pass has run.
fn track_pointer(panels: Query<&Interaction, With<Panel>>, mut pointer: ResMut<PointerContext>) {
    pointer.over_ui = panels
        .iter()
        .any(|interaction| !matches!(interaction, Interaction::None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_anchor_pins_exactly_two_edges() {
        // A corner anchor must pin one vertical and one horizontal edge and leave
        // the others automatic, or the panel stretches across the screen.
        for anchor in [
            Anchor::TopLeft,
            Anchor::TopRight,
            Anchor::BottomLeft,
            Anchor::BottomRight,
        ] {
            let node = anchor.node();
            let pinned_vertical =
                (node.top != Val::Auto) as u32 + (node.bottom != Val::Auto) as u32;
            let pinned_horizontal =
                (node.left != Val::Auto) as u32 + (node.right != Val::Auto) as u32;
            assert_eq!(pinned_vertical, 1, "{anchor:?}");
            assert_eq!(pinned_horizontal, 1, "{anchor:?}");
        }
    }

    #[test]
    fn theme_colors_are_opaque_enough_to_read() {
        // The panel may be translucent; the text may not.
        assert!(theme::panel_bg().alpha() < 1.0);
        assert!(theme::text().alpha() > 0.99);
        assert!(theme::accent().alpha() > 0.99);
    }
}
