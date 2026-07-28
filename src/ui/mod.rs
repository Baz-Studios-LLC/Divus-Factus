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
}

/// At most this many bubbles at once: sparse is the point. If everyone
/// talks over each other, nobody is worth watching.
const BUBBLE_CAP: usize = 7;

/// Spawns a bubble per Say, skipping speakers who already have one.
fn speak(
    mut commands: Commands,
    time: Res<Time>,
    mut messages: MessageReader<Say>,
    live: Query<&Bubble>,
) {
    for say in messages.read() {
        if live.iter().count() >= BUBBLE_CAP || live.iter().any(|b| b.speaker == say.speaker) {
            continue;
        }
        // Speech wears the gold border everything divine-adjacent wears;
        // thoughts wear a cool bone-grey one and dimmer text — readable as
        // "inner" at a glance, with no punctuation dressing.
        let border = if say.thought {
            theme::text_dim().with_alpha(0.4)
        } else {
            theme::panel_border()
        };
        let bubble = commands
            .spawn((
                Bubble {
                    speaker: say.speaker,
                    until: time.elapsed_secs() + 4.5,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-1000),
                    top: px(-1000),
                    padding: UiRect::axes(px(7), px(3)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(8)),
                    max_width: px(230),
                    ..default()
                },
                BackgroundColor(theme::panel_bg().with_alpha(0.85)),
                BorderColor::all(border),
            ))
            .id();
        commands.spawn((
            Text::new(say.text.clone()),
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
    time: Res<Time>,
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
                node.left = px(at.x - size.x * 0.5);
                node.top = px(at.y - size.y);
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
    time: Res<Time>,
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

    // The frame: no padding of its own, so the title bar can run edge to
    // edge like a real window's chrome. A soft drop shadow lifts the whole
    // thing off the world behind it.
    let root = commands
        .spawn((
            Panel,
            UiWindow,
            Node {
                min_width: px(min_width),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::panel_bg()),
            BorderColor::all(theme::panel_border()),
            BoxShadow::new(Color::BLACK.with_alpha(0.5), px(3), px(6), px(0), px(16)),
            Interaction::default(),
            ChildOf(strip),
        ))
        .id();

    let title_bar = commands
        .spawn((
            DragHandle(root),
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: px(16),
                padding: UiRect::axes(px(theme::PAD), px(7)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundColor(theme::title_bg()),
            BorderColor::all(theme::panel_border()),
            Interaction::default(),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Text::new(title),
        TextFont {
            font_size: FontSize::Px(15.0),
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
                width: px(18),
                height: px(18),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BorderColor::all(theme::panel_border()),
            Interaction::default(),
            ChildOf(title_bar),
        ))
        .id();
    commands.spawn((dim("x"), ChildOf(close)));

    // The gold thread under the chrome: one bright line, then the body.
    commands.spawn((
        Node {
            width: percent(100),
            height: px(2),
            ..default()
        },
        BackgroundColor(theme::accent().with_alpha(0.35)),
        ChildOf(root),
    ));

    let body = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                padding: UiRect::all(px(theme::PAD)),
                ..default()
            },
            ChildOf(root),
        ))
        .id();

    // Corner studs: four small gold squares, the craftsman's touch that says
    // this frame was made, not printed.
    for (left, top) in [(false, false), (true, false), (false, true), (true, true)] {
        let auto = Val::Auto;
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: if left { px(3) } else { auto },
                right: if left { auto } else { px(3) },
                top: if top { px(3) } else { auto },
                bottom: if top { auto } else { px(3) },
                width: px(4),
                height: px(4),
                ..default()
            },
            BackgroundColor(theme::accent().with_alpha(0.55)),
            ChildOf(root),
        ));
    }

    WindowHandles {
        root,
        title_bar,
        body,
    }
}

/// A scrollable region: the wheel moves it while the pointer is over it.
#[derive(Component)]
pub struct Scrollable;

/// The wheel scrolls whatever scrollable region it is over. Bevy clamps the
/// position to the content, so this only has to push.
pub fn scroll_regions(
    mouse_scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    mut regions: Query<(&Interaction, &mut ScrollPosition), With<Scrollable>>,
) {
    if mouse_scroll.delta.y == 0.0 {
        return;
    }
    for (interaction, mut scroll) in &mut regions {
        if *interaction != Interaction::None {
            scroll.0.y -= mouse_scroll.delta.y * 18.0;
        }
    }
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
    let window = self::window(commands, title, list_width + 320.0);
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
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                // Content never bleeds past the window; it is cut at the sill.
                overflow: Overflow::clip(),
                ..default()
            },
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
