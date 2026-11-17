// Protocol mapping reference: GoogleChromeLabs/dictation_support (Apache-2.0).
// Copyright 2022 Google LLC.
// https://github.com/GoogleChromeLabs/dictation_support/blob/077292a27ba77e40468fb464889fe3b5c121737d/src/speechmike_hid_device.ts
// Full license: licenses/dictation-support-APACHE-2.0.txt.

use std::fmt;

use crate::shortcuts::types::{ActivationMode, KeyEventType};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ControlAction {
    Record(ActivationMode, KeyEventType),
    Cancel,
    PasteLast,
    CancelOwned,
}

#[derive(Debug, Default)]
pub(crate) struct Controls {
    pub(super) enabled: bool,
    pub(super) record_available: bool,
    pub(super) held_record_mode: Option<ActivationMode>,
    pub(super) baseline_ready: bool,
    pub(super) baseline_buttons: Option<Vec<Button>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeError {
    EmptyReport,
    UnexpectedCommand,
    TruncatedReport,
    UnsupportedDeviceFamily,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyReport => "Empty SpeechMike report",
            Self::UnexpectedCommand => "Unexpected SpeechMike report type",
            Self::TruncatedReport => "Truncated SpeechMike report",
            Self::UnsupportedDeviceFamily => "Unsupported SpeechMike device family",
        })
    }
}

impl std::error::Error for DecodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Button {
    Record,
    Instruction,
    InsertOverwrite,
    EndOfLetter,
    Play,
    Rewind,
    Forward,
    F1,
    F2,
    F3,
    F4,
}

impl Button {
    const ALL: [Self; 11] = [
        Self::Record,
        Self::Instruction,
        Self::InsertOverwrite,
        Self::EndOfLetter,
        Self::Play,
        Self::Rewind,
        Self::Forward,
        Self::F1,
        Self::F2,
        Self::F3,
        Self::F4,
    ];

    fn mask(self) -> u16 {
        match self {
            Self::Record => 1 << 8,
            Self::Instruction => 1 << 15,
            Self::InsertOverwrite => 1 << 14,
            Self::EndOfLetter => 1 << 13,
            Self::Play => 1 << 10,
            Self::Rewind => 1 << 12,
            Self::Forward => 1 << 11,
            Self::F1 => 1 << 1,
            Self::F2 => 1 << 2,
            Self::F3 => 1 << 3,
            Self::F4 => 1 << 4,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Record => "REC",
            Self::Instruction => "INSTRUCTION",
            Self::InsertOverwrite => "INS/OVR",
            Self::EndOfLetter => "EOL/PRIO",
            Self::Play => "PLAY",
            Self::Rewind => "REWIND",
            Self::Forward => "FORWARD",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ButtonChange {
    pub(crate) button: Button,
    pub(crate) pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ButtonUpdate {
    InitialState(Vec<Button>),
    Changes(Vec<ButtonChange>),
}

#[derive(Debug, Default)]
pub(crate) struct ButtonTracker {
    previous: Option<u16>,
}

impl ButtonTracker {
    pub(crate) fn snapshot(&self) -> Option<ButtonUpdate> {
        self.previous.map(|mask| {
            ButtonUpdate::InitialState(
                Button::ALL
                    .into_iter()
                    .filter(|button| mask & button.mask() != 0)
                    .collect(),
            )
        })
    }

    pub(crate) fn update(&mut self, mask: u16) -> ButtonUpdate {
        let previous = self.previous.replace(mask);
        match previous {
            None => ButtonUpdate::InitialState(
                Button::ALL
                    .into_iter()
                    .filter(|button| mask & button.mask() != 0)
                    .collect(),
            ),
            Some(previous) => ButtonUpdate::Changes(
                Button::ALL
                    .into_iter()
                    .filter_map(|button| {
                        let pressed = mask & button.mask() != 0;
                        let was_pressed = previous & button.mask() != 0;
                        (pressed != was_pressed).then_some(ButtonChange { button, pressed })
                    })
                    .collect(),
            ),
        }
    }
}
