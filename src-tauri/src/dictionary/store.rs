use std::{fs, sync::Arc};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::Store;

use crate::dictionary::{normalize_import_content, DictionaryError};

fn open_store(app: &AppHandle) -> Result<Arc<Store<Wry>>, String> {
    crate::utils::defaults::store_with_defaults(
        app,
        "dictionary.json",
        "hospital/default-dictionnary.json",
        validate_bundled_dictionary,
    )
}

fn validate_bundled_dictionary(value: &serde_json::Value) -> Result<(), String> {
    let words = value
        .as_object()
        .ok_or_else(|| "Bundled dictionary must be a JSON object".to_string())?;
    for (word, languages) in words {
        if word.trim().is_empty()
            || !languages
                .as_array()
                .is_some_and(|languages| languages.iter().all(serde_json::Value::is_string))
        {
            return Err(
                "Bundled dictionary does not match the native dictionary format".to_string(),
            );
        }
    }
    Ok(())
}

fn contains_word_case_insensitive(words: &[String], word: &str) -> bool {
    words.iter().any(|w| w.eq_ignore_ascii_case(word))
}

/// Words are the store keys. The value is a vestigial per-word language list:
/// ignored on read and written empty, so the on-disk format stays compatible
/// with older versions in both directions.
pub fn load(app: &AppHandle) -> Result<Vec<String>, String> {
    let store = open_store(app)?;
    Ok(store.entries().into_iter().map(|(word, _)| word).collect())
}

pub fn save(app: &AppHandle, words: &[String]) -> Result<(), String> {
    let store = open_store(app)?;
    store.reset();
    for word in words {
        store.set(word, serde_json::json!([]));
    }
    Ok(())
}

pub fn migrate_and_load(
    app: &AppHandle,
    dictionary_from_settings: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut words = load(app)?;
    if !dictionary_from_settings.is_empty() {
        for word in dictionary_from_settings {
            if !contains_word_case_insensitive(&words, &word) {
                words.push(word);
            }
        }
        save(app, &words)?;
    }
    Ok(words)
}

pub fn export_dictionary(app: &AppHandle, file_path: String) -> Result<(), String> {
    log::debug!("Exporting dictionary to file: {}", file_path);
    let words = load(app)?;
    let content = match words.is_empty() {
        true => String::new(),
        false => format!("{}\n", words.join("\n")),
    };

    fs::write(&file_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_dictionary_format(words: &[String]) -> Result<(), DictionaryError> {
    for word in words {
        let has_digit = word.chars().any(|c| c.is_ascii_digit());
        let space_count = word.chars().filter(|c| *c == ' ').count();
        if has_digit || space_count > 1 {
            return Err(DictionaryError::InvalidWordFormat(word.to_string()));
        }
    }
    if words.is_empty() {
        return Err(DictionaryError::EmptyDictionary);
    }
    Ok(())
}

pub fn import_dictionary(app: &AppHandle, file_path: String) -> Result<(), String> {
    let raw = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    log::debug!("New dictionary: {} from file: {}", raw, &file_path);

    let is_legacy_csv = std::path::Path::new(&file_path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));

    let normalized = normalize_import_content(&raw, is_legacy_csv);
    validate_dictionary_format(&normalized).map_err(|e| e.to_string())?;
    let mut words = load(app)?;
    for word in normalized {
        if !contains_word_case_insensitive(&words, &word) {
            words.push(word);
        }
    }
    save(app, &words)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hospital_dictionary_model_matches_the_native_format() {
        let value = serde_json::from_str(include_str!(
            "../../../resources/hospital/default-dictionnary.json"
        ))
        .expect("Hospital dictionary model must contain valid JSON");

        assert!(validate_bundled_dictionary(&value).is_ok());
    }

    #[test]
    fn hospital_dictionary_entries_can_be_used_for_phrase_boosting() {
        use crate::engine::helpers::{load_tokenizer, tokenize_word_to_ids, word_variants};

        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../resources/hospital/default-dictionnary.json"
        ))
        .unwrap_or_else(|_| panic!("Hospital dictionary model must contain valid JSON"));
        let entries = value
            .as_object()
            .expect("Hospital dictionary model must be a JSON object");
        assert!(
            !entries.is_empty(),
            "Hospital dictionary must contain entries"
        );

        let tokenizer_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/tokenizer.json");
        let tokenizer = load_tokenizer(Some(&tokenizer_path))
            .expect("Bundled tokenizer must be available for dictionary validation");

        for entry in entries.keys() {
            assert!(
                word_variants(entry)
                    .iter()
                    .any(|variant| tokenize_word_to_ids(&tokenizer, variant).is_some()),
                "Every hospital dictionary entry must have a tokenizable variant"
            );
        }
    }

    #[test]
    fn bundled_dictionary_accepts_empty_and_legacy_language_lists() {
        for value in [
            serde_json::json!({}),
            serde_json::json!({"Terme fictif": [], "Exemple": ["fr", "en"]}),
        ] {
            assert!(validate_bundled_dictionary(&value).is_ok());
        }
    }

    #[test]
    fn bundled_dictionary_rejects_invalid_structures_without_exposing_values() {
        for value in [
            serde_json::json!([]),
            serde_json::json!({"synthetic-private-word": "fr"}),
            serde_json::json!({"synthetic-private-word": [42]}),
            serde_json::json!({" ": []}),
        ] {
            let error = validate_bundled_dictionary(&value).unwrap_err();
            assert!(!error.contains("synthetic-private-word"));
        }
    }

    #[test]
    fn test_validate_dictionary_format_valid_multiple_words() {
        let result =
            validate_dictionary_format(&normalize_import_content("hello\nWORLD\ntest", false));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_trims_whitespace() {
        let result = validate_dictionary_format(&normalize_import_content(
            "  hello  \n  world  \n  test  ",
            false,
        ));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_skips_empty_entries() {
        let result =
            validate_dictionary_format(&normalize_import_content("hello\n\nworld\n\ntest", false));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_invalid_with_numbers() {
        let result =
            validate_dictionary_format(&normalize_import_content("hello\nworld123\ntest", false));
        assert!(result.is_err());
        match result.unwrap_err() {
            DictionaryError::InvalidWordFormat(word) => {
                assert_eq!(word, "world123");
            }
            _ => panic!("Expected InvalidWordFormat error"),
        }
    }

    #[test]
    fn test_validate_dictionary_format_valid_with_hyphen() {
        let result =
            validate_dictionary_format(&normalize_import_content("hello\nworld-test\ntest", false));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_valid_two_word_pair() {
        let result = validate_dictionary_format(&normalize_import_content(
            "hello \nworld test\ntest",
            false,
        ));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_invalid_multiple_spaces() {
        let result =
            validate_dictionary_format(&normalize_import_content("hello\na b c\ntest", false));
        assert!(result.is_err());
        match result.unwrap_err() {
            DictionaryError::InvalidWordFormat(word) => {
                assert_eq!(word, "a b c");
            }
            _ => panic!("Expected InvalidWordFormat error"),
        }
    }

    #[test]
    fn test_validate_dictionary_format_valid_with_apostrophe() {
        let result = validate_dictionary_format(&normalize_import_content("aujourd'hui", false));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_dictionary_format_empty_string() {
        let result = validate_dictionary_format(&normalize_import_content("", false));
        assert!(result.is_err());
        match result.unwrap_err() {
            DictionaryError::EmptyDictionary => {}
            _ => panic!("Expected EmptyDictionary error"),
        }
    }

    #[test]
    fn test_validate_dictionary_format_only_newlines() {
        let result = validate_dictionary_format(&normalize_import_content("\n\n\n", false));
        assert!(result.is_err());
        match result.unwrap_err() {
            DictionaryError::EmptyDictionary => {}
            _ => panic!("Expected EmptyDictionary error"),
        }
    }
}
