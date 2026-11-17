// Protocol constants and wire layout adapted from GoogleChromeLabs/dictation_support
// (Apache-2.0), Copyright 2022 Google LLC. Hardware behavior still needs validation.
// https://github.com/GoogleChromeLabs/dictation_support/blob/077292a27ba77e40468fb464889fe3b5c121737d/src/dictation_device_manager.ts
// https://github.com/GoogleChromeLabs/dictation_support/blob/077292a27ba77e40468fb464889fe3b5c121737d/src/speechmike_hid_device.ts
// Full license: licenses/dictation-support-APACHE-2.0.txt.

#[cfg(test)]
use super::types::{Button, ButtonChange};
pub(crate) use super::types::{ButtonTracker, ButtonUpdate, DecodeError};

#[cfg(target_os = "windows")]
pub(crate) const VENDOR_ID: u16 = 0x0911;
#[cfg(target_os = "windows")]
pub(crate) const PRODUCT_ID: u16 = 0x0c1c;
#[cfg(target_os = "windows")]
pub(crate) const USAGE_PAGE: u16 = 0xffa0;
#[cfg(target_os = "windows")]
pub(crate) const USAGE: u16 = 1;
pub(crate) const DEVICE_CODE_COMMAND: u8 = 0x8b;
pub(crate) const EVENT_MODE_COMMAND: u8 = 0x8d;
pub(crate) const BUTTONS_COMMAND: u8 = 0x80;

fn validate_report(report: &[u8], command: u8, min_length: usize) -> Result<(), DecodeError> {
    match report.first() {
        None => Err(DecodeError::EmptyReport),
        Some(value) if *value != command => Err(DecodeError::UnexpectedCommand),
        Some(_) if report.len() < min_length => Err(DecodeError::TruncatedReport),
        Some(_) => Ok(()),
    }
}

pub(crate) fn device_code(report: &[u8]) -> Result<u16, DecodeError> {
    validate_report(report, DEVICE_CODE_COMMAND, 8)?;
    if report[1] != 0 {
        return Err(DecodeError::UnsupportedDeviceFamily);
    }

    // The reference driver reads these three model fields in big endian order.
    let air = u16::from_be_bytes([report[2], report[3]]);
    let touch = u16::from_be_bytes([report[4], report[5]]);
    let premium = u16::from_be_bytes([report[6], report[7]]);
    Ok(air.max(touch).max(premium))
}

pub(crate) fn event_mode(report: &[u8]) -> Result<u8, DecodeError> {
    validate_report(report, EVENT_MODE_COMMAND, 9)?;
    Ok(report[8])
}

pub(crate) fn button_mask(report: &[u8]) -> Result<u16, DecodeError> {
    validate_report(report, BUTTONS_COMMAND, 9)?;
    // HIDAPI omits the report ID for an unnumbered input report. The caller must
    // pass that payload unchanged; guessing or stripping bytes would shift bits.
    Ok(u16::from_le_bytes([report[7], report[8]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buttons(mask: u16) -> [u8; 9] {
        let [low, high] = mask.to_le_bytes();
        [BUTTONS_COMMAND, 0, 0, 0, 0, 0, 0, low, high]
    }

    #[test]
    fn button_payload_decodes_little_endian_record_and_function_buttons() {
        let mask = button_mask(&[0x80, 0, 0, 0, 0, 0, 0, 0x02, 0x01]).unwrap();
        assert_eq!(mask, 0x0102);

        let mut tracker = ButtonTracker::default();
        assert_eq!(
            tracker.update(mask),
            ButtonUpdate::InitialState(vec![Button::Record, Button::F1])
        );
        assert_eq!(Button::Record.label(), "REC");
    }

    #[test]
    fn every_truncated_button_payload_is_rejected() {
        let report = buttons(0x0100);
        assert_eq!(button_mask(&[]), Err(DecodeError::EmptyReport));
        for length in 1..report.len() {
            assert_eq!(
                button_mask(&report[..length]),
                Err(DecodeError::TruncatedReport)
            );
        }
    }

    #[test]
    fn wrong_command_and_report_id_prefix_are_not_guessed() {
        assert_eq!(
            button_mask(&[0x81, 0, 0, 0, 0, 0, 0, 0, 1]),
            Err(DecodeError::UnexpectedCommand)
        );
        assert_eq!(
            button_mask(&[0, 0x80, 0, 0, 0, 0, 0, 0, 0, 1]),
            Err(DecodeError::UnexpectedCommand)
        );
    }

    #[test]
    fn padding_after_a_valid_report_does_not_change_the_button_mask() {
        let mut report = buttons(0x0100).to_vec();
        report.extend_from_slice(&[0xff; 16]);
        assert_eq!(button_mask(&report), Ok(0x0100));
    }

    #[test]
    fn premium_touch_model_is_decoded_in_big_endian_order() {
        let report = [0x8b, 0, 0, 0, 0x0e, 0x74, 0, 0];
        assert_eq!(device_code(&report), Ok(3700));
    }

    #[test]
    fn highest_model_field_matches_the_reference_protocol() {
        let report = [0x8b, 0, 0x0f, 0xa0, 0x0e, 0x74, 0x0d, 0xac];
        assert_eq!(device_code(&report), Ok(4000));
    }

    #[test]
    fn speechone_response_is_rejected_instead_of_read_as_a_touch_model() {
        assert_eq!(
            device_code(&[0x8b, 1, 0, 0, 0x0e, 0x74, 0, 0]),
            Err(DecodeError::UnsupportedDeviceFamily)
        );
    }

    #[test]
    fn model_queries_reject_empty_short_or_unrelated_responses() {
        assert_eq!(device_code(&[]), Err(DecodeError::EmptyReport));
        assert_eq!(
            device_code(&[0x8b, 0, 0, 0, 0x0e, 0x74, 0]),
            Err(DecodeError::TruncatedReport)
        );
        assert_eq!(
            device_code(&[0x80, 0, 0, 0, 0x0e, 0x74, 0, 0]),
            Err(DecodeError::UnexpectedCommand)
        );
    }

    #[test]
    fn event_mode_query_reads_the_mode_without_changing_it() {
        assert_eq!(event_mode(&[0x8d, 0, 0, 0, 0, 0, 0, 0, 0]), Ok(0));
        assert_eq!(event_mode(&[0x8d, 0, 0, 0, 0, 0, 0, 0, 1]), Ok(1));
        assert_eq!(event_mode(&[0x8d, 0, 0, 0, 0, 0, 0, 0, 0xff]), Ok(0xff));
        assert_eq!(event_mode(&[]), Err(DecodeError::EmptyReport));
        assert_eq!(event_mode(&[0x8d; 8]), Err(DecodeError::TruncatedReport));
        assert_eq!(event_mode(&[0x80; 9]), Err(DecodeError::UnexpectedCommand));
    }

    #[test]
    fn initially_held_record_button_is_a_baseline_not_a_press() {
        let mut tracker = ButtonTracker::default();
        assert_eq!(
            tracker.update(0x0100),
            ButtonUpdate::InitialState(vec![Button::Record])
        );
        assert_eq!(tracker.update(0x0100), ButtonUpdate::Changes(Vec::new()));
        assert_eq!(
            tracker.update(0),
            ButtonUpdate::Changes(vec![ButtonChange {
                button: Button::Record,
                pressed: false,
            }])
        );
    }

    #[test]
    fn press_hold_and_release_emit_only_two_changes() {
        let mut tracker = ButtonTracker::default();
        assert_eq!(tracker.update(0), ButtonUpdate::InitialState(Vec::new()));
        assert_eq!(
            tracker.update(0x0100),
            ButtonUpdate::Changes(vec![ButtonChange {
                button: Button::Record,
                pressed: true,
            }])
        );
        assert_eq!(tracker.update(0x0100), ButtonUpdate::Changes(Vec::new()));
        assert_eq!(
            tracker.update(0),
            ButtonUpdate::Changes(vec![ButtonChange {
                button: Button::Record,
                pressed: false,
            }])
        );
        assert_eq!(tracker.update(0), ButtonUpdate::Changes(Vec::new()));
    }

    #[test]
    fn simultaneous_buttons_and_partial_releases_are_independent() {
        let mut tracker = ButtonTracker::default();
        tracker.update(0);
        assert_eq!(
            tracker.update(0x0106),
            ButtonUpdate::Changes(vec![
                ButtonChange {
                    button: Button::Record,
                    pressed: true,
                },
                ButtonChange {
                    button: Button::F1,
                    pressed: true,
                },
                ButtonChange {
                    button: Button::F2,
                    pressed: true,
                },
            ])
        );
        assert_eq!(
            tracker.update(0x0002),
            ButtonUpdate::Changes(vec![
                ButtonChange {
                    button: Button::Record,
                    pressed: false,
                },
                ButtonChange {
                    button: Button::F2,
                    pressed: false,
                },
            ])
        );
    }

    #[test]
    fn unrecognized_bits_do_not_create_physical_button_events() {
        let mut tracker = ButtonTracker::default();
        assert_eq!(
            tracker.update(0x0201),
            ButtonUpdate::InitialState(Vec::new())
        );
        assert_eq!(tracker.update(0), ButtonUpdate::Changes(Vec::new()));
        assert_eq!(
            tracker.update(0x0301),
            ButtonUpdate::Changes(vec![ButtonChange {
                button: Button::Record,
                pressed: true,
            }])
        );
    }

    #[test]
    fn decode_errors_do_not_include_report_content() {
        let report = [0xef, 0x53, 0x45, 0x43, 0x52, 0x45, 0x54, 0, 0];
        let error = button_mask(&report).unwrap_err();
        assert_eq!(error.to_string(), "Unexpected SpeechMike report type");
        assert_eq!(format!("{error:?}"), "UnexpectedCommand");
    }
}
