use crate::audio::types::{AudioState, RecordingMode, RecordingTrigger};
use crate::overlay::overlay::notify_overlay_error;
use crate::shortcuts::modifiers::wait_for_modifiers_released;
use crate::shortcuts::registry::ShortcutRegistryState;
use crate::shortcuts::types::{
    recording_state, ActivationMode, KeyEventType, RecordingLifecycle, RecordingOwner,
    RecordingSource, ShortcutAction, ShortcutRegistry, ShortcutState,
};
use log::{debug, info, warn};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const SHORTCUT_COOLDOWN: Duration = Duration::from_millis(250);

/// Covers what the modifier wait cannot: the binding's own letter may still be
/// held, and X11 auto-repeat would then turn the injected paste into several.
/// Also the only margin under Wayland, where the pressed keys are unknown.
const MODIFIER_RELEASE_DELAY: Duration = Duration::from_millis(50);

static PASTE_LAST_ACTIVE: AtomicBool = AtomicBool::new(false);

fn within_cooldown(last: &Mutex<Instant>) -> bool {
    last.lock().elapsed() < SHORTCUT_COOLDOWN
}

/// Verifie qu'un mode LLM est utilisable (LLM Connect active + prompt configure).
/// Retourne Ok(()) si pret, Err(()) sinon. Emet `llm-connect-disabled` si LLM
/// Connect est desactive apres l'onboarding, ou `llm-mode-not-configured` si le
/// mode existe sans prompt, uniquement si `notify` est vrai
/// (le clavier passe false sur Release pour eviter le double-fire press+release).
pub(crate) fn ensure_llm_mode_ready(app: &AppHandle, index: usize, notify: bool) -> Result<(), ()> {
    let settings = crate::llm::helpers::load_llm_connect_settings(app);
    match (settings.onboarding_completed, settings.enabled) {
        (false, _) => {
            warn!("LLM Connect disabled: llm-mode {} ignored", index + 1);
            return Err(());
        }
        (true, false) => {
            warn!("LLM Connect turned off: llm-mode {} ignored", index + 1);
            if notify {
                notify_overlay_error(app, "llm-connect-disabled");
            }
            return Err(());
        }
        (true, true) => {}
    }
    if settings
        .modes
        .get(index)
        .is_none_or(|m| m.prompt.trim().is_empty())
    {
        if notify {
            warn!(
                "LLM mode {} not configured, emitting llm-mode-not-configured",
                index + 1
            );
            let _ = app.emit(
                "llm-mode-not-configured",
                serde_json::json!({ "mode": index + 1 }),
            );
        }
        return Err(());
    }
    Ok(())
}

pub(crate) fn ensure_command_ready(app: &AppHandle, notify: bool) -> Result<(), ()> {
    let settings = crate::llm::helpers::load_llm_connect_settings(app);
    match (
        settings.onboarding_completed,
        settings.command.enabled,
        settings.command.model.trim().is_empty(),
    ) {
        (false, _, _) => {
            warn!("Command mode not configured, recording ignored");
            Err(())
        }
        (true, false, _) => {
            warn!("Command mode turned off, recording ignored");
            if notify {
                notify_overlay_error(app, "command-disabled");
            }
            Err(())
        }
        (true, true, true) => {
            warn!("Command mode has no model, recording ignored");
            if notify {
                notify_overlay_error(app, "command-not-configured");
            }
            Err(())
        }
        (true, true, false) => Ok(()),
    }
}

pub(crate) fn is_recording_idle() -> bool {
    *recording_state().source.lock() == RecordingSource::None
}

pub fn handle_shortcut_event(
    app: &AppHandle,
    action: &ShortcutAction,
    mode: &ActivationMode,
    event_type: KeyEventType,
) {
    let shortcut_state = app.state::<ShortcutState>();

    match action {
        ShortcutAction::StartRecording => {
            let app_for_fn = app.clone();
            handle_recording_event(
                app,
                RecordingSource::Standard,
                mode,
                event_type,
                &shortcut_state,
                RecordingOwner::Other,
                move || crate::audio::record_audio(&app_for_fn, RecordingMode::Standard),
            );
        }
        ShortcutAction::StartRecordingCommand => {
            if is_recording_idle()
                && ensure_command_ready(app, event_type == KeyEventType::Pressed).is_err()
            {
                return;
            }
            let app_for_fn = app.clone();
            handle_recording_event(
                app,
                RecordingSource::Command,
                mode,
                event_type,
                &shortcut_state,
                RecordingOwner::Other,
                move || crate::audio::record_audio(&app_for_fn, RecordingMode::Command),
            );
        }
        ShortcutAction::PasteLastTranscript => {
            if event_type == KeyEventType::Pressed {
                spawn_paste_last_transcript(app);
            }
        }
        ShortcutAction::StartRecordingLlmMode(index) => {
            if ensure_llm_mode_ready(app, *index, event_type == KeyEventType::Pressed).is_err() {
                return;
            }
            crate::llm::switch_active_mode_silent(app, *index);
            let app_for_fn = app.clone();
            handle_recording_event(
                app,
                RecordingSource::Llm,
                mode,
                event_type,
                &shortcut_state,
                RecordingOwner::Other,
                move || crate::audio::record_audio(&app_for_fn, RecordingMode::Llm),
            );
        }
        ShortcutAction::TransformSelectionLlmMode(index) => {
            if event_type != KeyEventType::Released {
                return;
            }
            crate::llm::spawn_transform_selection(app, *index);
        }
        ShortcutAction::CancelRecording => {
            if event_type == KeyEventType::Pressed && !is_recording_idle() {
                force_cancel_recording(app);
            }
        }
        ShortcutAction::ToggleVoiceMode => {
            if event_type == KeyEventType::Pressed {
                let mut last_switch = recording_state().last_mode_switch.lock();
                if last_switch.elapsed() <= Duration::from_millis(50) {
                    return;
                }
                *last_switch = Instant::now();
                drop(last_switch);
                let _ = app.emit("voice-mode-toggle-requested", ());
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn handle_speechmike_record_event(
    app: &AppHandle,
    mode: &ActivationMode,
    event_type: KeyEventType,
) {
    let shortcut_state = app.state::<ShortcutState>();
    if shortcut_state.is_suspended() || shortcut_state.is_capturing() {
        cancel_speechmike_recording(app);
        return;
    }
    let app_for_fn = app.clone();
    handle_recording_event(
        app,
        RecordingSource::Standard,
        mode,
        event_type,
        &shortcut_state,
        RecordingOwner::SpeechMike,
        move || crate::audio::record_audio(&app_for_fn, RecordingMode::Standard),
    );
}

/// Waits for the physical modifiers: synthetic keys merge with the real
/// keyboard state, so pasting while the binding is still held reaches the
/// target app as the whole combination. Runs off the shortcut thread because
/// on X11 that thread also drains the key events the wait relies on.
pub(crate) fn spawn_paste_last_transcript(app: &AppHandle) {
    let transcript = match crate::history::get_last_transcription(app) {
        Ok(transcript) => transcript,
        Err(e) => {
            warn!("Paste last transcript: no transcript available ({})", e);
            return;
        }
    };

    // Overlapping pastes would fight over the clipboard snapshot.
    if PASTE_LAST_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        debug!("Paste last transcript: already running, request ignored");
        return;
    }

    let app = app.clone();
    std::thread::spawn(move || {
        wait_for_modifiers_released();
        std::thread::sleep(MODIFIER_RELEASE_DELAY);
        let _ = crate::audio::write_last_transcription(&app, &transcript);
        PASTE_LAST_ACTIVE.store(false, Ordering::SeqCst);
    });
}

fn handle_recording_event<F>(
    app: &AppHandle,
    target: RecordingSource,
    mode: &ActivationMode,
    event_type: KeyEventType,
    shortcut_state: &ShortcutState,
    owner: RecordingOwner,
    start_fn: F,
) where
    F: FnOnce() + Send + 'static,
{
    match mode {
        ActivationMode::PushToTalk => {
            pushtotalk_recording_action(app, target, event_type, shortcut_state, owner, start_fn)
        }
        ActivationMode::ToggleToTalk => {
            if event_type == KeyEventType::Released {
                toggle_recording_action_for_owner(app, target, shortcut_state, owner, start_fn);
            }
        }
    }
}

fn pushtotalk_recording_action<F>(
    app: &AppHandle,
    target: RecordingSource,
    event_type: KeyEventType,
    _shortcut_state: &ShortcutState,
    owner: RecordingOwner,
    start_fn: F,
) where
    F: FnOnce() + Send + 'static,
{
    let state = recording_state();
    if state.ending.load(Ordering::SeqCst) {
        return;
    }
    let mut lifecycle = state.lifecycle.lock();
    if lifecycle.is_ending() {
        return;
    }
    reconcile_audio_owner(app, &mut lifecycle);
    let mut recording_source = state.source.lock();

    match event_type {
        KeyEventType::Pressed => {
            if *recording_source == RecordingSource::None {
                start_recording(
                    app,
                    &mut recording_source,
                    &mut lifecycle,
                    target,
                    owner,
                    start_fn,
                );
            }
        }
        KeyEventType::Released => {
            if *recording_source == target && lifecycle.can_control(owner) {
                if let Some(revision) = pre_stop(&mut recording_source, &mut lifecycle) {
                    drop(recording_source);
                    finish_stop(app, revision);
                }
            }
        }
    }
}

pub(crate) fn toggle_recording_action<F>(
    app: &AppHandle,
    target: RecordingSource,
    shortcut_state: &ShortcutState,
    start_fn: F,
) where
    F: FnOnce() + Send + 'static,
{
    toggle_recording_action_for_owner(app, target, shortcut_state, RecordingOwner::Other, start_fn);
}

fn toggle_recording_action_for_owner<F>(
    app: &AppHandle,
    target: RecordingSource,
    shortcut_state: &ShortcutState,
    owner: RecordingOwner,
    start_fn: F,
) where
    F: FnOnce() + Send + 'static,
{
    let state = recording_state();
    if state.ending.load(Ordering::SeqCst) {
        return;
    }
    let mut lifecycle = state.lifecycle.lock();
    if lifecycle.is_ending() {
        return;
    }
    reconcile_audio_owner(app, &mut lifecycle);
    let mut recording_source = state.source.lock();

    if *recording_source == target {
        if !lifecycle.can_control(owner) {
            return;
        }
        if within_cooldown(&recording_state().last_toggle_start) {
            return;
        }
        if let Some(revision) = pre_stop(&mut recording_source, &mut lifecycle) {
            shortcut_state.set_toggled(false);
            *recording_state().last_toggle_stop.lock() = Instant::now();
            drop(recording_source);
            finish_stop(app, revision);
        }
    } else if *recording_source == RecordingSource::None {
        if within_cooldown(&recording_state().last_toggle_stop) {
            info!("ToggleToTalk start ignored (cooldown after stop)");
            return;
        }
        if start_recording(
            app,
            &mut recording_source,
            &mut lifecycle,
            target,
            owner,
            start_fn,
        ) {
            shortcut_state.set_toggled(true);
        }
    }
}

fn start_recording<F>(
    app: &AppHandle,
    recording_source: &mut RecordingSource,
    lifecycle: &mut RecordingLifecycle,
    target: RecordingSource,
    owner: RecordingOwner,
    start_fn: F,
) -> bool
where
    F: FnOnce() + Send + 'static,
{
    let audio = app.state::<AudioState>();
    let Some(ticket) = lifecycle.reserve_start(
        owner,
        audio.current_session(),
        *recording_source == RecordingSource::None,
        audio.is_session_active(),
    ) else {
        return false;
    };
    crate::onboarding::onboarding::capture_focus_at_record_start(app);
    *recording_source = target;
    *recording_state().last_toggle_start.lock() = Instant::now();
    // Run off-thread so the shortcut processor stays reactive during the
    // ~100ms CPAL init and doesn't queue up auto-repeat events.
    let app = app.clone();
    std::thread::spawn(move || {
        let state = recording_state();
        // This guard also covers cancellation: a queued start cannot outlive a
        // disconnect, and cancellation waits for microphone initialization to finish.
        let mut lifecycle = state.lifecycle.lock();
        if !lifecycle.is_current(ticket) {
            return;
        }
        let audio = app.state::<AudioState>();
        if *state.source.lock() != target
            || audio.current_session() != ticket.audio_before_start
            || audio.is_session_active()
        {
            lifecycle.invalidate();
            return;
        }
        if owner == RecordingOwner::SpeechMike {
            audio.set_recording_trigger(RecordingTrigger::Keyboard);
        }
        start_fn();
        let generation = audio.current_session();
        if generation != ticket.audio_before_start.wrapping_add(1) {
            // An external audio entry point superseded this request.
            lifecycle.invalidate();
            return;
        }
        if audio.is_session_active() {
            lifecycle.started(ticket, generation);
            reconcile_audio_owner(&app, &mut lifecycle);
            info!("Started {:?} recording", target);
        } else {
            lifecycle.invalidate();
            *state.source.lock() = RecordingSource::None;
            app.state::<ShortcutState>().set_toggled(false);
            crate::audio::cancel_recording(&app);
        }
    });
    true
}

fn reconcile_audio_owner(app: &AppHandle, lifecycle: &mut RecordingLifecycle) {
    let audio = app.state::<AudioState>();
    lifecycle.reconcile_audio(
        audio.current_session(),
        audio.get_recording_trigger() == RecordingTrigger::Keyboard,
    );
}

fn pre_stop(
    recording_source: &mut RecordingSource,
    lifecycle: &mut RecordingLifecycle,
) -> Option<u64> {
    let revision = lifecycle.reserve_stop()?;
    *recording_source = RecordingSource::None;
    recording_state().ending.store(true, Ordering::SeqCst);
    Some(revision)
}

fn finish_stop(app: &AppHandle, revision: u64) {
    // Off-thread because stop_recording blocks on the LLM request and paste
    // (~1s), during which the processor must keep handling keyboard events.
    let app = app.clone();
    std::thread::spawn(move || {
        if run_reserved_stop(&app, revision) {
            info!("Stopped recording");
        }
    });
}

fn run_reserved_stop(app: &AppHandle, revision: u64) -> bool {
    let state = recording_state();
    {
        let mut lifecycle = state.lifecycle.lock();
        if !lifecycle.begin_stop(revision) {
            return false;
        }
    }
    // Finalization can take seconds. Keep admission closed, but allow cancel to
    // invalidate the audio generation and cancel pending work without waiting.
    let _ = crate::audio::stop_recording(app);
    let mut lifecycle = state.lifecycle.lock();
    lifecycle.complete_stop(revision);
    state.ending.store(lifecycle.is_ending(), Ordering::SeqCst);
    true
}

pub fn force_stop_recording(app: &AppHandle) {
    let state = recording_state();
    let revision = {
        let mut lifecycle = state.lifecycle.lock();
        let mut recording_source = state.source.lock();
        let Some(revision) = pre_stop(&mut recording_source, &mut lifecycle) else {
            return;
        };
        app.state::<ShortcutState>().set_toggled(false);
        revision
    };
    run_reserved_stop(app, revision);
}

pub fn force_cancel_recording(app: &AppHandle) {
    cancel_recording_for_owner(app, false);
}

#[cfg(target_os = "windows")]
pub(crate) fn cancel_speechmike_recording(app: &AppHandle) {
    cancel_recording_for_owner(app, true);
}

#[cfg(target_os = "windows")]
pub(crate) fn speechmike_is_recording(app: &AppHandle) -> bool {
    let state = recording_state();
    let mut lifecycle = state.lifecycle.lock();
    reconcile_audio_owner(app, &mut lifecycle);
    *state.source.lock() == RecordingSource::Standard
        && lifecycle.can_control(RecordingOwner::SpeechMike)
}

#[cfg(target_os = "windows")]
pub(crate) fn speechmike_input_available(app: &AppHandle) -> bool {
    let shortcut_state = app.state::<ShortcutState>();
    if shortcut_state.is_suspended()
        || shortcut_state.is_capturing()
        || recording_state().ending.load(Ordering::SeqCst)
    {
        return false;
    }
    if speechmike_is_recording(app) {
        return true;
    }
    let state = recording_state();
    let lifecycle = state.lifecycle.lock();
    !lifecycle.is_ending()
        && *state.source.lock() == RecordingSource::None
        && !app.state::<AudioState>().is_session_active()
}

fn cancel_recording_for_owner(app: &AppHandle, speechmike_only: bool) {
    let state = recording_state();
    let mut lifecycle = state.lifecycle.lock();
    reconcile_audio_owner(app, &mut lifecycle);
    if speechmike_only
        && (*state.source.lock() != RecordingSource::Standard
            || !lifecycle.can_control(RecordingOwner::SpeechMike))
    {
        return;
    }
    let had_audio = app.state::<AudioState>().is_session_active();
    lifecycle.invalidate();
    state.ending.store(lifecycle.is_ending(), Ordering::SeqCst);
    let shortcut_state = app.state::<ShortcutState>();
    shortcut_state.set_toggled(false);
    {
        let mut recording_source = recording_state().source.lock();
        *recording_source = RecordingSource::None;
    }
    if !speechmike_only || had_audio {
        crate::audio::cancel_recording(app);
    }
}

#[cfg(target_os = "linux")]
pub fn init_shortcuts(app: AppHandle) {
    let settings = crate::settings::load_settings(&app);
    let registry = ShortcutRegistry::from_settings(&settings);

    app.manage(ShortcutState::new());
    app.manage(ShortcutRegistryState::new(registry));

    crate::shortcuts::platform_linux::init(app);
}

#[cfg(target_os = "windows")]
pub fn init_shortcuts(app: AppHandle) {
    let settings = crate::settings::load_settings(&app);
    let registry = ShortcutRegistry::from_settings(&settings);

    app.manage(ShortcutState::new());
    app.manage(ShortcutRegistryState::new(registry));

    crate::shortcuts::platform_windows::init(app);
}

#[cfg(target_os = "macos")]
pub fn init_shortcuts(app: AppHandle) {
    let settings = crate::settings::load_settings(&app);
    let registry = ShortcutRegistry::from_settings(&settings);

    app.manage(ShortcutState::new());
    app.manage(ShortcutRegistryState::new(registry));

    crate::shortcuts::platform_macos::init(app);
}

#[cfg(test)]
mod tests {
    use super::{within_cooldown, SHORTCUT_COOLDOWN};
    use parking_lot::Mutex;
    use std::time::{Duration, Instant};

    #[test]
    fn cooldown_active_for_recent_instant() {
        let recent = Mutex::new(Instant::now());
        assert!(within_cooldown(&recent));
    }

    #[test]
    fn cooldown_inactive_past_threshold() {
        let past = Mutex::new(Instant::now() - SHORTCUT_COOLDOWN - Duration::from_millis(50));
        assert!(!within_cooldown(&past));
    }

    #[test]
    fn cooldown_inactive_at_exact_threshold() {
        let at_boundary = Mutex::new(Instant::now() - SHORTCUT_COOLDOWN);
        assert!(!within_cooldown(&at_boundary));
    }

    #[test]
    fn cooldown_threshold_matches_documented_value() {
        // PTT/Toggle behaviour assumes 250 ms. Larger lets auto-repeat noise
        // through, smaller drops legit very-short taps.
        assert_eq!(SHORTCUT_COOLDOWN, Duration::from_millis(250));
    }
}
