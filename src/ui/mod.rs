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
        // Ordo is the UI kit, for the whole game and not just the trial.
        // Brett: "make sure all UI stuff is using Ordo... if Ordo is missing
        // something we should add it to Ordo not hand roll it in game." The
        // game lends the kit its own pigment; the kit paints from roles.
        app.add_plugins(ordo::OrdoPlugin::with_theme("theme.ordo.toml"))
            .add_systems(Startup, lend_ramps);
        app.init_resource::<PointerContext>()
            .init_resource::<SetAside>()
            .init_resource::<WindowDrag>()
            .add_message::<Notice>()
            .add_message::<Say>()
            .add_systems(
                Startup,
                (
                    spawn_toast_shelf,
                    spawn_prayer_shelf,
                    spawn_frost_glass,
                    load_fonts,
                    spawn_date_card,
                ),
            )
            .add_systems(PreUpdate, track_pointer.after(bevy::ui::UiSystems::Focus))
            .add_systems(
                Update,
                (
                    // Toasts and bubbles belong to play. The pre-game's veil is
                    // translucent now, and the world lives behind it — but its
                    // chatter, thoughts and notices stay backstage until the
                    // player has actually come down to the village.
                    show_notices.run_if(in_state(crate::GameState::Playing)),
                    keep_the_prayer_shelf.run_if(in_state(crate::GameState::Playing)),
                    set_askings_aside.run_if(in_state(crate::GameState::Playing)),
                    // The frosted glass and the standing-down shelves: one
                    // subject - the book owning the screen. The glass
                    // follows the god's own stance after the frost decides.
                    (frost_the_world, frost_glass_follows, hud_stands_down).chain(),
                    age_toasts,
                    style_buttons,
                    drag_windows,
                    close_windows,
                    prune_hidden_windows,
                    scroll_regions,
                    focus_windows,
                    dress_display_text,
                    switch_tabs,
                    speak.run_if(in_state(crate::GameState::Playing)),
                    float_bubbles,
                    update_date_card,
                ),
            );
    }
}

/// The date, written large in the world's own hand: season and day above,
/// the year small beneath. Top-left, always readable, only during play.
#[derive(Component)]
struct DateBig;

#[derive(Component)]
struct DateSmall;

#[derive(Component)]
struct DateCard;

fn spawn_date_card(mut commands: Commands) {
    let card = commands
        .spawn((
            DateCard,
            Node {
                position_type: PositionType::Absolute,
                left: px(18),
                top: px(12),
                flex_direction: FlexDirection::Column,
                row_gap: px(0),
                ..default()
            },
            GlobalZIndex(60),
            Visibility::Hidden,
        ))
        .id();
    commands.spawn((
        DateBig,
        DisplayBoldFace,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(36.0),
            ..default()
        },
        TextColor(Color::WHITE.with_alpha(0.78)),
        ChildOf(card),
    ));
    commands.spawn((
        DateSmall,
        SerifFace,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::WHITE.with_alpha(0.66)),
        Node {
            margin: UiRect::left(px(26)),
            ..default()
        },
        ChildOf(card),
    ));
}

/// Keeps the date true, twice a second, and only during play.
fn update_date_card(
    time: Res<Time<Real>>,
    mut since: Local<f32>,
    state: Res<State<crate::GameState>>,
    clock: Option<Res<crate::calendar::WorldClock>>,
    debug: Option<Res<crate::debug::DebugState>>,
    books: Query<&Visibility, (With<crate::debug::village::VillagePanel>, Without<DateCard>)>,
    mut card: Query<&mut Visibility, With<DateCard>>,
    mut big: Query<&mut Text, (With<DateBig>, Without<DateSmall>)>,
    mut small: Query<&mut Text, (With<DateSmall>, Without<DateBig>)>,
) {
    // Visibility answers the book the FRAME it moves: it used to wait on
    // the text's half-second clock, and the card trailed the codex by a
    // visible beat, both ways. Only the date's lettering is paced.
    // The F1 instrument panel owns this corner while it is up, and it
    // carries the date already.
    let book_open = books.iter().any(|v| *v != Visibility::Hidden);
    let playing = *state.get() == crate::GameState::Playing
        && !debug.is_some_and(|d| d.hud_visible)
        // The book's footer carries the date while it is open; the card
        // over the rail was two clocks fighting for one corner.
        && !book_open;
    for mut visibility in &mut card {
        let fresh = if playing {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != fresh {
            *visibility = fresh;
        }
    }
    if !playing {
        return;
    }
    *since += time.delta_secs();
    if *since < 0.5 {
        return;
    }
    *since = 0.0;
    let Some(clock) = clock else {
        return;
    };
    let season = clock.season().name();
    let mut season_cased: String = season.to_string();
    if let Some(first) = season_cased.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    let heading = format!("{} {}", season_cased, clock.day_of_season());
    let sub = format!("YEAR {} - {}", clock.year(), clock.phase_name());
    if let Ok(mut text) = big.single_mut()
        && text.0 != heading
    {
        *text = Text::new(heading);
    }
    if let Ok(mut text) = small.single_mut()
        && text.0 != sub
    {
        *text = Text::new(sub);
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
    /// Addressed to the god: a prayer's bubble wears pink — the one channel
    /// aimed at the player, worth catching from the corner of an eye.
    pub prayer: bool,
}

/// Dresses a line for its bubble: a capital to open, a full stop to close
/// unless the line already ends in its own mark. Every bubble passes through
/// here, so the hand-written corpus and the composed lines wear the same
/// grammar without editing two hundred strings.
fn sentence(text: &str) -> String {
    let trimmed = text.trim();
    let mut out = String::with_capacity(trimmed.len() + 1);
    let mut chars = trimmed.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
    if !matches!(
        out.chars().last(),
        Some('.') | Some('?') | Some('!') | Some('…') | None
    ) {
        out.push('.');
    }
    out
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
    // Scrub telemetry: a bubble's true cost, attributed rather than guessed.
    mut spent: Local<f32>,
    // Real time: a bubble is for the player's eyes, and must stay
    // readable however hard the world is hasted.
    time: Res<Time<Real>>,
    mut messages: MessageReader<Say>,
    attention: Option<Res<crate::attention::Attention>>,
    names: Query<&crate::villager::Person>,
    // The FLAT transform: `regard` speaks sim coordinates and bends them
    // itself. (`float_bubbles` keeps the bent GlobalTransform - screen
    // projection is render-space work.)
    speakers: Query<&Transform, Without<Bubble>>,
    live: Query<&Bubble>,
) {
    let started = std::time::Instant::now();
    let mut spawned = 0u32;
    for say in messages.read() {
        // Every Say sent is meant for the screen: the senders own the
        // question of WHETHER a moment speaks (and gate it on being
        // watched); this system only owns the bubble. The composed-only
        // rule that used to live here belonged to the retired teller's
        // judging days, and its last effect was silencing written lines
        // that had every right to play.
        if live.iter().count() >= BUBBLE_CAP || live.iter().any(|b| b.speaker == say.speaker) {
            continue;
        }
        // The one place every line in the game passes through, whoever wrote
        // it. A bubble over someone off the frame is built and hidden in the
        // same breath, and one over a speck across the valley is a full-sized
        // box of text with nothing under it to belong to. Neither is worth a
        // slot out of the seven, and neither is worth the words.
        let seen = speakers.get(say.speaker).is_ok_and(|at| {
            crate::attention::regard(attention.as_deref(), at.translation).worth_saying()
        });
        if !seen {
            continue;
        }
        // Speech wears gold, thoughts wear blue, prayers wear pink — kind
        // readable at a glance before a word is read. All three inks at
        // the same strength: speech used to borrow the panel chrome's
        // whisper-gold (0.35 alpha against the others' 0.95) and its 2px
        // border read thinner than the thought bubble's despite being the
        // same width. Brett: "the chat borders feel thiner for some
        // reason." The reason was paint, not pixels.
        let border = if say.prayer {
            palette::shade(&palette::CLOTH_PINK, 1.0).with_alpha(0.95)
        } else if say.thought {
            palette::shade(&palette::CLOTH_BLUE, 0.75).with_alpha(0.95)
        } else {
            palette::shade(&palette::CLOTH_GOLD, 0.85).with_alpha(0.95)
        };
        let bubble = commands
            .spawn((
                Bubble {
                    speaker: say.speaker,
                    until: time.elapsed_secs() + 4.5,
                    lift: if say.thought { 26.0 } else { 8.0 },
                },
                // The stand-down covers bubbles with the rest of the HUD:
                // a word said under an open book plays to the frost.
                GameHud,
                // Under all interface chrome: a window dragged over a bubble
                // must cover it. Ordo's own rung for world-anchored things.
                ordo::Layer::World,
                UiTransform::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(-1000),
                    top: px(-1000),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(1),
                    padding: UiRect::axes(px(8), px(4)),
                    border: UiRect::all(px(2)),
                    // A thought's box rounds a little softer than speech.
                    border_radius: BorderRadius::all(if say.thought { px(12) } else { px(8) }),
                    max_width: px(230),
                    ..default()
                },
                BackgroundColor(theme::panel_bg()),
                BorderColor::all(border),
            ))
            .id();
        if say.thought {
            // The classic trail: two shrinking discs stepping down toward
            // the head — the comic-strip mark for "inner", one glance, four
            // nodes where the old cloud spent ninety.
            for (drop, size) in [(7.0_f32, 9.0_f32), (17.0, 6.0)] {
                commands.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        bottom: px(-drop - size),
                        width: px(size),
                        height: px(size),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(percent(50)),
                        ..default()
                    },
                    UiTransform {
                        translation: Val2::new(percent(-50), px(0)),
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
                        right: px(2),
                        bottom: px(2),
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
            Text::new(sentence(&say.text)),
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
        spawned += 1;
    }
    if spawned > 0 {
        *spent = started.elapsed().as_secs_f32() * 1000.0;
        if *spent > 2.0 {
            info!(
                "scrub: speak spawned {spawned} bubble(s) in {:.1}ms",
                *spent
            );
        }
    }
}

/// Bubbles ride above their speakers' heads and fade off with them.
fn float_bubbles(
    mut commands: Commands,
    time: Res<Time<Real>>,
    cameras: Query<(&bevy::camera::Camera, &GlobalTransform)>,
    speakers: Query<&GlobalTransform, Without<Bubble>>,
    mut bubbles: Query<(
        Entity,
        &Bubble,
        &mut Node,
        &ComputedNode,
        &mut Visibility,
        &mut UiTransform,
    )>,
) {
    let Some((camera, camera_at)) = cameras
        .iter()
        .find(|(camera, _)| camera.order == 0 && camera.is_active)
    else {
        return;
    };
    // Bubbles already settled this frame, so later ones can stack clear.
    let mut placed: Vec<Rect> = Vec::new();
    for (entity, bubble, mut node, computed, mut visibility, mut ui) in &mut bubbles {
        if time.elapsed_secs() > bubble.until {
            commands.entity(entity).despawn();
            continue;
        }
        let Ok(speaker) = speakers.get(bubble.speaker) else {
            commands.entity(entity).despawn();
            continue;
        };
        let overhead = speaker.translation() + Vec3::Y * 2.3;
        // Ordo's one depth curve, borrowed until the bubbles become
        // placards themselves: far talk shrinks to presence instead of
        // stacking a wall of full-sized text over a busy square.
        let scale = ordo::depth_scale(overhead.distance(camera_at.translation()), 55.0);
        if ui.scale != Vec2::splat(scale) {
            ui.scale = Vec2::splat(scale);
        }
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
/// the moments a village remembers — a founding, a naming; prayers wear the
/// pink border the whole prayer channel wears, corner of the eye included.
#[derive(Message)]
pub struct Notice {
    pub text: String,
    pub fanfare: bool,
    pub prayer: bool,
}

impl Notice {
    pub fn new(text: impl Into<String>) -> Self {
        Notice {
            text: text.into(),
            fanfare: false,
            prayer: false,
        }
    }

    pub fn fanfare(text: impl Into<String>) -> Self {
        Notice {
            text: text.into(),
            fanfare: true,
            prayer: false,
        }
    }

    pub fn prayer(text: impl Into<String>) -> Self {
        Notice {
            text: text.into(),
            fanfare: false,
            prayer: true,
        }
    }
}

/// The bottom-right column toasts stack into, newest at the bottom.
#[derive(Component)]
pub(crate) struct ToastShelf;

/// One visible notice, counting down to its exit.
#[derive(Component)]
struct Toast {
    remaining: f32,
    /// Border alpha when fully present, so the fade knows where it started.
    border_alpha: f32,
    fanfare: bool,
    prayer: bool,
}

/// How many notices may be on screen before the oldest is pushed out.
const TOAST_CAP: usize = 6;

/// The prayer tracker: every open asking, pinned to the middle of the
/// left edge like a quest log. Brett: "Prayers are in the codex, but how
/// about we pin them to the middle of the left side of the screen in
/// cards like a quest tracker?" Cards wear the prayer bubble's own dress
/// — 2px pink, softly rounded — and pressing one flies to the asker, the
/// same press the codex board answers to.
#[derive(Component)]
pub(crate) struct PrayerShelf;

/// Askings the god has set aside: right-pressed off the shelf. The prayer
/// itself runs on — the codex board still shows it, hope still fades, the
/// receipt still lands — the shelf just stops holding it up. Pruned as
/// prayers close, so a soul who asks again gets a fresh card.
#[derive(Resource, Default)]
struct SetAside(bevy::platform::collections::HashSet<Entity>);

/// The action button on a card sets it aside. Brett: "maybe we can make
/// right click to dismiss and ignore the card?" Honours the mouse scheme,
/// so swapped buttons swap here too.
fn set_askings_aside(
    buttons: Res<ButtonInput<MouseButton>>,
    mouse: Res<crate::keymap::MouseScheme>,
    mut aside: ResMut<SetAside>,
    shelf: Query<Entity, With<PrayerShelf>>,
    cards: Query<(&Interaction, &crate::debug::village::PrayerRow, &ChildOf)>,
) {
    if !buttons.just_pressed(mouse.action()) {
        return;
    }
    let Ok(shelf) = shelf.single() else {
        return;
    };
    for (interaction, row, parent) in &cards {
        // Shelf cards only: the codex board's rows keep their one gesture.
        if parent.parent() != shelf || *interaction == Interaction::None {
            continue;
        }
        aside.0.insert(row.0);
    }
}

/// How many cards the shelf holds before it sums the rest in one line.
const SHELF_CARDS: usize = 4;

fn spawn_prayer_shelf(mut commands: Commands) {
    let shelf = commands
        .spawn((
            Name::new("The prayers, pinned"),
            PrayerShelf,
            GameHud,
            // Ordo's furniture: an edge-docked card stack, centred halfway
            // up the left wall. The kit owns WHERE a shelf stands and how
            // it stacks; the cards stay the game's.
            ordo::shelf(ordo::Anchor::Left),
        ))
        .id();
    commands
        .entity(shelf)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.width = px(238);
            node.row_gap = px(8);
            // Empty most of the time: no layout until there is an asking.
            node.display = Display::None;
        });
}

/// Keeps the pinned cards matching the open prayers — most urgent first,
/// rebuilt only when the askings actually change (the board's own
/// fingerprint idiom; a tracker that flinches every frame is noise).
#[allow(clippy::type_complexity)]
fn keep_the_prayer_shelf(
    mut commands: Commands,
    time: Res<Time>,
    mut last_look: Local<f32>,
    mut fingerprint: Local<u64>,
    mut aside: ResMut<SetAside>,
    portraits: Res<crate::debug::portrait::Portraits>,
    shelf: Query<Entity, With<PrayerShelf>>,
    praying: Query<
        (
            Entity,
            &crate::villager::Person,
            &crate::villager::belief::Prayer,
            Option<&crate::villager::work::Vocation>,
        ),
        (
            With<crate::villager::Villager>,
            Without<crate::creature::Corpse>,
        ),
    >,
) {
    *last_look += time.delta_secs();
    if *last_look < 0.5 {
        return;
    }
    *last_look = 0.0;
    let Ok(shelf) = shelf.single() else {
        return;
    };

    // Closed prayers leave the set-aside list, so a fresh asking from the
    // same soul earns a fresh card.
    aside
        .0
        .retain(|who| praying.iter().any(|(open, ..)| open == *who));

    let mut open: Vec<_> = praying
        .iter()
        .filter(|(who, ..)| !aside.0.contains(who))
        .collect();
    open.sort_by(|a, b| {
        a.2.remaining
            .partial_cmp(&b.2.remaining)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let asking = praying.iter().count();

    // Askings for the same thing stand together on one card, fronted by
    // the most urgent asker - a starving town is one asking with many
    // voices, not a wall of cards. Brett, at nine food cards: "we should
    // probably clump multiple people praying for the same thing into one
    // prayer card to prevent getting swamped." Dark and road prayers
    // never clump: each names a particular neighbour or a particular
    // flag, and folding those together would hide who wants what.
    use crate::villager::belief::PrayerKind;
    #[derive(PartialEq)]
    enum Clump {
        Food,
        Thanks,
        Question,
        Alone(Entity),
    }
    let clump_of = |who: Entity, kind: &PrayerKind| match kind {
        PrayerKind::Food => Clump::Food,
        PrayerKind::Devotion { grateful: true } => Clump::Thanks,
        PrayerKind::Devotion { grateful: false } => Clump::Question,
        _ => Clump::Alone(who),
    };
    // (front card, voices behind it) in urgency order: the first asking
    // of each kind keeps the card, the rest are counted onto it.
    let mut clumped: Vec<(
        (
            Entity,
            &crate::villager::Person,
            &crate::villager::belief::Prayer,
            Option<&crate::villager::work::Vocation>,
        ),
        usize,
    )> = Vec::new();
    let mut keys: Vec<Clump> = Vec::new();
    for row in &open {
        let key = clump_of(row.0, &row.2.kind);
        match keys.iter().position(|k| *k == key) {
            Some(i) => clumped[i].1 += 1,
            None => {
                keys.push(key);
                clumped.push((*row, 0));
            }
        }
    }

    let fresh = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        for ((who, _, prayer, _), voices) in clumped.iter().take(SHELF_CARDS) {
            who.to_bits().hash(&mut hasher);
            voices.hash(&mut hasher);
            crate::debug::village::hope_band(prayer.remaining).hash(&mut hasher);
        }
        asking.hash(&mut hasher);
        clumped.len().hash(&mut hasher);
        hasher.finish()
    };
    if fresh == *fingerprint {
        return;
    }
    *fingerprint = fresh;

    commands.entity(shelf).despawn_related::<Children>();
    // Display follows content: no askings, no layout.
    let showing = !open.is_empty();
    commands
        .entity(shelf)
        .entry::<Node>()
        .and_modify(move |mut node| {
            node.display = if showing {
                Display::Flex
            } else {
                Display::None
            };
        });

    let pink = crate::palette::shade(&crate::palette::CLOTH_PINK, 1.0);
    for ((who, person, prayer, vocation), voices) in clumped.iter().take(SHELF_CARDS) {
        let card = commands
            .spawn((
                crate::debug::village::PrayerRow(*who),
                Interaction::default(),
                HoverHint::new(&person.name, "press to fly - action-press to set aside"),
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    // Room to breathe: the words used to press against the
                    // border. Brett: "prayer cards need some padding to
                    // make them look more polished."
                    row_gap: px(6),
                    // One number, both axes: the padding is the page
                    // grid's own rhythm.
                    padding: UiRect::all(px(crate::debug::village::RHYTHM)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(theme::panel_bg()),
                BorderColor::all(pink.with_alpha(0.95)),
                ChildOf(shelf),
            ))
            .id();
        // The asker's own face beside their ask — the studio's true
        // portrait, framed in the trade's colour (or the prayer's pink
        // for souls without a calling yet).
        let livery = vocation
            .map(|trade| crate::villager::attire::livery(*trade).cloth)
            .map(|tone| crate::palette::color_at(tone.palette_index()))
            .unwrap_or(pink);
        // Ruled off beneath the face and the name, so the header reads
        // apart from the words. Brett: "can we get a divider line under
        // the portrait and name on the card?"
        let head = commands
            .spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    padding: UiRect::bottom(px(8)),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(pink.with_alpha(0.3)),
                ChildOf(card),
            ))
            .id();
        let face = commands
            .spawn((
                Node {
                    width: px(28),
                    height: px(28),
                    flex_shrink: 0.0,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(livery.with_alpha(0.16)),
                BorderColor::all(livery.with_alpha(0.9)),
                ChildOf(head),
            ))
            .id();
        crate::debug::portrait::set_the_face(
            &mut commands,
            face,
            &portraits,
            *who,
            livery.with_alpha(0.9),
        );
        let line = if *voices > 0 {
            prayer.kind.ask_line_many(&person.name, *voices)
        } else {
            prayer.kind.ask_line(&person.name)
        };
        commands.spawn((
            body(line),
            Node {
                flex_grow: 1.0,
                min_width: px(0),
                ..default()
            },
            ChildOf(head),
        ));
        if let Some(words) = &prayer.words {
            let quoted = commands
                .spawn((dim(format!("\u{201c}{words}\u{201d}")), ChildOf(card)))
                .id();
            commands
                .entity(quoted)
                .insert(TextColor(pink.with_alpha(0.75)));
        }
        commands.spawn((
            dim(crate::debug::village::hope_band(prayer.remaining)),
            ChildOf(card),
        ));
    }
    // Whoever the shown cards do not speak for - the clumps past the
    // shelf's edge, and the askings set aside.
    let covered: usize = clumped
        .iter()
        .take(SHELF_CARDS)
        .map(|(_, voices)| 1 + voices)
        .sum();
    if asking > covered {
        commands.spawn((
            dim(format!("and {} more asking", asking - covered)),
            ChildOf(shelf),
        ));
    }
}

fn spawn_toast_shelf(mut commands: Commands) {
    commands.spawn((
        Name::new("Notices"),
        ToastShelf,
        GameHud,
        // The same Ordo furniture the prayer shelf stands on, in its
        // bottom-right setting.
        ordo::shelf(ordo::Anchor::BottomRight),
    ));
    // And centre stage, for the days that earn a trumpet.
    commands.spawn((
        Name::new("The proclamation stage"),
        GameHud,
        ordo::proclamation_stage(),
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

        let border = if notice.prayer {
            // The prayer channel's pink, the same the bubble wears.
            palette::shade(&palette::CLOTH_PINK, 1.0).with_alpha(0.85)
        } else if notice.fanfare {
            theme::accent().with_alpha(0.75)
        } else {
            theme::panel_border()
        };
        let toast = commands
            .spawn((
                Toast {
                    remaining: if notice.fanfare || notice.prayer {
                        9.0
                    } else {
                        6.0
                    },
                    border_alpha: if notice.fanfare || notice.prayer {
                        0.85
                    } else {
                        0.35
                    },
                    fanfare: notice.fanfare,
                    prayer: notice.prayer,
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
        let border_color = if toast.prayer {
            palette::shade(&palette::CLOTH_PINK, 1.0)
        } else if toast.fanfare {
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

/// The game hands Ordo its own pigment.
///
/// Ordo names roles and never ships colours, so the interface stays dyed from
/// the very ramps the villagers' clothes are dyed from — which is the whole
/// reason a kit that shipped its own palette would be no use here. Moved out
/// of the trial the day Ordo stopped being a trial.
fn lend_ramps(mut ramps: ResMut<ordo::Ramps>) {
    ramps.register("cloth_gold", |t| {
        crate::palette::shade(&crate::palette::CLOTH_GOLD, t)
    });
    ramps.register("bone", |t| crate::palette::shade(&crate::palette::BONE, t));
    ramps.register("cloth_gold_smooth", |t| {
        crate::palette::shade_smooth(&crate::palette::CLOTH_GOLD, t)
    });
    ramps.register("bone_smooth", |t| {
        crate::palette::shade_smooth(&crate::palette::BONE, t)
    });
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
        // Deep charcoal with a whisper of night blue, near opaque: the
        // world must not bleed through and mud the reading surface -
        // gold belongs to the line-work, not the air.
        Color::srgb(0.045, 0.05, 0.062).with_alpha(0.985)
    }

    /// Title-bar fill: a shade lighter than the panel, so the chrome reads
    /// as a distinct part the way real windows do.
    pub fn title_bg() -> Color {
        Color::srgb(0.075, 0.082, 0.102).with_alpha(0.99)
    }

    /// The warm card: a parchment-dark fill for detail panes, so content
    /// areas read as two materials - dark wells beside warm boards - the
    /// way a built interface does, without leaving the palette.
    pub fn card_bg() -> Color {
        Color::srgb(0.058, 0.062, 0.078)
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

/// Marks a root of the in-game HUD — the toolbar, the time controls, the
/// belief meter, the toast shelf. The title screen hides these: its scrim is
/// translucent now, and the game's furniture showing through the front door
/// reads as a bug, not a view.
#[derive(Component)]
pub struct GameHud;

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
    /// The title's text, so a many-paged window can retitle itself.
    #[allow(dead_code)]
    pub title_text: Entity,
    /// The subtitle's text, when the window was given one.
    #[allow(dead_code)]
    pub subtitle_text: Option<Entity>,
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
    window_impl_titled(commands, title, None, min_width, centred)
}

fn window_impl_titled(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
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
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.045, 0.04)),
            BorderColor::all(Color::BLACK.with_alpha(0.8)),
            BoxShadow::new(Color::BLACK.with_alpha(0.65), px(4), px(9), px(2), px(26)),
            Interaction::default(),
            // A window is solid: clicks on its body must never fall
            // through to the toolbar or the world behind it.
            bevy::ui::FocusPolicy::Block,
            ChildOf(strip),
        ))
        .id();
    let frame = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(0)),
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
    let title_words = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(1),
                ..default()
            },
            ChildOf(title_bar),
        ))
        .id();
    let title_text = commands
        .spawn((
            Text::new(title),
            DisplayFace,
            TextFont {
                font_size: FontSize::Px(20.0),
                ..default()
            },
            TextColor(theme::accent()),
            ChildOf(title_words),
        ))
        .id();
    let subtitle_text =
        subtitle.map(|subtitle| commands.spawn((dim(subtitle), ChildOf(title_words))).id());
    let close = commands
        .spawn((
            CloseButton(root),
            UiButton,
            // A bare saltire, as mocked: no box, no border - the mark
            // alone is the button.
            Node {
                width: px(26),
                height: px(26),
                ..default()
            },
            Interaction::default(),
            ChildOf(title_bar),
        ))
        .id();
    // A drawn saltire, not a typeset letter: two crossed bars in the
    // same hand-set vocabulary as every other glyph in the interface.
    for turn in [45.0, -45.0] {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(5),
                top: px(12),
                width: px(16),
                height: px(2),
                ..default()
            },
            UiTransform::from_rotation(Rot2::degrees(turn)),
            BackgroundColor(theme::accent().with_alpha(0.9)),
            ChildOf(close),
        ));
    }

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
        title_text,
        subtitle_text,
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
    just_shown: Query<(&Visibility, &ChildOf), (With<UiWindow>, Changed<Visibility>)>,
    strips: Query<Option<&GlobalZIndex>>,
    mut stack: Local<i32>,
) {
    // A window that just opened comes to the front unasked - opening IS
    // choosing it.
    for (visibility, strip) in &just_shown {
        if *visibility != Visibility::Hidden {
            *stack += 1;
            if *stack > 200 {
                *stack = 1;
            }
            commands
                .entity(strip.parent())
                .insert(GlobalZIndex(10 + *stack));
        }
    }
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

/// The panes of a split view: the window chrome, the list on the left, and
/// the detail pane on the right. Any window that pairs a roster with a
// ---------------------------------------------------------------------------
// The codex vocabulary: the pieces the People window proved, lifted into the
// kit so every new page is assembled, not re-invented.
// ---------------------------------------------------------------------------

/// A titled, subtitled window shell with nothing inside, for callers
/// building their own bands. `centred` opens it dead centre of the screen.
#[allow(dead_code)] // The codex outgrew it; other panels may yet want one.
pub fn titled_window(
    commands: &mut Commands,
    title: &str,
    subtitle: Option<&str>,
    min_width: f32,
    centred: bool,
) -> WindowHandles {
    window_impl_titled(commands, title, subtitle, min_width, centred)
}

/// A row splitting into an inset list rail and a framed detail pane — the
/// People window's anatomy as a kit piece, for pages that carry their own
/// bands above and below the split. Returns (list, detail).
#[allow(dead_code)] // Retired by the page grid; kept for panels outside the book.
pub fn split_row(
    commands: &mut Commands,
    parent: Entity,
    list_width: f32,
    gap: f32,
) -> (Entity, Entity) {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                column_gap: px(gap),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    let list = commands
        .spawn((
            Node {
                width: px(list_width),
                flex_shrink: 0.0,
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                overflow: Overflow::scroll_y(),
                padding: UiRect::all(px(6)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            // The inset well, a step darker than the panel.
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
                min_width: px(0),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme::GAP),
                padding: px(theme::PAD).into(),
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(0)),
                // The frame never scrolls as a whole; inner wells scroll.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme::card_bg()),
            BorderColor::all(theme::card_border()),
            Interaction::default(),
            ChildOf(row),
        ))
        .id();
    (list, detail)
}

/// A big-number stat plate: the label as a centred header across the top,
/// the glyph's ring medallion and the figure side by side beneath it.
/// Returns (seat, number) — the caller puts its glyph in the seat and marks
/// the number for live updates.
pub fn stat_plate(commands: &mut Commands, parent: Entity, label_text: &str) -> (Entity, Entity) {
    let plate = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(10), px(9)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                ..default()
            },
            BackgroundColor(theme::title_bg()),
            BorderColor::all(theme::panel_border()),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(label_text.to_uppercase()),
        DisplayFace,
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(theme::text_dim()),
        ChildOf(plate),
    ));
    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(12),
                ..default()
            },
            ChildOf(plate),
        ))
        .id();
    let seat = commands
        .spawn((
            Node {
                width: px(38),
                height: px(38),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(38)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.25)),
            BorderColor::all(theme::accent().with_alpha(0.4)),
            ChildOf(row),
        ))
        .id();
    let number = commands
        .spawn((
            Text::new("0"),
            DisplayFace,
            TextFont {
                font_size: FontSize::Px(32.0),
                ..default()
            },
            TextColor(theme::accent()),
            // One pixel of settlement, measured against the seat's own
            // centre: Cinzel's line box and the flex centring land the
            // digits a hair high without it.
            Node {
                margin: UiRect::top(px(3)).with_bottom(px(-3)),
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    (seat, number)
}

/// A titled card well — the People footer's card face as a kit piece: a
/// quiet dark plate with a small engraved title and content below. Returns
/// the card; children go straight in under the title.
pub fn card_well(commands: &mut Commands, parent: Entity, title: &str) -> Entity {
    let card = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_height: px(0),
                min_width: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                // Every well in the book breathes on the page grid's own
                // rhythm, the same in both directions.
                padding: UiRect::all(px(crate::debug::village::RHYTHM)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.22)),
            BorderColor::all(theme::text_dim().with_alpha(0.18)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(title),
        DisplayFace,
        TextFont {
            font_size: FontSize::Px(theme::SMALL_SIZE),
            ..default()
        },
        TextColor(theme::accent().with_alpha(0.9)),
        ChildOf(card),
    ));
    card
}

/// A ruled label/value line — the ledger's activity rows. Returns the value
/// text entity for live updates.
pub fn ruled_row(commands: &mut Commands, parent: Entity, label_text: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(px(4)),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::text_dim().with_alpha(0.12)),
            ChildOf(parent),
        ))
        .id();
    commands.spawn((label(label_text), ChildOf(row)));
    commands.spawn((body(""), ChildOf(row))).id()
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

/// Keeps every hidden window out of the LAYOUT tree, not just off the screen.
///
/// The scrub's biggest single find: a `Visibility::Hidden` tree still gets
/// laid out, so every closed panel was a tax on every UI change anywhere —
/// spawning one thought bubble re-laid-out the whole shut codex. Syncing
/// `Display::None` onto hidden windows prunes them from the computation
/// entirely; visibility remains the single source of truth.
pub fn prune_hidden_windows(
    mut windows: Query<(&Visibility, &mut Node), (With<Panel>, Changed<Visibility>)>,
) {
    for (visibility, mut node) in &mut windows {
        let display = if *visibility == Visibility::Hidden {
            Display::None
        } else {
            Display::Flex
        };
        if node.display != display {
            node.display = display;
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
        // Declared unwrappable: the serif face's +2px dressing once pushed
        // "belief in you" past the label column and the whole row folded.
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
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
                    padding: UiRect::axes(px(22), px(8)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(0)),
                    ..default()
                },
                BackgroundColor(if index == 0 {
                    theme::title_bg()
                } else {
                    Color::BLACK.with_alpha(0.18)
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
            DisplayFace,
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
    mut fills: Query<&mut BackgroundColor, With<TabButton>>,
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
                    theme::accent().with_alpha(0.85)
                } else {
                    theme::panel_border().with_alpha(0.35)
                });
            }
            if let Ok(mut fill) = fills.get_mut(other) {
                fill.0 = if active {
                    theme::title_bg().into()
                } else {
                    Color::BLACK.with_alpha(0.18)
                };
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
        DisplayFace,
        TextFont {
            font_size: FontSize::Px(theme::TITLE_SIZE),
            ..default()
        },
        TextColor(theme::accent()),
    )
}

/// A display-face headline at a chosen size - the big names.
pub fn title_sized(text: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(text),
        DisplayFace,
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(theme::accent()),
    )
}

/// Marks text that speaks in the display face - Cinzel, the engraved
/// capitals - rather than the working mono. Titles and names, never data.
#[derive(Component)]
pub struct DisplayFace;

/// The flourished bold display face — Cinzel Decorative.
#[derive(Component)]
pub struct DisplayBoldFace;

/// Marks text that speaks in the reading face - EB Garamond, the warm
/// serif - for labels, values and running words on the panels.
#[derive(Component)]
pub struct SerifFace;

/// The game's loaded typefaces.
#[derive(Resource)]
pub struct Fonts {
    pub display: Handle<Font>,
    /// Cinzel Decorative Bold: the flourished capitals, for the few places
    /// that carry a heading alone — the date plaque wears it.
    pub display_bold: Handle<Font>,
    pub text: Handle<Font>,
}

pub(crate) fn load_fonts(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Fonts {
        display: assets.load("fonts/Cinzel.ttf"),
        display_bold: assets.load("fonts/CinzelDecorative-Bold.ttf"),
        text: assets.load("fonts/EBGaramond.ttf"),
    });
}

/// Dresses every newly spawned display-face text in the display font.
/// Spawn helpers build bundles through Commands and cannot reach assets;
/// this system is the tailor that fits them afterwards.
pub(crate) fn dress_display_text(
    fonts: Option<Res<Fonts>>,
    mut fresh: Query<&mut TextFont, Added<DisplayFace>>,
    mut bold: Query<&mut TextFont, (Added<DisplayBoldFace>, Without<DisplayFace>)>,
    mut prose: Query<
        &mut TextFont,
        (
            Added<SerifFace>,
            Without<DisplayFace>,
            Without<DisplayBoldFace>,
        ),
    >,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for mut font in &mut fresh {
        font.font = fonts.display.clone().into();
    }
    for mut font in &mut bold {
        font.font = fonts.display_bold.clone().into();
    }
    for mut font in &mut prose {
        font.font = fonts.text.clone().into();
        // Garamond runs small on the body: a step up keeps the panels
        // as readable as the mono they replace.
        font.font_size = FontSize::Px(match font.font_size {
            FontSize::Px(size) => size + 2.0,
            _ => 17.0,
        });
    }
}

/// Ordinary readable text.
pub fn body(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        SerifFace,
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
        SerifFace,
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
        SerifFace,
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
        SerifFace,
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
        .spawn((
            GameHud,
            Node {
                position_type: PositionType::Absolute,
                top,
                bottom,
                left: px(0),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id()
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
fn track_pointer(panels: Query<&Interaction>, mut pointer: ResMut<PointerContext>) {
    // ANY hovered interactive node counts - not just Panel-marked ones.
    // Buttons win the pick over their window, so checking panels alone
    // read "not over UI" the instant a cursor touched a button, and the
    // hand jittered between its interface pose and its world pose along
    // every button edge.
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
    fn every_bubble_reads_as_a_proper_sentence() {
        assert_eq!(sentence("my belly aches"), "My belly aches.");
        assert_eq!(sentence("was it the god?"), "Was it the god?");
        assert_eq!(sentence("I saw it, I did"), "I saw it, I did.");
        assert_eq!(sentence("so it goes..."), "So it goes...");
        assert_eq!(sentence(""), "");
    }

    #[test]
    fn theme_colors_are_opaque_enough_to_read() {
        // The panel may be translucent; the text may not.
        assert!(theme::panel_bg().alpha() < 1.0);
        assert!(theme::text().alpha() > 0.99);
        assert!(theme::accent().alpha() > 0.99);
    }
}

/// The frosted glass: while the book is open, the world behind it blurs
/// heavily — alive in motion, unreadable in detail, so nothing fights the
/// text. Brett: "can we apply a heavy blur to the game world when the menu
/// is open?" Done with the renderer's own depth of field, focus pulled to
/// the god's nose and the aperture thrown wide.
/// The pane of frosted glass the book lies on: a fullscreen picture of
/// the small live painting the frost camera makes.
#[derive(Component)]
pub(crate) struct FrostPane;

/// The little camera that paints the world onto the glass while the book
/// is open.
#[derive(Component)]
pub(crate) struct FrostCamera;

/// Marks the god camera while its own eyes are closed for reading.
#[derive(Component)]
pub(crate) struct Frosted;

/// The glass's grain: the world is painted this small and stretched over
/// the whole window, so the frost is enormous and costs almost nothing.
const FROST_FACE: (u32, u32) = (480, 270);

/// Raises the frost rig once: the glass texture, the sleeping camera that
/// paints it, and the fullscreen pane that shows it under the book.
pub(crate) fn spawn_frost_glass(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    look: Res<crate::render::LookSettings>,
) {
    let glass = images.add(bevy::image::Image::new_target_texture(
        FROST_FACE.0,
        FROST_FACE.1,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands.spawn((
        Name::new("Frost Camera"),
        FrostCamera,
        Camera3d::default(),
        Camera {
            order: -5,
            is_active: false,
            ..default()
        },
        bevy::camera::RenderTarget::Image(glass.clone().into()),
        // The glass wears the god camera's own look - HDR, the same
        // tonemapper, the depth prepass the water reads - or its noon
        // comes out as someone else's dusk. The grading is mirrored live
        // in frost_glass_follows, since F8/F9 can retune it mid-game.
        bevy::camera::Hdr,
        bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
        bevy::core_pipeline::prepass::DepthPrepass,
        bevy::render::view::ColorGrading::default(),
        // At 270 rows this small blur is a huge one on the stretched
        // glass, for a few dozen taps per tiny pixel.
        bevy::post_process::dof::DepthOfField {
            mode: bevy::post_process::dof::DepthOfFieldMode::Gaussian,
            focal_distance: 0.05,
            sensor_height: 0.01866,
            aperture_f_stops: 0.008,
            max_circle_of_confusion_diameter: look.frost,
            max_depth: f32::INFINITY,
        },
        Transform::default(),
        bevy::camera::visibility::RenderLayers::from_layers(&[0, crate::globe::GLOBE_LAYER]),
    ));
    commands.spawn((
        FrostPane,
        Name::new("Frost Pane"),
        bevy::ui::widget::ImageNode::new(glass).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            top: px(0),
            width: percent(100),
            height: percent(100),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(-2),
    ));
}

/// While the book is open the world goes behind frosted glass — and not
/// by squinting: the god camera's own eyes close (its layers strip, so
/// the window pass carries only the book), the frost camera paints the
/// live world onto a palm-sized glass, and the pane stretches that glass
/// across the window. Painted small, blurred wide, stretched huge: the
/// world is colour and motion and nothing legible, at a fraction of the
/// old full-resolution blur's price. Brett: "I want it super blurry" —
/// "maybe we need to use another method?" This is that method.
pub(crate) fn frost_the_world(
    mut commands: Commands,
    books: Query<&Visibility, With<crate::debug::village::VillagePanel>>,
    gods: Query<(Entity, Has<Frosted>), With<crate::camera::GodCamera>>,
    mut frost: Query<&mut Camera, With<FrostCamera>>,
    mut panes: Query<&mut Node, With<FrostPane>>,
) {
    // DIVUS_FACTUS_FROST forces the glass on from boot, so the frost can
    // be photographed and judged without a hand to open the book.
    //
    // The god camera keeps rendering the world UNDER the pane while the
    // book is open. Its layers must never be stripped: the streamer and
    // every other camera-keyed governor follow what the window's camera
    // sees, and blinding it unloaded the world out from under the frost
    // camera too — the glass showed nothing but clouds over the void.
    let reading = books.iter().any(|v| *v != Visibility::Hidden)
        || std::env::var("DIVUS_FACTUS_FROST").is_ok();
    for (camera, frosted) in &gods {
        if reading && !frosted {
            commands.entity(camera).insert(Frosted);
            for mut lens in &mut frost {
                lens.is_active = true;
            }
            for mut pane in &mut panes {
                pane.display = Display::Flex;
            }
        } else if !reading && frosted {
            commands.entity(camera).remove::<Frosted>();
            for mut lens in &mut frost {
                lens.is_active = false;
            }
            for mut pane in &mut panes {
                pane.display = Display::None;
            }
        }
    }
}

/// The frost camera RIDES the god camera: adopted as its child with an
/// identity transform, so propagation itself guarantees the two share an
/// eye every frame. A hand-rolled position sync froze mid-flight once —
/// the glass stared at the sea floor from under the world while the god
/// flew on — and a child cannot be left behind. The lens is still copied
/// by hand: projections do not inherit.
pub(crate) fn frost_glass_follows(
    mut commands: Commands,
    gods: Query<
        (
            Entity,
            &Projection,
            Option<&bevy::render::view::ColorGrading>,
        ),
        (With<crate::camera::GodCamera>, Without<FrostCamera>),
    >,
    mut frost: Query<
        (
            Entity,
            &mut Projection,
            Option<&mut bevy::render::view::ColorGrading>,
            Has<ChildOf>,
        ),
        (With<FrostCamera>, Without<crate::camera::GodCamera>),
    >,
) {
    let Ok((god, lens, grading)) = gods.single() else {
        return;
    };
    for (glass, mut glass_lens, glass_grading, adopted) in &mut frost {
        if !adopted {
            commands
                .entity(glass)
                .insert((ChildOf(god), Transform::IDENTITY));
        }
        *glass_lens = lens.clone();
        // The god's grading carries the look's exposure and saturation;
        // the glass wears the same or reads darker than the world it
        // reflects.
        if let (Some(grading), Some(mut glass_grading)) = (grading, glass_grading) {
            *glass_grading = grading.clone();
        }
    }
}

/// The shelves stand down while the book is open: toasts and prayer cards
/// live on the toast layer, ABOVE the book, and a tray ghosting over the
/// page is clutter. They return, queue intact, when the book closes.
#[allow(clippy::type_complexity)]
pub(crate) fn hud_stands_down(
    books: Query<&Visibility, With<crate::debug::village::VillagePanel>>,
    mut hud: Query<&mut Node, With<GameHud>>,
) {
    // The whole HUD stands down while the book is open: shelves, hotbar,
    // belief ladder, speed apron, date card — the book owns the screen,
    // and its footer carries what a reader still needs. Brett: "maybe all
    // in game UI should hide when the codex opens?"
    let reading = books.iter().any(|v| *v != Visibility::Hidden);
    for mut node in &mut hud {
        if reading {
            if node.display != Display::None {
                node.display = Display::None;
            }
        } else if node.display != Display::Flex {
            // Owners with their own display rules (the prayer shelf
            // empties itself) correct this on their next pass; a
            // childless Flex node draws nothing in the meantime.
            node.display = Display::Flex;
        }
    }
}
