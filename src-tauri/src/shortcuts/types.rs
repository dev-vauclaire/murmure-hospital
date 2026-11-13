use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyEventType {
    Pressed,
    Released,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShortcutAction {
    StartRecording,
    StartRecordingCommand,
    PasteLastTranscript,
    StartRecordingLlmMode(usize),
    TransformSelectionLlmMode(usize),
    CancelRecording,
    ToggleVoiceMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivationMode {
    PushToTalk,
    ToggleToTalk,
}

#[derive(Debug, Clone)]
pub struct ShortcutBinding {
    pub keys: Vec<i32>,
    pub action: ShortcutAction,
    pub activation_mode: ActivationMode,
}

// === Registry ===

#[derive(Debug, Clone)]
pub struct ShortcutRegistry {
    pub bindings: Vec<ShortcutBinding>,
}

// === States ===

pub struct ShortcutState {
    pub suspended: AtomicBool,
    pub is_toggled: AtomicBool,
    pub capture_active: AtomicBool,
    pub capture_available: AtomicBool,
    pub capture_keys: Mutex<Vec<i32>>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RecordingSource {
    None,
    Standard,
    Llm,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordingOwner {
    Other,
    SpeechMike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordingTicket {
    pub(crate) revision: u64,
    pub(crate) owner: RecordingOwner,
    pub(crate) audio_before_start: u64,
}

#[derive(Debug, Default)]
pub(crate) struct RecordingLifecycle {
    revision: u64,
    request: Option<RecordingTicket>,
    audio_generation: Option<u64>,
    ending: Option<EndingRequest>,
}

#[derive(Debug, Clone, Copy)]
struct EndingRequest {
    revision: u64,
    running: bool,
}

impl RecordingLifecycle {
    pub(crate) fn reserve_start(
        &mut self,
        owner: RecordingOwner,
        audio_generation: u64,
        source_idle: bool,
        audio_active: bool,
    ) -> Option<RecordingTicket> {
        if self.ending.is_some() || !source_idle || audio_active {
            return None;
        }
        self.revision = self.revision.wrapping_add(1);
        let ticket = RecordingTicket {
            revision: self.revision,
            owner,
            audio_before_start: audio_generation,
        };
        self.request = Some(ticket);
        self.audio_generation = None;
        Some(ticket)
    }

    pub(crate) fn is_current(&self, ticket: RecordingTicket) -> bool {
        self.ending.is_none() && self.request == Some(ticket) && self.revision == ticket.revision
    }

    pub(crate) fn can_control(&self, owner: RecordingOwner) -> bool {
        match owner {
            RecordingOwner::SpeechMike => self
                .request
                .is_some_and(|request| request.owner == RecordingOwner::SpeechMike),
            RecordingOwner::Other => self
                .request
                .is_none_or(|request| request.owner != RecordingOwner::SpeechMike),
        }
    }

    pub(crate) fn started(&mut self, ticket: RecordingTicket, generation: u64) {
        if self.is_current(ticket) {
            self.audio_generation = Some(generation);
        }
    }

    /// Other audio entry points do not use this controller (for example wake word).
    /// Forget a hardware claim when one of those entry points replaces its session.
    pub(crate) fn reconcile_audio(&mut self, generation: u64, keyboard_trigger: bool) {
        if let Some(request) = self.request {
            if request.owner == RecordingOwner::SpeechMike {
                let expected = self.audio_generation.unwrap_or(request.audio_before_start);
                if generation != expected || (self.audio_generation.is_some() && !keyboard_trigger)
                {
                    self.invalidate();
                }
            }
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.request = None;
        self.audio_generation = None;
        // A running stop owns the drained pipeline until it returns. Cancellation
        // invalidates its audio generation immediately, but no new start may take
        // over the shared recorder while that stop is still unwinding.
        if self.ending.is_none_or(|ending| !ending.running) {
            self.ending = None;
        }
    }

    pub(crate) fn reserve_stop(&mut self) -> Option<u64> {
        if self.ending.is_some() {
            return None;
        }
        self.invalidate();
        self.ending = Some(EndingRequest {
            revision: self.revision,
            running: false,
        });
        Some(self.revision)
    }

    pub(crate) fn stop_is_current(&self, revision: u64) -> bool {
        self.revision == revision
            && self
                .ending
                .is_some_and(|ending| ending.revision == revision)
    }

    pub(crate) fn begin_stop(&mut self, revision: u64) -> bool {
        if !self.stop_is_current(revision) {
            return false;
        }
        match self.ending.as_mut() {
            Some(ending) if !ending.running => {
                ending.running = true;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn complete_stop(&mut self, revision: u64) {
        // A cancelled running stop still has to release its own reservation.
        if self
            .ending
            .is_some_and(|ending| ending.revision == revision)
        {
            self.ending = None;
        }
    }

    pub(crate) fn is_ending(&self) -> bool {
        self.ending.is_some()
    }
}

pub struct RecordingState {
    pub(crate) source: Mutex<RecordingSource>,
    pub(crate) lifecycle: Mutex<RecordingLifecycle>,
    pub(crate) ending: AtomicBool,
    pub(crate) last_mode_switch: Mutex<Instant>,
    pub(crate) last_toggle_stop: Mutex<Instant>,
    pub(crate) last_toggle_start: Mutex<Instant>,
}

impl RecordingState {
    pub fn new() -> Self {
        let past = Instant::now() - Duration::from_secs(1);
        Self {
            source: Mutex::new(RecordingSource::None),
            lifecycle: Mutex::new(RecordingLifecycle::default()),
            ending: AtomicBool::new(false),
            last_mode_switch: Mutex::new(past),
            last_toggle_stop: Mutex::new(past),
            last_toggle_start: Mutex::new(past),
        }
    }
}

static RECORDING_STATE: once_cell::sync::Lazy<RecordingState> =
    once_cell::sync::Lazy::new(RecordingState::new);

pub fn recording_state() -> &'static RecordingState {
    &RECORDING_STATE
}

#[cfg(test)]
mod tests {
    use super::{RecordingLifecycle, RecordingOwner};

    #[test]
    fn native_start_admission_rejects_active_audio_and_non_idle_sources() {
        let mut lifecycle = RecordingLifecycle::default();
        assert!(lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, true, true)
            .is_none());
        assert!(lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, false, false)
            .is_none());
        let ticket = lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, true, false)
            .unwrap();
        assert!(lifecycle.is_current(ticket));
        assert!(lifecycle.can_control(RecordingOwner::SpeechMike));
        assert!(!lifecycle.can_control(RecordingOwner::Other));
    }

    #[test]
    fn cancellation_invalidates_a_start_before_its_worker_runs() {
        let mut lifecycle = RecordingLifecycle::default();
        let queued = lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, true, false)
            .unwrap();
        lifecycle.invalidate();
        assert!(!lifecycle.is_current(queued));

        let keyboard = lifecycle
            .reserve_start(RecordingOwner::Other, 10, true, false)
            .unwrap();
        assert!(lifecycle.is_current(keyboard));
        assert!(!lifecycle.can_control(RecordingOwner::SpeechMike));
        assert!(lifecycle.can_control(RecordingOwner::Other));
        assert!(!lifecycle.is_current(queued));
    }

    #[test]
    fn finalization_blocks_starts_and_a_cancel_invalidates_queued_stops() {
        let mut lifecycle = RecordingLifecycle::default();
        lifecycle.reserve_start(RecordingOwner::SpeechMike, 10, true, false);
        let stop = lifecycle.reserve_stop().unwrap();
        assert!(lifecycle.stop_is_current(stop));
        assert!(lifecycle.is_ending());
        assert!(lifecycle
            .reserve_start(RecordingOwner::Other, 10, true, false)
            .is_none());

        lifecycle.invalidate();
        assert!(!lifecycle.stop_is_current(stop));
        assert!(!lifecycle.is_ending());
        let replacement_stop = lifecycle.reserve_stop().unwrap();
        lifecycle.complete_stop(stop);
        assert!(lifecycle.stop_is_current(replacement_stop));
        lifecycle.complete_stop(replacement_stop);
        assert!(!lifecycle.is_ending());
    }

    #[test]
    fn cancellation_of_a_running_stop_keeps_admission_closed_until_completion() {
        let mut lifecycle = RecordingLifecycle::default();
        lifecycle.reserve_start(RecordingOwner::SpeechMike, 10, true, false);
        let stop = lifecycle.reserve_stop().unwrap();
        assert!(lifecycle.begin_stop(stop));
        assert!(!lifecycle.begin_stop(stop));
        assert!(lifecycle.reserve_stop().is_none());

        lifecycle.invalidate();
        assert!(!lifecycle.stop_is_current(stop));
        assert!(lifecycle.is_ending());
        assert!(lifecycle
            .reserve_start(RecordingOwner::Other, 12, true, false)
            .is_none());
        lifecycle.complete_stop(stop);
        assert!(!lifecycle.is_ending());
        assert!(lifecycle
            .reserve_start(RecordingOwner::Other, 12, true, false)
            .is_some());
    }

    #[test]
    fn an_obsolete_stop_cannot_release_a_newer_session() {
        let mut lifecycle = RecordingLifecycle::default();
        let old = lifecycle.reserve_stop().unwrap();
        lifecycle.invalidate();
        assert!(!lifecycle.begin_stop(old));

        let current = lifecycle.reserve_stop().unwrap();
        assert!(lifecycle.begin_stop(current));
        lifecycle.complete_stop(old);
        assert!(lifecycle.is_ending());
        lifecycle.complete_stop(current);
        let keyboard = lifecycle
            .reserve_start(RecordingOwner::Other, 15, true, false)
            .unwrap();
        lifecycle.complete_stop(current);
        assert!(lifecycle.is_current(keyboard));
    }

    #[test]
    fn a_completed_speechmike_start_keeps_ownership_until_its_audio_is_replaced() {
        let mut lifecycle = RecordingLifecycle::default();
        let ticket = lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, true, false)
            .unwrap();
        lifecycle.reconcile_audio(10, false);
        assert!(lifecycle.is_current(ticket));
        lifecycle.started(ticket, 11);
        lifecycle.reconcile_audio(11, true);
        assert!(lifecycle.can_control(RecordingOwner::SpeechMike));

        lifecycle.reconcile_audio(12, true);
        assert!(!lifecycle.can_control(RecordingOwner::SpeechMike));
        assert!(!lifecycle.is_current(ticket));
        assert!(lifecycle.can_control(RecordingOwner::Other));
    }

    #[test]
    fn an_external_start_or_wake_word_trigger_retires_hardware_ownership() {
        for pending in [true, false] {
            let mut lifecycle = RecordingLifecycle::default();
            let ticket = lifecycle
                .reserve_start(RecordingOwner::SpeechMike, 10, true, false)
                .unwrap();
            if pending {
                lifecycle.reconcile_audio(11, true);
            } else {
                lifecycle.started(ticket, 11);
                lifecycle.reconcile_audio(11, false);
            }
            assert!(!lifecycle.can_control(RecordingOwner::SpeechMike));
            assert!(!lifecycle.is_current(ticket));
        }
    }

    #[test]
    fn replacing_an_external_source_never_grants_a_stale_hardware_ticket() {
        let mut lifecycle = RecordingLifecycle::default();
        let obsolete = lifecycle
            .reserve_start(RecordingOwner::SpeechMike, 10, true, false)
            .unwrap();
        lifecycle.reconcile_audio(11, false);
        let current = lifecycle
            .reserve_start(RecordingOwner::Other, 12, true, false)
            .unwrap();
        lifecycle.started(obsolete, 11);
        assert!(lifecycle.is_current(current));
        assert!(!lifecycle.is_current(obsolete));
        assert!(!lifecycle.can_control(RecordingOwner::SpeechMike));
    }
}
