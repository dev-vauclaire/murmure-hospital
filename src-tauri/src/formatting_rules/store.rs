use super::types::{FormattingSettings, MatchMode};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::Store;

const STORE_FILE: &str = "formatting_rules.json";
const SETTINGS_KEY: &str = "settings";

fn open_store(app: &AppHandle) -> Result<Arc<Store<Wry>>, String> {
    crate::utils::defaults::store_with_defaults(
        app,
        STORE_FILE,
        "hospital/default-formatting-rules.json",
        validate_bundled_formatting_rules,
    )
}

fn validate_bundled_formatting_rules(value: &serde_json::Value) -> Result<(), String> {
    let settings = value
        .get(SETTINGS_KEY)
        .ok_or_else(|| "Bundled formatting rules must contain a settings object".to_string())?;
    let settings: FormattingSettings = serde_json::from_value(settings.clone())
        .map_err(|_| "Bundled formatting rules do not match the native format".to_string())?;
    let mut ids = HashSet::new();
    for rule in &settings.rules {
        if rule.id.trim().is_empty() || !ids.insert(&rule.id) {
            return Err(
                "Bundled formatting rules must have unique nonempty identifiers".to_string(),
            );
        }
        if rule.match_mode == MatchMode::Regex && regex::Regex::new(&rule.trigger).is_err() {
            return Err(
                "Bundled formatting rules contain an invalid regular expression".to_string(),
            );
        }
    }
    Ok(())
}

/// Load formatting settings from the store
pub fn load(app: &AppHandle) -> Result<FormattingSettings, String> {
    let store = open_store(app)?;

    match store.get(SETTINGS_KEY) {
        Some(value) => serde_json::from_value::<FormattingSettings>(value)
            .map_err(|e| format!("Failed to parse formatting settings: {}", e)),
        None => Ok(FormattingSettings::default()),
    }
}

/// Save formatting settings to the store
pub fn save(app: &AppHandle, settings: &FormattingSettings) -> Result<(), String> {
    let store = open_store(app)?;

    let value = serde_json::to_value(settings)
        .map_err(|e| format!("Failed to serialize formatting settings: {}", e))?;

    store.set(SETTINGS_KEY, value);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hospital_formatting_model_matches_the_native_format() {
        let value = serde_json::from_str(include_str!(
            "../../../resources/hospital/default-formatting-rules.json"
        ))
        .expect("Hospital formatting model must contain valid JSON");

        assert!(validate_bundled_formatting_rules(&value).is_ok());
    }

    fn hospital_settings() -> FormattingSettings {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../resources/hospital/default-formatting-rules.json"
        ))
        .expect("Hospital formatting model must contain valid JSON");
        serde_json::from_value(value[SETTINGS_KEY].clone())
            .expect("Hospital formatting model must match the native settings format")
    }

    #[test]
    fn hospital_commands_are_present_in_the_dictionary_and_replace_standalone_dictation() {
        let dictionary: serde_json::Value = serde_json::from_str(include_str!(
            "../../../resources/hospital/default-dictionnary.json"
        ))
        .expect("Hospital dictionary model must contain valid JSON");
        let settings = hospital_settings();
        for (command, transcription, expected) in [
            ("insérer un point", "Insérer un point.", ". "),
            ("insérer une virgule", "INSÉRER UNE VIRGULE.", ", "),
            ("saut de ligne", "Saut de ligne.", "\n"),
            ("nouveau paragraphe", "Nouveau paragraphe.", "\n\n"),
            ("ouvrir les guillemets", "Ouvrir les guillemets.", "\" "),
            ("fermer les guillemets", "Fermer les guillemets.", "\" "),
        ] {
            assert!(
                dictionary.get(command).is_some(),
                "Every default voice command must have a dictionary entry"
            );
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                expected
            );
        }
    }

    #[test]
    fn hospital_commands_format_sentences_with_clean_spacing() {
        let settings = hospital_settings();
        for (transcription, expected) in [
            (
                "Bonjour insérer une virgule comment allez-vous insérer un point",
                "Bonjour, comment allez-vous. ",
            ),
            ("Bonjour. Saut de ligne. Suite.", "Bonjour.\nSuite. "),
            ("Titre nouveau paragraphe Contenu.", "Titre\n\nContenu. "),
            (
                "Exemple ouvrir les guillemets Bonjour fermer les guillemets insérer un point",
                "Exemple \"Bonjour\". ",
            ),
            (
                "Ouvrir les guillemets. Bonjour fermer les guillemets.",
                "\"Bonjour\" ",
            ),
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                expected
            );
        }
    }

    #[test]
    fn hospital_commands_preserve_ordinary_words_and_short_text_punctuation() {
        let settings = hospital_settings();
        for transcription in [
            "Un point douloureux, une virgule et une pointure.",
            "Réinsérer un point, insérer un pointage, nouveau paragraphes et saut de lignes.",
            "Bonjour.",
            "Vingt-trois exemples.",
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                format!("{transcription} ")
            );
        }
    }

    #[test]
    fn hospital_line_commands_preserve_consecutive_line_breaks() {
        let settings = hospital_settings();
        for (transcription, expected) in [
            ("saut de ligne saut de ligne", "\n\n"),
            ("nouveau paragraphe nouveau paragraphe", "\n\n\n\n"),
            (
                "Bonjour saut de ligne suite nouveau paragraphe fin.",
                "Bonjour\nsuite\n\nfin. ",
            ),
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                expected
            );
        }
    }

    #[test]
    fn hospital_command_sequences_preserve_generated_punctuation() {
        let settings = hospital_settings();
        for (transcription, expected) in [
            ("insérer un point insérer une virgule", "., "),
            ("insérer une virgule insérer un point", ",. "),
            ("Insérer un point. Insérer une virgule.", "., "),
            ("Fermer les guillemets. Insérer un point.", "\". "),
            ("Insérer un point. Saut de ligne.", ".\n"),
            ("Insérer un point. Nouveau paragraphe.", ".\n\n"),
            ("Saut de ligne. Insérer un point.", "\n. "),
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                expected
            );
        }
    }

    #[test]
    fn hospital_commands_normalize_recognizer_punctuation_before_replacement() {
        let settings = hospital_settings();
        for (command, expected) in [
            ("insérer un point", ". "),
            ("insérer une virgule", ", "),
            ("saut de ligne", "\n"),
            ("nouveau paragraphe", "\n\n"),
            ("ouvrir les guillemets", "\" "),
            ("fermer les guillemets", "\" "),
        ] {
            for punctuation in [".", ",", "..", ",."] {
                assert_eq!(
                    crate::formatting_rules::apply_formatting(
                        format!("{command}{punctuation}"),
                        &settings
                    ),
                    expected
                );
            }
        }
        assert_eq!(
            crate::formatting_rules::apply_formatting(
                "Bonjour insérer un point.Suite.".to_string(),
                &settings
            ),
            "Bonjour. Suite. "
        );
    }

    fn model_with_rules(rules: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"settings": {"built_in": {}, "rules": rules}})
    }

    fn synthetic_rule(id: &str, trigger: &str, mode: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "trigger": trigger,
            "replacement": ".",
            "enabled": true,
            "match_mode": mode
        })
    }

    #[test]
    fn bundled_rules_accept_empty_and_synthetic_smart_exact_regex_rules() {
        assert!(
            validate_bundled_formatting_rules(&model_with_rules(serde_json::json!([]))).is_ok()
        );
        let value = model_with_rules(serde_json::json!([
            synthetic_rule("smart-example", "point", "smart"),
            synthetic_rule("exact-example", "POINT", "exact"),
            synthetic_rule("regex-example", "(?i)\\bpoint\\b", "regex")
        ]));
        assert!(validate_bundled_formatting_rules(&value).is_ok());
    }

    #[test]
    fn bundled_rules_reject_missing_settings_and_invalid_shapes() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"settings": null}),
            serde_json::json!({"settings": {"built_in": {}, "rules": "invalid"}}),
            model_with_rules(serde_json::json!([synthetic_rule(
                "id", "point", "unknown"
            )])),
        ] {
            assert!(validate_bundled_formatting_rules(&value).is_err());
        }
    }

    #[test]
    fn bundled_rules_reject_duplicate_and_blank_identifiers() {
        for rules in [
            serde_json::json!([synthetic_rule(" ", "point", "smart")]),
            serde_json::json!([
                synthetic_rule("duplicate", "point", "smart"),
                synthetic_rule("duplicate", "virgule", "smart")
            ]),
        ] {
            assert!(validate_bundled_formatting_rules(&model_with_rules(rules)).is_err());
        }
    }

    #[test]
    fn bundled_regex_validation_does_not_expose_rule_content() {
        let value = model_with_rules(serde_json::json!([synthetic_rule(
            "id",
            "(synthetic-private-pattern",
            "regex"
        )]));
        let error = validate_bundled_formatting_rules(&value).unwrap_err();

        assert!(!error.contains("synthetic-private-pattern"));
    }
}
