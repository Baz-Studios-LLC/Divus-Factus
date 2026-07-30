//! Unattended capture: the screenshot harness soaks verify with.

use crate::terrain::LoadedChunks;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use super::*;

/// Pending automatic screenshot, if one was requested by environment variable.
#[derive(Resource)]
pub(crate) struct AutoCapture {
    pub(crate) path: String,
    pub(crate) delay: f32,
    pub(crate) taken: bool,
}

pub(crate) fn auto_capture(
    mut commands: Commands,
    // Real time: the capture delay is a wall-clock promise, and must not
    // stretch on the title screen's pause or shrink under EGREGORE_SPEED.
    time: Res<Time<Real>>,
    mut capture: ResMut<AutoCapture>,
    target: Option<Res<crate::render::CaptureTarget>>,
    chunk_probe: Option<Res<LoadedChunks>>,
    entity_probe: Query<Entity>,
    fps_probe: Res<DiagnosticsStore>,
    rig_probe: Query<&crate::camera::CameraRig>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture.taken {
        // Give the save a moment to land before tearing the app down.
        if time.elapsed_secs() > capture.delay + 1.5 {
            exit.write(AppExit::Success);
        }
        return;
    }

    if time.elapsed_secs() >= capture.delay {
        capture.taken = true;
        let rig = rig_probe.single().ok();
        info!(
            "PERF chunks={} entities={} fps={:.0} dist={:.0} focus=({:.0},{:.0})",
            chunk_probe.map_or(0, |c| c.count()),
            entity_probe.iter().count(),
            fps_probe
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0),
            rig.map_or(0.0, |r| r.distance),
            rig.map_or(0.0, |r| r.focus.x),
            rig.map_or(0.0, |r| r.focus.z),
        );
        let path = capture.path.clone();

        // Prefer the offscreen target set up by the render pipeline; fall back to
        // the window if capture mode did not manage to create one.
        let screenshot = match &target {
            Some(target) => Screenshot::image(target.image.clone()),
            None => Screenshot::primary_window(),
        };
        commands.spawn(screenshot).observe(save_to_disk(path));
    }
}

/// Saves a screenshot on F12.
pub(crate) fn screenshot_on_request(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut counter: Local<u32>,
) {
    if keys.just_pressed(KeyCode::F12) {
        let path = format!("egregore-{:03}.png", *counter);
        *counter += 1;
        info!("saving screenshot to {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Rebuilds the paperdoll when the selection changes: the person's actual
/// genome, rebuilt on the stage.
/// In capture mode, someone is always selected: screenshots need a subject.
pub(crate) fn capture_preselect(
    mut selected: ResMut<SelectedPerson>,
    people: Query<
        Entity,
        (
            With<Villager>,
            With<Person>,
            Without<crate::creature::Corpse>,
        ),
    >,
    mut history: Query<&mut Visibility, With<super::HistoryPanel>>,
    mut codex_windows: Query<
        &mut Visibility,
        (With<super::VillagePanel>, Without<super::HistoryPanel>),
    >,
    codex: Option<ResMut<super::village::Codex>>,
) {
    if crate::capture_path().is_none() || crate::title::title_capture() {
        return;
    }
    // EGREGORE_OPEN=history photographs the chronicle; =village, the codex on
    // its ledger page; anything else opens the codex on the People page with
    // somebody selected, so a screenshot always has a subject.
    if std::env::var("EGREGORE_OPEN").is_ok_and(|w| w == "history") {
        for mut visibility in &mut history {
            *visibility = Visibility::Visible;
        }
        return;
    }
    for mut visibility in &mut codex_windows {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Visible;
        }
    }
    if std::env::var("EGREGORE_OPEN").is_ok_and(|w| w == "village") {
        return;
    }
    if let Some(mut codex) = codex
        && codex.page != super::village::CodexPage::People
    {
        codex.page = super::village::CodexPage::People;
    }
    if selected.0.is_some() {
        return;
    }
    if let Some(person) = people.iter().next() {
        selected.0 = Some(person);
    }
}
