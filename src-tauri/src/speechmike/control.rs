use crate::shortcuts::types::{ActivationMode, KeyEventType};

use super::types::{Button, ButtonUpdate, ControlAction, Controls};

impl Controls {
    pub(super) fn reset_baseline(&mut self) {
        self.baseline_ready = false;
        self.baseline_buttons = None;
        self.held_record_mode = None;
    }

    /// Keep the last physical snapshot, but never replay a press from another
    /// input profile. Buttons held during a switch have to be released first.
    pub(super) fn rearm(&mut self) {
        self.held_record_mode = None;
        self.baseline_ready = self.baseline_buttons.as_ref().is_some_and(Vec::is_empty);
    }

    /// The first observed state is not a fresh press. Arm only once all held
    /// buttons have been released, including when identification supplied it.
    pub(super) fn observe_baseline(&mut self, update: &ButtonUpdate) -> bool {
        match update {
            ButtonUpdate::InitialState(buttons) => self.baseline_buttons = Some(buttons.clone()),
            ButtonUpdate::Changes(changes) => {
                if let Some(buttons) = self.baseline_buttons.as_mut() {
                    for change in changes {
                        if change.pressed && !buttons.contains(&change.button) {
                            buttons.push(change.button);
                        } else if !change.pressed {
                            buttons.retain(|button| *button != change.button);
                        }
                    }
                }
            }
        }
        if !self.baseline_ready {
            self.baseline_ready = self.baseline_buttons.as_ref().is_some_and(Vec::is_empty);
        }
        self.baseline_ready
    }

    pub(super) fn set_record_available(&mut self, available: bool) {
        self.record_available = available;
        if !available {
            self.held_record_mode = None;
        }
    }

    /// Disarming on capture, suspension or disconnect also cancels a toggle session.
    pub(super) fn set_enabled(&mut self, enabled: bool) -> Option<ControlAction> {
        let cancel_owned = self.enabled && !enabled;
        self.enabled = enabled;
        if !enabled {
            self.held_record_mode = None;
        }
        cancel_owned.then_some(ControlAction::CancelOwned)
    }

    pub(super) fn buttons(
        &mut self,
        update: ButtonUpdate,
        mode: ActivationMode,
    ) -> Vec<ControlAction> {
        if !self.enabled {
            return Vec::new();
        }
        let changes = match update {
            // Connecting with a held button must never start recording or paste.
            ButtonUpdate::InitialState(_) => {
                self.held_record_mode = None;
                return Vec::new();
            }
            ButtonUpdate::Changes(changes) => changes,
        };

        // Cancel wins over REC release in the same report: no unwanted transcription.
        if changes
            .iter()
            .any(|change| change.button == Button::F1 && change.pressed)
        {
            self.held_record_mode = None;
            return vec![ControlAction::Cancel];
        }

        let mut actions = Vec::new();
        for change in changes {
            match (change.button, change.pressed) {
                (Button::Record, true)
                    if self.record_available && self.held_record_mode.is_none() =>
                {
                    self.held_record_mode = Some(mode.clone());
                    actions.push(ControlAction::Record(mode.clone(), KeyEventType::Pressed));
                }
                (Button::Record, false) => {
                    if let Some(pressed_mode) = self.held_record_mode.take() {
                        actions.push(ControlAction::Record(pressed_mode, KeyEventType::Released));
                    }
                }
                (Button::F2, true) => actions.push(ControlAction::PasteLast),
                _ => {}
            }
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::ButtonChange;
    use super::*;

    fn change(button: Button, pressed: bool) -> ButtonUpdate {
        ButtonUpdate::Changes(vec![ButtonChange { button, pressed }])
    }

    fn enabled() -> Controls {
        let mut controls = Controls::default();
        assert_eq!(controls.set_enabled(true), None);
        controls.set_record_available(true);
        controls
    }

    #[test]
    fn connecting_with_held_buttons_does_not_record_cancel_or_paste() {
        let mut controls = enabled();
        assert!(controls
            .buttons(
                ButtonUpdate::InitialState(vec![Button::Record, Button::F1, Button::F2]),
                ActivationMode::ToggleToTalk,
            )
            .is_empty());
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::ToggleToTalk)
            .is_empty());
    }

    #[test]
    fn record_preserves_press_and_release_for_the_native_controller() {
        let mut controls = enabled();
        assert_eq!(
            controls.buttons(change(Button::Record, true), ActivationMode::ToggleToTalk),
            vec![ControlAction::Record(
                ActivationMode::ToggleToTalk,
                KeyEventType::Pressed
            )]
        );
        assert_eq!(
            controls.buttons(change(Button::Record, false), ActivationMode::ToggleToTalk),
            vec![ControlAction::Record(
                ActivationMode::ToggleToTalk,
                KeyEventType::Released
            )]
        );
    }

    #[test]
    fn release_uses_the_mode_at_press_even_if_preferences_change() {
        let mut controls = enabled();
        controls.buttons(change(Button::Record, true), ActivationMode::PushToTalk);
        assert_eq!(
            controls.buttons(change(Button::Record, false), ActivationMode::ToggleToTalk),
            vec![ControlAction::Record(
                ActivationMode::PushToTalk,
                KeyEventType::Released
            )]
        );
    }

    #[test]
    fn cancel_has_priority_and_disarms_pending_record_release() {
        let mut controls = enabled();
        controls.buttons(change(Button::Record, true), ActivationMode::ToggleToTalk);
        assert_eq!(
            controls.buttons(
                ButtonUpdate::Changes(vec![
                    ButtonChange {
                        button: Button::Record,
                        pressed: false
                    },
                    ButtonChange {
                        button: Button::F2,
                        pressed: true
                    },
                    ButtonChange {
                        button: Button::F1,
                        pressed: true
                    },
                ]),
                ActivationMode::ToggleToTalk
            ),
            vec![ControlAction::Cancel]
        );
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::ToggleToTalk)
            .is_empty());
    }

    #[test]
    fn disconnect_or_suspension_cancels_owned_once_and_requires_a_new_press() {
        let mut controls = enabled();
        controls.buttons(change(Button::Record, true), ActivationMode::PushToTalk);
        assert_eq!(
            controls.set_enabled(false),
            Some(ControlAction::CancelOwned)
        );
        assert_eq!(controls.set_enabled(false), None);
        assert!(controls
            .buttons(change(Button::F2, true), ActivationMode::PushToTalk)
            .is_empty());
        controls.set_enabled(true);
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::PushToTalk)
            .is_empty());
        assert_eq!(
            controls.buttons(change(Button::Record, true), ActivationMode::PushToTalk),
            vec![ControlAction::Record(
                ActivationMode::PushToTalk,
                KeyEventType::Pressed
            )]
        );
    }

    #[test]
    fn a_press_ignored_during_capture_does_not_toggle_after_capture() {
        let mut controls = Controls::default();
        assert!(controls
            .buttons(change(Button::Record, true), ActivationMode::ToggleToTalk)
            .is_empty());
        controls.set_enabled(true);
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::ToggleToTalk)
            .is_empty());
    }

    #[test]
    fn f2_pastes_only_on_press_and_unassigned_buttons_are_ignored() {
        let mut controls = enabled();
        assert_eq!(
            controls.buttons(change(Button::F2, true), ActivationMode::PushToTalk),
            vec![ControlAction::PasteLast]
        );
        assert!(controls
            .buttons(change(Button::F2, false), ActivationMode::PushToTalk)
            .is_empty());
        for button in [
            Button::F3,
            Button::F4,
            Button::Play,
            Button::Instruction,
            Button::Rewind,
            Button::Forward,
        ] {
            assert!(controls
                .buttons(change(button, true), ActivationMode::PushToTalk)
                .is_empty());
        }
    }

    #[test]
    fn repeated_record_press_does_not_create_another_start() {
        let mut controls = enabled();
        controls.buttons(change(Button::Record, true), ActivationMode::PushToTalk);
        assert!(controls
            .buttons(change(Button::Record, true), ActivationMode::PushToTalk)
            .is_empty());
    }

    #[test]
    fn record_pressed_while_busy_cannot_start_on_a_later_release() {
        let mut controls = enabled();
        controls.set_record_available(false);
        assert!(controls
            .buttons(change(Button::Record, true), ActivationMode::ToggleToTalk)
            .is_empty());
        controls.set_record_available(true);
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::ToggleToTalk)
            .is_empty());
        controls.set_record_available(false);
        assert_eq!(
            controls.buttons(change(Button::F1, true), ActivationMode::ToggleToTalk),
            vec![ControlAction::Cancel]
        );
        assert_eq!(
            controls.buttons(change(Button::F2, true), ActivationMode::ToggleToTalk),
            vec![ControlAction::PasteLast]
        );
    }

    #[test]
    fn readiness_waits_for_a_known_released_baseline() {
        let mut controls = Controls::default();
        assert!(!controls.observe_baseline(&change(Button::Record, false)));
        assert!(!controls.observe_baseline(&ButtonUpdate::InitialState(vec![Button::Record])));
        assert!(!controls.observe_baseline(&change(Button::F2, true)));
        assert!(!controls.observe_baseline(&change(Button::Record, false)));
        assert!(controls.observe_baseline(&change(Button::F2, false)));
        controls.reset_baseline();
        assert!(!controls.observe_baseline(&change(Button::Record, false)));
        assert!(controls.observe_baseline(&ButtonUpdate::InitialState(Vec::new())));
    }

    #[test]
    fn transport_snapshot_preserves_state_without_emitting_new_presses() {
        use super::super::types::ButtonTracker;
        let mut tracker = ButtonTracker::default();
        assert_eq!(tracker.snapshot(), None);
        tracker.update(0x0100);
        assert_eq!(
            tracker.snapshot(),
            Some(ButtonUpdate::InitialState(vec![Button::Record]))
        );
        tracker.update(0);
        assert_eq!(
            tracker.snapshot(),
            Some(ButtonUpdate::InitialState(Vec::new()))
        );
    }

    #[test]
    fn profile_switch_waits_for_buttons_pressed_while_hardware_was_inactive() {
        let mut controls = Controls::default();
        assert!(controls.observe_baseline(&ButtonUpdate::InitialState(Vec::new())));
        // Tracking continues while the keyboard controls another microphone.
        assert!(controls.observe_baseline(&change(Button::F2, true)));
        controls.rearm();
        assert!(!controls.baseline_ready);
        assert!(controls.observe_baseline(&change(Button::F2, false)));
        controls.set_enabled(true);
        controls.set_record_available(true);
        assert!(controls
            .buttons(change(Button::F2, false), ActivationMode::PushToTalk)
            .is_empty());
        assert_eq!(
            controls.buttons(change(Button::F2, true), ActivationMode::PushToTalk),
            vec![ControlAction::PasteLast]
        );
    }

    #[test]
    fn a_rec_held_before_profile_activation_does_not_start_on_release() {
        let mut controls = Controls::default();
        controls.observe_baseline(&ButtonUpdate::InitialState(Vec::new()));
        controls.observe_baseline(&change(Button::Record, true));
        controls.rearm();
        assert!(!controls.baseline_ready);
        assert!(controls.observe_baseline(&change(Button::Record, false)));
        controls.set_enabled(true);
        controls.set_record_available(true);
        assert!(controls
            .buttons(change(Button::Record, false), ActivationMode::ToggleToTalk)
            .is_empty());
        assert_eq!(
            controls.buttons(change(Button::Record, true), ActivationMode::ToggleToTalk),
            vec![ControlAction::Record(
                ActivationMode::ToggleToTalk,
                KeyEventType::Pressed
            )]
        );
    }
}
