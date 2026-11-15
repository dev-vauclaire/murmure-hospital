use log::debug;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use crate::shortcuts::registry::ShortcutRegistryState;
use crate::shortcuts::types::{KeyEventType, ShortcutState};

const RELEASE_WATCH_MODIFIER_KEYS: &[i32] = &[0x11, 0x10, 0x12, 0x5B, 0x5C];

fn check_keys_pressed(keys: &[i32]) -> bool {
    keys.iter()
        .all(|&vk| (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0)
}

fn any_key_pressed(keys: &[i32]) -> bool {
    keys.iter()
        .any(|&vk| (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0)
}

pub(crate) fn any_modifier_held() -> bool {
    any_key_pressed(RELEASE_WATCH_MODIFIER_KEYS)
}

fn scan_pressed_vks() -> HashSet<i32> {
    (0x01..=0xFE)
        .filter(|&vk| (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0)
        .collect()
}

pub fn init(app: AppHandle) {
    std::thread::spawn(move || {
        app.state::<ShortcutState>().set_capture_available(true);
        debug!("Starting Windows keyboard polling");

        let mut active_bindings: HashSet<usize> = HashSet::new();
        let mut last_press_times: Vec<Instant> = Vec::new();
        let mut previous_pressed: HashSet<i32> = HashSet::new();
        let mut was_capturing = false;
        let mut keyboard_admission = crate::speechmike::KeyboardAdmission::default();
        let mut profile_revision = None;

        loop {
            let shortcut_state = app.state::<ShortcutState>();
            if shortcut_state.is_capturing() {
                if !was_capturing {
                    previous_pressed = scan_pressed_vks();
                    was_capturing = true;
                }

                let pressed = scan_pressed_vks();
                for &vk in pressed.difference(&previous_pressed) {
                    crate::shortcuts::capture::handle_capture_key(&app, vk);
                }
                previous_pressed = pressed;
                std::thread::sleep(Duration::from_millis(32));
                continue;
            }
            was_capturing = false;

            if shortcut_state.is_suspended() {
                std::thread::sleep(Duration::from_millis(32));
                continue;
            }

            // Read the profile before acquiring the registry: an admitted native
            // action may wait for a tray/UI task while a UI setter updates bindings.
            let (keyboard_enabled, revision) = crate::speechmike::keyboard_state(&app);
            let registry_state = app.state::<ShortcutRegistryState>();
            let registry = registry_state.0.read();
            if profile_revision != Some(revision) {
                // An old release must not stop or toggle a session after the
                // microphone switches input profiles. Leave hidden bindings alone.
                active_bindings.retain(|index| {
                    registry.bindings.get(*index).is_some_and(|binding| {
                        !crate::speechmike::is_keyboard_control_action(&binding.action)
                    })
                });
                profile_revision = Some(revision);
            }
            let controlled_keys_held = registry.bindings.iter().any(|binding| {
                crate::speechmike::is_keyboard_control_action(&binding.action)
                    && any_key_pressed(&binding.keys)
            });
            let keyboard_armed =
                keyboard_admission.update(revision, keyboard_enabled, controlled_keys_held);

            while last_press_times.len() < registry.bindings.len() {
                last_press_times.push(Instant::now() - Duration::from_secs(1));
            }

            for (i, binding) in registry.bindings.iter().enumerate() {
                if binding.keys.is_empty() {
                    continue;
                }
                if crate::speechmike::is_keyboard_control_action(&binding.action) && !keyboard_armed
                {
                    active_bindings.remove(&i);
                    continue;
                }

                let all_pressed = check_keys_pressed(&binding.keys);
                // Ensure no extra modifier keys are pressed beyond what the binding expects
                const MODIFIER_KEYS: &[i32] = &[0x11, 0x10, 0x12, 0x5B]; // Ctrl, Shift, Alt, Meta
                let extra_modifier_pressed = MODIFIER_KEYS.iter().any(|&vk| {
                    !binding.keys.contains(&vk)
                        && (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
                });

                if all_pressed && !extra_modifier_pressed && !active_bindings.contains(&i) {
                    // Debounce for auto-repeat
                    if last_press_times[i].elapsed() < Duration::from_millis(150) {
                        continue;
                    }

                    debug!("Shortcut Pressed: {:?}", binding.action);
                    last_press_times[i] = Instant::now();
                    active_bindings.insert(i);

                    let action = binding.action.clone();
                    let mode = binding.activation_mode.clone();
                    drop(registry);

                    crate::speechmike::handle_keyboard_event(
                        &app,
                        &action,
                        &mode,
                        KeyEventType::Pressed,
                    );
                    break;
                } else if !all_pressed && active_bindings.contains(&i) {
                    debug!("Shortcut Released: {:?}", binding.action);
                    active_bindings.remove(&i);

                    let action = binding.action.clone();
                    let mode = binding.activation_mode.clone();
                    drop(registry);

                    crate::speechmike::handle_keyboard_event(
                        &app,
                        &action,
                        &mode,
                        KeyEventType::Released,
                    );
                    break;
                }
            }

            std::thread::sleep(Duration::from_millis(32));
        }
    });
}
