use crate::audio::chunking::ChunkPipeline;
use crate::audio::recorder::AudioRecorder;
use crate::engine::ParakeetEngine;
use cpal::Device;
use parking_lot::Mutex;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct MicInfo {
    pub id: String,
    pub label: String,
}

#[derive(Default)]
pub struct PreviewSnapshot {
    pub queue: Vec<f32>,
    pub generation: u64,
    pub revision: u64,
}

pub struct AudioState {
    pub recorder: Mutex<Option<AudioRecorder>>,
    pub engine: Mutex<Option<ParakeetEngine>>,
    /// Readiness must remain observable while inference holds the engine lock.
    engine_ready: AtomicBool,
    pub current_file_name: Mutex<Option<String>>,
    recording_mode: AtomicU8,
    recording_trigger: AtomicU8,
    /// Cached audio input device to avoid re-enumerating devices on each recording
    pub cached_device: Mutex<Option<Device>>,
    /// Wake word to strip from the end of the transcription (set by validate trigger)
    pub strip_word: Mutex<Option<String>>,
    pub streaming_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub streaming_stop: Arc<AtomicBool>,
    pub preview_snapshot: Arc<Mutex<PreviewSnapshot>>,
    pub chunk_inference_active: Arc<AtomicBool>,
    /// The chunking pipeline of the active session
    pub chunk_pipeline: Mutex<Option<ChunkPipeline>>,
    session_gen: AtomicU64,
    /// Spans the whole session, from the record request to the end of
    /// post-processing. Unlike `recorder`, reading it never blocks on the
    /// microphone init and it stays true while the LLM finalizes.
    session_active: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingMode {
    Standard = 0,
    Llm = 1,
    Command = 2,
}

impl RecordingMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            RecordingMode::Standard => "standard",
            RecordingMode::Llm => "llm",
            RecordingMode::Command => "command",
        }
    }
}

impl From<u8> for RecordingMode {
    fn from(val: u8) -> Self {
        match val {
            0 => RecordingMode::Standard,
            1 => RecordingMode::Llm,
            2 => RecordingMode::Command,
            _ => RecordingMode::Standard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordingTrigger {
    Keyboard = 0,
    WakeWord = 1,
}

impl From<u8> for RecordingTrigger {
    fn from(val: u8) -> Self {
        match val {
            1 => RecordingTrigger::WakeWord,
            _ => RecordingTrigger::Keyboard,
        }
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            recorder: Mutex::new(None),
            engine: Mutex::new(None),
            engine_ready: AtomicBool::new(false),
            current_file_name: Mutex::new(None),
            recording_mode: AtomicU8::new(RecordingMode::Standard as u8),
            recording_trigger: AtomicU8::new(RecordingTrigger::Keyboard as u8),
            cached_device: Mutex::new(None),
            strip_word: Mutex::new(None),
            streaming_handle: Mutex::new(None),
            streaming_stop: Arc::new(AtomicBool::new(false)),
            preview_snapshot: Arc::new(Mutex::new(PreviewSnapshot::default())),
            chunk_inference_active: Arc::new(AtomicBool::new(false)),
            chunk_pipeline: Mutex::new(None),
            session_gen: AtomicU64::new(0),
            session_active: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_engine_ready(&self) -> bool {
        self.engine_ready.load(Ordering::Acquire)
    }

    /// Call only after a successfully loaded engine has been installed.
    pub(crate) fn mark_engine_ready(&self) {
        self.engine_ready.store(true, Ordering::Release);
    }

    pub fn begin_session(&self) -> u64 {
        self.session_gen.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn invalidate_session(&self) {
        self.session_gen.fetch_add(1, Ordering::SeqCst);
    }

    pub fn current_session(&self) -> u64 {
        self.session_gen.load(Ordering::SeqCst)
    }

    pub fn set_session_active(&self, active: bool) {
        self.session_active.store(active, Ordering::SeqCst);
    }

    pub fn is_session_active(&self) -> bool {
        self.session_active.load(Ordering::SeqCst)
    }

    pub fn set_recording_mode(&self, mode: RecordingMode) {
        self.recording_mode.store(mode as u8, Ordering::SeqCst);
    }

    pub fn get_recording_mode(&self) -> RecordingMode {
        self.recording_mode.load(Ordering::SeqCst).into()
    }

    pub fn set_recording_trigger(&self, trigger: RecordingTrigger) {
        self.recording_trigger
            .store(trigger as u8, Ordering::SeqCst);
    }

    pub fn get_recording_trigger(&self) -> RecordingTrigger {
        self.recording_trigger.load(Ordering::SeqCst).into()
    }

    /// Sets the cached audio device
    pub fn set_cached_device(&self, device: Option<Device>) {
        *self.cached_device.lock() = device;
    }

    /// Gets a clone of the cached audio device
    pub fn get_cached_device(&self) -> Option<Device> {
        self.cached_device.lock().clone()
    }
}

pub(super) enum RecorderStartError {
    Busy,
    DirUnavailable,
    InitFailed,
    StartFailed,
    Superseded,
}

#[cfg(test)]
mod tests {
    use super::AudioState;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    #[test]
    fn engine_readiness_starts_false_and_is_marked_after_loading() {
        let state = AudioState::new();
        assert!(!state.is_engine_ready());

        state.mark_engine_ready();
        assert!(state.is_engine_ready());

        state.mark_engine_ready();
        assert!(state.is_engine_ready());
    }

    #[test]
    fn engine_readiness_is_independent_of_recording_sessions() {
        let state = AudioState::new();
        state.mark_engine_ready();

        state.begin_session();
        state.set_session_active(true);
        assert!(state.is_engine_ready());

        state.invalidate_session();
        state.set_session_active(false);
        assert!(state.is_engine_ready());
    }

    #[test]
    fn engine_readiness_does_not_wait_for_the_engine_lock() {
        let state = Arc::new(AudioState::new());
        state.mark_engine_ready();
        let engine_guard = state.engine.lock();
        let (sender, receiver) = mpsc::channel();
        let reader_state = state.clone();
        let reader = std::thread::spawn(move || {
            let _ = sender.send(reader_state.is_engine_ready());
        });

        let readiness = receiver.recv_timeout(Duration::from_secs(2));
        // Release before joining so a regression cannot leave the test blocked.
        drop(engine_guard);
        reader.join().expect("readiness reader should complete");
        assert_eq!(readiness, Ok(true));
    }
}
