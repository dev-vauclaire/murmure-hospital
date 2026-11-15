use serde::Serialize;

/// UI state only: hardware identifiers remain inside the Windows backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MicrophoneControls {
    pub(crate) profile: &'static str,
    pub(crate) pending_profile: Option<&'static str>,
    pub(crate) speech_mike_status: &'static str,
    pub(crate) identity_status: &'static str,
}

impl Default for MicrophoneControls {
    fn default() -> Self {
        Self {
            profile: "keyboard",
            pending_profile: None,
            speech_mike_status: "unsupported",
            identity_status: "unsupported",
        }
    }
}

#[cfg(any(target_os = "windows", test))]
mod controller {
    use super::MicrophoneControls;
    use crate::shortcuts::types::{KeyEventType, ShortcutAction};

    pub(crate) fn is_keyboard_control_action(action: &ShortcutAction) -> bool {
        matches!(
            action,
            ShortcutAction::StartRecording
                | ShortcutAction::PasteLastTranscript
                | ShortcutAction::CancelRecording
        )
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum InputProfile {
        Keyboard,
        SpeechMike,
    }

    impl InputProfile {
        fn as_str(self) -> &'static str {
            match self {
                Self::Keyboard => "keyboard",
                Self::SpeechMike => "speechmike",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum IdentityStatus {
        Matched,
        NotMatched,
        Unavailable,
    }

    impl IdentityStatus {
        fn as_str(self) -> &'static str {
            match self {
                Self::Matched => "matched",
                Self::NotMatched => "not_matched",
                Self::Unavailable => "unavailable",
            }
        }
    }

    pub(crate) struct ProfileController {
        active: InputProfile,
        desired: InputProfile,
        identity: IdentityStatus,
        status: &'static str,
        revision: u64,
        verified_selection: Option<u64>,
        keyboard_press_selection: Option<u64>,
        hardware_press_selection: Option<u64>,
    }

    impl Default for ProfileController {
        fn default() -> Self {
            Self {
                active: InputProfile::Keyboard,
                desired: InputProfile::Keyboard,
                identity: IdentityStatus::Unavailable,
                status: "searching",
                revision: 0,
                verified_selection: None,
                keyboard_press_selection: None,
                hardware_press_selection: None,
            }
        }
    }

    impl ProfileController {
        pub(crate) fn snapshot(&self) -> MicrophoneControls {
            MicrophoneControls {
                profile: self.active.as_str(),
                pending_profile: (self.desired != self.active).then_some(self.desired.as_str()),
                speech_mike_status: self.status,
                identity_status: self.identity.as_str(),
            }
        }

        pub(crate) fn profile(&self) -> InputProfile {
            self.active
        }

        pub(crate) fn revision(&self) -> u64 {
            self.revision
        }

        pub(crate) fn set_status(&mut self, status: &'static str) {
            self.status = status;
        }

        pub(crate) fn set_identity(&mut self, identity: IdentityStatus) {
            self.identity = identity;
            self.desired = match identity {
                IdentityStatus::Matched => InputProfile::SpeechMike,
                IdentityStatus::NotMatched | IdentityStatus::Unavailable => InputProfile::Keyboard,
            };
        }

        pub(crate) fn confirm_identity(&mut self, identity: IdentityStatus, selection: u64) {
            // Odd revisions mean the persisted choice/cache is being changed.
            if !selection.is_multiple_of(2) {
                return;
            }
            self.set_identity(identity);
            self.verified_selection = Some(selection);
        }

        pub(crate) fn can_start(&self, selection: u64) -> bool {
            selection.is_multiple_of(2)
                && self.verified_selection == Some(selection)
                && self.active == self.desired
        }

        /// A toggle releases into a new start only if its press belongs to the
        /// same validated selection. Existing owners retain their stop gesture.
        pub(crate) fn admit_record_event(
            &mut self,
            origin: InputProfile,
            event: KeyEventType,
            selection: u64,
            source_idle: bool,
            busy: bool,
        ) -> bool {
            let can_start = self.active == origin && self.can_start(selection);
            let gesture = match origin {
                InputProfile::Keyboard => &mut self.keyboard_press_selection,
                InputProfile::SpeechMike => &mut self.hardware_press_selection,
            };
            if self.active != origin {
                *gesture = None;
                return false;
            }
            if !source_idle {
                *gesture = None;
                return true;
            }
            if busy {
                *gesture = None;
                return false;
            }
            match event {
                KeyEventType::Pressed => {
                    *gesture = can_start.then_some(selection);
                    can_start
                }
                KeyEventType::Released => gesture.take() == Some(selection) && can_start,
            }
        }

        /// Keep the owner of an in-flight recording able to stop it. A pending
        /// selection affects only the next session and never creates a dictation.
        pub(crate) fn settle(&mut self, busy: bool) {
            if !busy && self.active != self.desired {
                self.active = self.desired;
                self.revision = self.revision.wrapping_add(1);
                self.keyboard_press_selection = None;
                self.hardware_press_selection = None;
            }
        }
    }

    #[derive(Debug, Default)]
    pub(crate) struct KeyboardAdmission {
        revision: Option<u64>,
        armed: bool,
    }

    impl KeyboardAdmission {
        /// Re-enabling the keyboard must not turn a key held in the previous
        /// profile into a new press or an orphaned toggle release.
        pub(crate) fn update(
            &mut self,
            revision: u64,
            enabled: bool,
            controlled_keys_held: bool,
        ) -> bool {
            if self.revision != Some(revision) || !enabled {
                self.revision = Some(revision);
                self.armed = false;
            }
            if enabled && !controlled_keys_held {
                self.armed = true;
            }
            enabled && self.armed
        }
    }
}

#[cfg(any(target_os = "windows", test))]
pub(crate) use controller::{
    is_keyboard_control_action, IdentityStatus, InputProfile, KeyboardAdmission, ProfileController,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_confirmed_selection_activates_the_hardware_profile_only_while_idle() {
        let mut state = ProfileController::default();
        state.set_identity(IdentityStatus::Matched);
        state.settle(true);
        assert_eq!(state.profile(), InputProfile::Keyboard);
        assert_eq!(state.snapshot().pending_profile, Some("speechmike"));
        state.settle(false);
        assert_eq!(state.profile(), InputProfile::SpeechMike);
        assert_eq!(state.snapshot().pending_profile, None);
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn a_new_audio_choice_preserves_the_current_hardware_session_until_it_finishes() {
        let mut state = ProfileController::default();
        state.set_identity(IdentityStatus::Matched);
        state.settle(false);
        state.set_identity(IdentityStatus::NotMatched);
        state.settle(true);
        assert_eq!(state.profile(), InputProfile::SpeechMike);
        assert_eq!(state.snapshot().pending_profile, Some("keyboard"));
        state.settle(false);
        assert_eq!(state.profile(), InputProfile::Keyboard);
        assert_eq!(state.revision(), 2);
    }

    #[test]
    fn changing_the_selection_back_cancels_the_pending_transition() {
        let mut state = ProfileController::default();
        state.set_identity(IdentityStatus::Matched);
        state.settle(true);
        state.set_identity(IdentityStatus::NotMatched);
        state.settle(true);
        assert_eq!(state.snapshot().pending_profile, None);
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn an_unavailable_identity_does_not_guess_from_names_or_hid_status() {
        let mut state = ProfileController::default();
        state.set_status("ready");
        state.set_identity(IdentityStatus::Unavailable);
        state.settle(false);
        assert_eq!(state.profile(), InputProfile::Keyboard);
        assert_eq!(state.snapshot().speech_mike_status, "ready");
        assert_eq!(state.snapshot().identity_status, "unavailable");
    }

    #[test]
    fn a_matching_device_with_a_wrong_button_mode_keeps_its_hardware_guide() {
        let mut state = ProfileController::default();
        state.set_status("wrong_mode");
        state.set_identity(IdentityStatus::Matched);
        state.settle(false);
        assert_eq!(state.profile(), InputProfile::SpeechMike);
        assert_eq!(state.snapshot().speech_mike_status, "wrong_mode");
    }

    #[test]
    fn reconnected_hardware_does_not_replay_a_keyboard_key_held_during_the_switch() {
        let mut gate = KeyboardAdmission::default();
        assert!(gate.update(0, true, false));
        assert!(!gate.update(1, false, true));
        assert!(!gate.update(2, true, true));
        assert!(!gate.update(2, true, true));
        assert!(gate.update(2, true, false));
        assert!(gate.update(2, true, true));
    }

    #[test]
    fn every_profile_revision_requires_a_released_keyboard_baseline() {
        let mut gate = KeyboardAdmission::default();
        assert!(!gate.update(0, true, true));
        assert!(gate.update(0, true, false));
        assert!(!gate.update(2, true, true));
        assert!(gate.update(2, true, false));
    }

    #[test]
    fn the_serialized_ui_snapshot_contains_no_hardware_identifier() {
        let state = ProfileController::default();
        let value = serde_json::to_value(state.snapshot()).expect("A snapshot is serializable");
        assert_eq!(value.as_object().expect("Snapshot object").len(), 4);
        assert_eq!(value["profile"], "keyboard");
        assert!(value.get("pendingProfile").is_some());
        assert_eq!(value["speechMikeStatus"], "searching");
        assert_eq!(value["identityStatus"], "unavailable");
    }

    #[test]
    fn profile_switching_targets_only_the_three_exposed_keyboard_actions() {
        use crate::shortcuts::types::ShortcutAction;
        assert!(is_keyboard_control_action(&ShortcutAction::StartRecording));
        assert!(is_keyboard_control_action(
            &ShortcutAction::PasteLastTranscript
        ));
        assert!(is_keyboard_control_action(&ShortcutAction::CancelRecording));
        assert!(!is_keyboard_control_action(
            &ShortcutAction::StartRecordingCommand
        ));
        assert!(!is_keyboard_control_action(
            &ShortcutAction::ToggleVoiceMode
        ));
    }

    #[test]
    fn a_press_rejected_before_validation_cannot_start_on_a_later_toggle_release() {
        use crate::shortcuts::types::KeyEventType;
        let mut state = ProfileController::default();
        assert!(!state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Pressed,
            2,
            true,
            false
        ));
        state.confirm_identity(IdentityStatus::NotMatched, 2);
        assert!(!state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Released,
            2,
            true,
            false
        ));
        assert!(state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Pressed,
            2,
            true,
            false
        ));
        assert!(state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Released,
            2,
            true,
            false
        ));
    }

    #[test]
    fn a_toggle_press_from_an_old_selection_cannot_start_a_new_keyboard_microphone() {
        use crate::shortcuts::types::KeyEventType;
        let mut state = ProfileController::default();
        state.confirm_identity(IdentityStatus::NotMatched, 0);
        assert!(state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Pressed,
            0,
            true,
            false
        ));
        state.confirm_identity(IdentityStatus::NotMatched, 2);
        assert!(!state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Released,
            2,
            true,
            false
        ));
    }

    #[test]
    fn a_current_owner_keeps_its_stop_release_while_the_new_selection_is_unverified() {
        use crate::shortcuts::types::KeyEventType;
        let mut state = ProfileController::default();
        state.confirm_identity(IdentityStatus::NotMatched, 0);
        assert!(state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Pressed,
            0,
            true,
            false
        ));
        assert!(state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Released,
            1,
            false,
            true
        ));
        assert!(!state.can_start(1));
    }

    #[test]
    fn a_hardware_toggle_also_requires_the_press_selection_to_still_be_current() {
        use crate::shortcuts::types::KeyEventType;
        let mut state = ProfileController::default();
        state.confirm_identity(IdentityStatus::Matched, 0);
        state.settle(false);
        assert!(state.admit_record_event(
            InputProfile::SpeechMike,
            KeyEventType::Pressed,
            0,
            true,
            false
        ));
        state.confirm_identity(IdentityStatus::Matched, 2);
        assert!(!state.admit_record_event(
            InputProfile::SpeechMike,
            KeyEventType::Released,
            2,
            true,
            false
        ));
        assert!(state.admit_record_event(
            InputProfile::SpeechMike,
            KeyEventType::Released,
            3,
            false,
            true
        ));
    }

    #[test]
    fn an_identity_observed_during_a_selection_write_cannot_validate_new_starts() {
        let mut state = ProfileController::default();
        state.confirm_identity(IdentityStatus::NotMatched, 0);
        assert!(state.can_start(0));
        state.confirm_identity(IdentityStatus::Matched, 1);
        assert!(!state.can_start(1));
        assert_eq!(state.snapshot().identity_status, "not_matched");
        assert!(!state.can_start(2));
    }

    #[test]
    fn a_pending_profile_does_not_admit_a_new_session_before_settling() {
        use crate::shortcuts::types::KeyEventType;
        let mut state = ProfileController::default();
        state.confirm_identity(IdentityStatus::Matched, 2);
        state.settle(true);
        assert!(!state.admit_record_event(
            InputProfile::Keyboard,
            KeyEventType::Pressed,
            2,
            true,
            false
        ));
        state.settle(false);
        assert!(state.admit_record_event(
            InputProfile::SpeechMike,
            KeyEventType::Pressed,
            2,
            true,
            false
        ));
    }
}
