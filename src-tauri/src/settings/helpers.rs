pub fn normalize_result_panel_mode(mode: &str) -> &'static str {
    match mode {
        "off" => "off",
        "command_llm" => "command_llm",
        "all" => "all",
        _ => "command",
    }
}

pub fn migrate_legacy_paste_method(settings: &mut serde_json::Value) {
    let Some(object) = settings.as_object_mut() else {
        return;
    };
    if matches!(
        object
            .get("paste_method")
            .and_then(serde_json::Value::as_str),
        Some("none" | "None")
    ) {
        object.insert("paste_method".to_string(), "ctrl_v".into());
        object.insert("auto_insert".to_string(), false.into());
    }
}

#[cfg(any(target_os = "windows", test))]
pub fn should_skip_automatic_start(
    start_on_login: bool,
    args: &[String],
    has_cli_action: bool,
) -> bool {
    !start_on_login && !has_cli_action && args.iter().any(|arg| arg == "--autostart")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_result_panel_modes_are_kept() {
        for mode in ["off", "command", "command_llm", "all"] {
            assert_eq!(normalize_result_panel_mode(mode), mode);
        }
    }

    #[test]
    fn unknown_result_panel_modes_fall_back_to_command() {
        for mode in ["commands", "", "Commands"] {
            assert_eq!(normalize_result_panel_mode(mode), "command");
        }
    }

    #[test]
    fn automatic_start_respects_the_user_preference() {
        let args = vec!["murmure-vauclaire".to_string(), "--autostart".to_string()];

        assert!(!should_skip_automatic_start(true, &args, false));
        assert!(should_skip_automatic_start(false, &args, false));
    }

    #[test]
    fn disabling_automatic_start_preserves_manual_and_hidden_launches() {
        for flags in [vec![], vec!["--hidden"]] {
            let args: Vec<String> = std::iter::once("murmure-vauclaire")
                .chain(flags)
                .map(str::to_string)
                .collect();

            assert!(!should_skip_automatic_start(false, &args, false));
        }
    }

    #[test]
    fn cli_actions_take_priority_over_a_disabled_automatic_start() {
        for action in [
            vec!["transcribe", "synthetic.wav"],
            vec!["import", "synthetic.murmure"],
            vec!["--transcription"],
            vec!["--quit"],
        ] {
            let args: Vec<String> = ["murmure-vauclaire", "--autostart"]
                .into_iter()
                .chain(action)
                .map(str::to_string)
                .collect();
            let command = crate::cli::parse_raw_args(&args).unwrap();

            assert!(command.is_some());
            assert!(!should_skip_automatic_start(
                false,
                &args,
                command.is_some()
            ));
        }
    }
}
