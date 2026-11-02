use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager};

use super::helpers::{migrate_legacy_paste_method, normalize_result_panel_mode};
use super::types::AppSettings;

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(format!("create_dir_all failed: {}", e));
    }
    Ok(dir.join("settings.json"))
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let path = match settings_path(app) {
        Ok(p) => p,
        Err(error) => {
            log::error!("Failed to locate user settings: {error}");
            return AppSettings::default();
        }
    };

    load_settings_from_path(&path, || {
        crate::utils::resources::resolve_resource_path(app, "hospital/settings.json")
    })
}

fn load_settings_from_path(
    path: &Path,
    bundled_settings: impl FnOnce() -> Option<PathBuf>,
) -> AppSettings {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match initialize_settings_file(path, bundled_settings()) {
                Ok(content) => content,
                Err(error) => {
                    log::error!("Failed to initialize user settings: {error}");
                    return AppSettings::default();
                }
            }
        }
        Err(error) => {
            log::error!("Failed to read user settings: {error}");
            return AppSettings::default();
        }
    };

    let mut settings = parse_settings(&content);
    settings.result_panel_mode =
        normalize_result_panel_mode(&settings.result_panel_mode).to_string();
    settings
}

fn initialize_settings_file(
    path: &Path,
    bundled_settings: Option<PathBuf>,
) -> Result<String, String> {
    let content = match bundled_settings {
        Some(bundled_path) => {
            let content = fs::read_to_string(bundled_path)
                .map_err(|error| format!("Failed to read bundled settings: {error}"))?;
            let value: serde_json::Value = serde_json::from_str(&content)
                .map_err(|_| "Bundled settings contain invalid JSON".to_string())?;

            // An exported .murmure envelope is not a native settings file. Serde's
            // defaults would otherwise silently accept it and ignore its settings.
            let object = value
                .as_object()
                .ok_or_else(|| "Bundled settings must be a JSON object".to_string())?;
            if ["version", "app_version", "exported_at", "categories"]
                .iter()
                .any(|key| object.contains_key(*key))
            {
                return Err("Bundled settings must not be a .murmure export".to_string());
            }
            // Do not include deserialization errors: they may contain values
            // from the configuration, including dictionary entries.
            deserialize_settings(value).map_err(|_| {
                "Bundled settings do not match the native settings format".to_string()
            })?;
            content
        }
        None => serde_json::to_string_pretty(&AppSettings::default())
            .map_err(|error| format!("Failed to serialize default settings: {error}"))?,
    };

    match create_settings_file(path, &content) {
        Ok(()) => Ok(content),
        // Another initializer may have created the profile after our first read.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => fs::read_to_string(path)
            .map_err(|error| format!("Failed to read existing user settings: {error}")),
        Err(error) => Err(format!("Failed to create user settings: {error}")),
    }
}

fn create_settings_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(error) = file.write_all(content.as_bytes()) {
        drop(file);
        if let Err(cleanup_error) = fs::remove_file(path) {
            log::error!("Failed to remove incomplete user settings: {cleanup_error}");
        }
        return Err(error);
    }
    Ok(())
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_path(app)?;
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

pub fn remove_dictionary_from_settings(
    app: &AppHandle,
    mut settings: AppSettings,
) -> Result<AppSettings, String> {
    settings.dictionary = Vec::new();
    save_settings(app, &settings)?;
    Ok(settings)
}

fn parse_settings(content: &str) -> AppSettings {
    serde_json::from_str(content)
        .and_then(deserialize_settings)
        .unwrap_or_default()
}

fn deserialize_settings(mut value: serde_json::Value) -> Result<AppSettings, serde_json::Error> {
    migrate_legacy_paste_method(&mut value);
    serde_json::from_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::PasteMethod;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("murmure-settings-tests-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).expect("Failed to create test directory");
            Self(path)
        }

        fn settings_path(&self) -> PathBuf {
            self.0.join("settings.json")
        }

        fn bundled_settings(&self, content: &str) -> PathBuf {
            let path = self.0.join("bundled-settings.json");
            fs::write(&path, content).expect("Failed to write test settings");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn legacy_none_paste_method_disables_auto_insert_and_keeps_other_settings() {
        for legacy in ["none", "None"] {
            let content = format!(
                r#"{{"paste_method":"{}","copy_to_clipboard":true,"api_port":4900}}"#,
                legacy
            );

            let settings = parse_settings(&content);

            assert_eq!(settings.paste_method, PasteMethod::CtrlV);
            assert!(!settings.auto_insert);
            assert!(settings.copy_to_clipboard);
            assert_eq!(settings.api_port, 4900);
        }
    }

    #[test]
    fn missing_auto_insert_defaults_to_true() {
        let settings = parse_settings(r#"{"paste_method":"direct"}"#);

        assert_eq!(settings.paste_method, PasteMethod::Direct);
        assert!(settings.auto_insert);
    }

    #[test]
    fn fresh_profile_copies_and_loads_bundled_settings() {
        let directory = TestDirectory::new();
        let content = "{\n  \"language\": \"fr\",\n  \"sound_volume\": 37,\n  \"paste_method\": \"direct\"\n}";
        let bundled_path = directory.bundled_settings(content);

        let settings = load_settings_from_path(&directory.settings_path(), || Some(bundled_path));

        assert_eq!(settings.language, "fr");
        assert_eq!(settings.sound_volume, 37);
        assert_eq!(settings.paste_method, PasteMethod::Direct);
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            content
        );
        assert!(settings.auto_insert);
    }

    #[test]
    fn existing_profile_is_preserved_without_resolving_bundled_settings() {
        let directory = TestDirectory::new();
        let content = r#"{"sound_volume":23}"#;
        fs::write(directory.settings_path(), content).unwrap();
        let resolved = std::cell::Cell::new(false);

        let settings = load_settings_from_path(&directory.settings_path(), || {
            resolved.set(true);
            None
        });

        assert!(!resolved.get());
        assert_eq!(settings.sound_volume, 23);
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            content
        );
    }

    #[test]
    fn later_launch_preserves_personal_preferences_after_model_changes() {
        let directory = TestDirectory::new();
        let bundled_path = directory.bundled_settings(r#"{"sound_volume":37}"#);
        load_settings_from_path(&directory.settings_path(), || Some(bundled_path.clone()));
        let personal_settings = r#"{"sound_volume":61,"paste_method":"direct"}"#;
        fs::write(directory.settings_path(), personal_settings).unwrap();
        fs::write(&bundled_path, r#"{"sound_volume":80}"#).unwrap();

        let settings = load_settings_from_path(&directory.settings_path(), || Some(bundled_path));

        assert_eq!(settings.sound_volume, 61);
        assert_eq!(settings.paste_method, PasteMethod::Direct);
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            personal_settings
        );
    }

    #[test]
    fn missing_bundled_settings_creates_native_defaults() {
        let directory = TestDirectory::new();

        let settings = load_settings_from_path(&directory.settings_path(), || None);
        let stored: AppSettings =
            serde_json::from_str(&fs::read_to_string(directory.settings_path()).unwrap()).unwrap();

        assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
        assert_eq!(stored.paste_method, AppSettings::default().paste_method);
    }

    #[test]
    fn invalid_bundled_settings_are_not_copied() {
        for content in [
            "{",
            "null",
            "[]",
            r#"{"sound_volume":"loud"}"#,
            r#"{"paste_method":"invalid"}"#,
            r#"{"version":1,"app_version":"2.0.3","categories":{"settings":{}}}"#,
        ] {
            let directory = TestDirectory::new();
            let bundled_path = directory.bundled_settings(content);

            let settings =
                load_settings_from_path(&directory.settings_path(), || Some(bundled_path));

            assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
            assert!(!directory.settings_path().exists());
        }
    }

    #[test]
    fn bundled_validation_errors_do_not_include_configuration_values() {
        let directory = TestDirectory::new();
        let bundled_path =
            directory.bundled_settings(r#"{"paste_method":"synthetic-private-value"}"#);

        let error =
            initialize_settings_file(&directory.settings_path(), Some(bundled_path)).unwrap_err();

        assert!(!error.contains("synthetic-private-value"));
        assert!(!directory.settings_path().exists());
    }

    #[test]
    fn invalid_existing_profile_is_not_replaced() {
        let directory = TestDirectory::new();
        let content = "invalid existing JSON";
        fs::write(directory.settings_path(), content).unwrap();
        let bundled_path = directory.bundled_settings(r#"{"sound_volume":37}"#);

        let settings = load_settings_from_path(&directory.settings_path(), || Some(bundled_path));

        assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            content
        );
    }

    #[test]
    fn read_errors_do_not_trigger_initialization() {
        let directory = TestDirectory::new();
        fs::create_dir(directory.settings_path()).unwrap();
        let resolved = std::cell::Cell::new(false);

        let settings = load_settings_from_path(&directory.settings_path(), || {
            resolved.set(true);
            None
        });

        assert!(!resolved.get());
        assert!(directory.settings_path().is_dir());
        assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
    }

    #[test]
    fn profile_created_during_initialization_is_not_overwritten() {
        let directory = TestDirectory::new();
        let bundled_path = directory.bundled_settings(r#"{"sound_volume":37}"#);
        let existing_content = r#"{"sound_volume":23}"#;

        let settings = load_settings_from_path(&directory.settings_path(), || {
            fs::write(directory.settings_path(), existing_content).unwrap();
            Some(bundled_path)
        });

        assert_eq!(settings.sound_volume, 23);
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            existing_content
        );
    }

    #[test]
    fn inaccessible_bundled_settings_do_not_create_a_profile() {
        let directory = TestDirectory::new();

        let settings = load_settings_from_path(&directory.settings_path(), || {
            Some(directory.0.join("missing-bundled-settings.json"))
        });

        assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
        assert!(!directory.settings_path().exists());
    }

    #[test]
    fn creation_failure_falls_back_without_copying_the_model() {
        let directory = TestDirectory::new();
        let bundled_path = directory.bundled_settings(r#"{"sound_volume":37}"#);
        let settings_path = directory.0.join("missing-directory/settings.json");

        let settings = load_settings_from_path(&settings_path, || Some(bundled_path));

        assert_eq!(settings.sound_volume, AppSettings::default().sound_volume);
        assert!(!settings_path.exists());
    }

    #[test]
    fn bundled_legacy_settings_use_existing_migrations() {
        let directory = TestDirectory::new();
        let content = r#"{"paste_method":"none","result_panel_mode":"invalid"}"#;
        let bundled_path = directory.bundled_settings(content);

        let settings = load_settings_from_path(&directory.settings_path(), || Some(bundled_path));

        assert_eq!(settings.paste_method, PasteMethod::CtrlV);
        assert!(!settings.auto_insert);
        assert_eq!(settings.result_panel_mode, "command");
        assert_eq!(
            fs::read_to_string(directory.settings_path()).unwrap(),
            content
        );
    }
}
