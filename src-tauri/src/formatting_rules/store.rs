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
    fn hospital_original_commands_replace_standalone_dictation() {
        let settings = hospital_settings();
        for (transcription, expected) in [
            ("Insérer un point.", ". "),
            ("INSÉRER UNE VIRGULE.", ", "),
            ("Saut de ligne.", "\n"),
            ("Nouveau paragraphe.", "\n\n"),
            ("Ouvrir les guillemets.", "\" "),
            ("Fermer les guillemets.", "\" "),
        ] {
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
            ("insérer un point insérer un point", ".. "),
            ("Bonjour insérer un point insérer un point", "Bonjour.. "),
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

    fn assert_hospital_formats(cases: &[(&str, &str)]) {
        let settings = hospital_settings();
        for (transcription, expected) in cases {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                *expected,
                "Unexpected formatting for synthetic transcription: {transcription:?}"
            );
        }
    }

    #[test]
    fn hospital_supports_the_thirty_voice_commands_with_recognizer_punctuation() {
        assert_hospital_formats(&[
            ("Insérer un point.", ". "),
            ("Insérer une virgule.", ", "),
            ("Insérer un point-virgule.", "; "),
            ("Insérer deux points.", ": "),
            ("Insérer un point d'interrogation.", "? "),
            ("Insérer un point d'exclamation.", "! "),
            ("Insérer des points de suspension.", "… "),
            ("Insérer un tiret.", "- "),
            ("Ouvrir les guillemets.", "\" "),
            ("Fermer les guillemets.", "\" "),
            ("Ouvrir une parenthèse.", "( "),
            ("Fermer une parenthèse.", ") "),
            ("Ouvrir un crochet.", "[ "),
            ("Fermer un crochet.", "] "),
            ("Ouvrir une accolade.", "{ "),
            ("Fermer une accolade.", "} "),
            ("Saut de ligne.", "\n"),
            ("Nouveau paragraphe.", "\n\n"),
            ("Insérer une puce.", "• "),
            ("Nouvelle puce.", "\n• "),
            ("Insérer un pour cent.", "% "),
            ("Insérer un signe plus.", "+ "),
            ("Insérer un signe moins.", "− "),
            ("Insérer un signe égal.", "= "),
            ("Insérer un signe fois.", "× "),
            ("Insérer une barre oblique.", "/ "),
            ("Insérer une arobase.", "@ "),
            ("Insérer un dièse.", "# "),
            ("Insérer un symbole degré.", "° "),
            ("Insérer un astérisque.", "* "),
        ]);
    }

    #[test]
    fn hospital_ui_examples_match_the_real_formatter() {
        let examples: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/features/personalize/formatting-rules/hospital-rule-examples.json"
        ))
        .expect("Hospital UI examples must contain valid JSON");
        let examples = examples
            .as_object()
            .expect("Examples must be keyed by rule ID");
        assert_eq!(examples.len(), 30);
        let settings = hospital_settings();
        for (id, example) in examples {
            assert!(settings.rules.iter().any(|rule| rule.id == *id));
            let spoken = example["spoken"].as_str().expect("Spoken example required");
            let expected = example["result"].as_str().expect("Example result required");
            let actual = crate::formatting_rules::apply_formatting(spoken.to_owned(), &settings);
            // The tooltip omits the final space used for insertion, but keeps
            // actual line breaks so users can see paragraph and bullet examples.
            assert_eq!(
                actual.trim_end_matches(' '),
                expected,
                "Invalid example: {id}"
            );
        }
    }

    #[test]
    fn hospital_spoken_punctuation_takes_priority_over_adjacent_automatic_punctuation() {
        assert_hospital_formats(&[
            ("Bonjour, insérer une virgule monde.", "Bonjour, monde. "),
            ("Bonjour, insérer une virgule, monde.", "Bonjour, monde. "),
            ("Bonjour insérer une virgule , monde.", "Bonjour, monde. "),
            ("Bonjour. Insérer une virgule. Monde.", "Bonjour, Monde. "),
            (
                "Bonjour , . insérer une virgule . , monde.",
                "Bonjour, monde. ",
            ),
            ("Bonjour.	Insérer un point.	Suite.", "Bonjour. Suite. "),
            ("Bonjour,insérer une virgule,monde.", "Bonjour, monde. "),
            (", insérer une virgule , .", ", "),
        ]);
    }

    #[test]
    fn hospital_long_commands_are_not_partially_replaced_by_point_or_puce() {
        assert_hospital_formats(&[
            ("insérer un point virgule", "; "),
            ("insérer un point‐virgule", "; "),
            ("insérer un point d’interrogation", "? "),
            ("insérer un point d’exclamation", "! "),
            (
                "Bonjour insérer un point-virgule suite insérer un point",
                "Bonjour ; suite. ",
            ),
            (
                "Est-ce correct insérer un point d'interrogation",
                "Est-ce correct ? ",
            ),
            ("Attention insérer un point d'exclamation", "Attention ! "),
            ("insérer un point’", ".’ "),
            ("Texte, insérer un point’", "Texte.’ "),
            ("Texte insérer un point/fin", "Texte./fin "),
            (
                "Première partie insérer deux points suite",
                "Première partie : suite ",
            ),
            ("insérer une puce alpha", "• alpha "),
            (
                "nouvelle puce alpha nouvelle puce bêta",
                "\n• alpha\n• bêta ",
            ),
        ]);
    }

    #[test]
    fn hospital_mixed_punctuation_preserves_numbers_abbreviations_and_negations() {
        let settings = hospital_settings();
        for transcription in [
            "Valeur fictive : 12,5 ; autre valeur : 12.5.",
            "Attendez... aucune modification n'est demandée.",
            "Le Dr. Exemple ne constate rien.",
            "Version 2.0.3, le 01/02/2026.",
            "Ni l'un, ni l'autre : rien n'est confirmé !",
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                format!("{transcription} ")
            );
        }
        assert_hospital_formats(&[
            (
                "Valeur 12,5, insérer une virgule pas de changement.",
                "Valeur 12,5, pas de changement. ",
            ),
            (
                "Valeur 12.5. insérer un point Aucune négation n'est supprimée.",
                "Valeur 12.5. Aucune négation n'est supprimée. ",
            ),
        ]);
    }

    #[test]
    fn hospital_new_commands_preserve_ordinary_words_and_complete_word_boundaries() {
        let settings = hospital_settings();
        for transcription in [
            "Une parenthèse, un crochet, une accolade et une puce.",
            "Un pour cent, un signe plus et un symbole degré.",
            "Réinsérer un tiret et insérer un tiretage.",
            "Ouvrir une parenthèses et fermer une parenthèses.",
            "De nouvelles puces et un nouveau paragraphes.",
        ] {
            assert_eq!(
                crate::formatting_rules::apply_formatting(transcription.to_string(), &settings),
                format!("{transcription} ")
            );
        }
    }

    #[test]
    fn hospital_new_bracket_commands_keep_text_and_generated_punctuation() {
        assert_hospital_formats(&[
            (
                "Exemple ouvrir une parenthèse texte fermer une parenthèse insérer un point",
                "Exemple (texte). ",
            ),
            (
                "Ouvrir un crochet texte fermer un crochet insérer une virgule suite",
                "[texte], suite ",
            ),
            (
                "Ouvrir une accolade texte fermer une accolade insérer un point",
                "{texte}. ",
            ),
            ("Insérer un point-virgule. Insérer une virgule.", ";, "),
            (
                "Insérer un point d'interrogation. Saut de ligne. Suite.",
                "?\nSuite. ",
            ),
            (
                "Bonjour. Saut de ligne. Suite insérer un point.",
                "Bonjour.\nSuite. ",
            ),
        ]);
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
