use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_store::{Store, StoreExt};

// Initialize and open together: the store plugin caches the first file it reads.
static STORE_INITIALIZATION: Mutex<()> = Mutex::new(());

pub fn store_with_defaults(
    app: &AppHandle,
    store_file: &str,
    resource: &str,
    validate: impl FnOnce(&serde_json::Value) -> Result<(), String>,
) -> Result<Arc<Store<Wry>>, String> {
    let _guard = STORE_INITIALIZATION
        .lock()
        .map_err(|_| "Failed to lock default store initialization".to_string())?;
    if let Some(store) = app.get_store(store_file) {
        return Ok(store);
    }

    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join(store_file);
    if let Err(error) = initialize_file_from_resource(
        &path,
        || super::resources::resolve_resource_path(app, resource),
        validate,
    ) {
        // Keep native store behavior when a bundled model is missing or invalid.
        log::error!("Failed to initialize {store_file} from bundled defaults: {error}");
    }

    app.store(store_file).map_err(|error| error.to_string())
}

fn initialize_file_from_resource(
    path: &Path,
    bundled_resource: impl FnOnce() -> Option<PathBuf>,
    validate: impl FnOnce(&serde_json::Value) -> Result<(), String>,
) -> Result<(), String> {
    // Even empty, corrupt or unreadable user files belong to the user. A dangling
    // symlink also counts as an existing file and must not be replaced.
    match fs::symlink_metadata(path) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Failed to inspect user file: {error}")),
    }

    let Some(resource_path) = bundled_resource() else {
        return Ok(());
    };
    let content = fs::read_to_string(resource_path)
        .map_err(|error| format!("Failed to read bundled defaults: {error}"))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|_| "Bundled defaults contain invalid JSON".to_string())?;
    if !value.is_object() {
        return Err("Bundled defaults must be a JSON object".to_string());
    }
    validate(&value)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create user data directory: {error}"))?;
    }
    match write_new_file(path, &content) {
        Ok(()) => Ok(()),
        // A profile may have appeared after our first check. Never overwrite it.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(format!("Failed to create user file: {error}")),
    }
}

pub(crate) fn write_new_file(path: &Path, content: &str) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(error) = file.write_all(content.as_bytes()) {
        drop(file);
        if let Err(cleanup_error) = fs::remove_file(path) {
            log::error!("Failed to remove incomplete user file: {cleanup_error}");
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("murmure-defaults-tests-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn bundled_file(&self, content: &str) -> PathBuf {
            let path = self.0.join("default-file.json");
            fs::write(&path, content).unwrap();
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_user_file_is_copied_without_modifying_the_model() {
        let directory = TestDirectory::new();
        let content = "{\n  \"synthetic-term\": []\n}\n";
        let resource = directory.bundled_file(content);
        let path = directory.0.join("profile/dictionary.json");

        initialize_file_from_resource(&path, || Some(resource.clone()), |_| Ok(())).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), content);
        assert_eq!(fs::read_to_string(resource).unwrap(), content);
    }

    #[test]
    fn existing_user_files_are_preserved_without_resolving_the_model() {
        for content in ["{}", "", "invalid JSON", r#"{"personal-term":[]}"#] {
            let directory = TestDirectory::new();
            let path = directory.0.join("dictionary.json");
            fs::write(&path, content).unwrap();

            initialize_file_from_resource(
                &path,
                || panic!("Existing user files must not load the model"),
                |_| panic!("Existing user files must not be validated as models"),
            )
            .unwrap();

            assert_eq!(fs::read_to_string(path).unwrap(), content);
        }
    }

    #[test]
    fn only_the_missing_file_of_an_existing_profile_is_initialized() {
        let directory = TestDirectory::new();
        let settings = directory.0.join("settings.json");
        let dictionary = directory.0.join("dictionary.json");
        let rules = directory.0.join("formatting_rules.json");
        fs::write(&settings, r#"{"language":"fr"}"#).unwrap();
        fs::write(&rules, r#"{"settings":{"built_in":{},"rules":[]}}"#).unwrap();
        let resource = directory.bundled_file(r#"{"synthetic-term":[]}"#);

        initialize_file_from_resource(&dictionary, || Some(resource), |_| Ok(())).unwrap();

        assert_eq!(
            fs::read_to_string(settings).unwrap(),
            r#"{"language":"fr"}"#
        );
        assert_eq!(
            fs::read_to_string(rules).unwrap(),
            r#"{"settings":{"built_in":{},"rules":[]}}"#
        );
        assert!(dictionary.is_file());
    }

    #[test]
    fn later_launch_preserves_user_edits_and_does_not_restore_deleted_entries() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");
        let resource = directory.bundled_file(r#"{"synthetic-term":[]}"#);
        initialize_file_from_resource(&path, || Some(resource.clone()), |_| Ok(())).unwrap();
        fs::write(&path, "{}").unwrap();
        fs::write(&resource, r#"{"new-term":[]}"#).unwrap();

        initialize_file_from_resource(&path, || Some(resource), |_| Ok(())).unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "{}");
    }

    #[test]
    fn missing_model_leaves_native_store_initialization_available() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");

        initialize_file_from_resource(&path, || None, |_| Ok(())).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn invalid_models_are_not_copied() {
        for content in ["{", "null", "[]"] {
            let directory = TestDirectory::new();
            let path = directory.0.join("dictionary.json");
            let resource = directory.bundled_file(content);

            assert!(initialize_file_from_resource(&path, || Some(resource), |_| Ok(())).is_err());
            assert!(!path.exists());
        }
    }

    #[test]
    fn rejected_model_is_not_copied() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");
        let resource = directory.bundled_file("{}");

        let result = initialize_file_from_resource(
            &path,
            || Some(resource),
            |_| Err("Invalid model schema".to_string()),
        );

        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn concurrent_creation_never_overwrites_the_existing_user_file() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");
        let resource = directory.bundled_file(r#"{"synthetic-term":[]}"#);

        initialize_file_from_resource(
            &path,
            || {
                fs::write(&path, "{}").unwrap();
                Some(resource)
            },
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(fs::read_to_string(path).unwrap(), "{}");
    }

    #[test]
    fn existing_directory_is_preserved() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");
        fs::create_dir(&path).unwrap();

        initialize_file_from_resource(&path, || panic!("Must preserve user path"), |_| Ok(()))
            .unwrap();

        assert!(path.is_dir());
    }

    #[test]
    fn unreadable_model_does_not_create_a_user_file() {
        let directory = TestDirectory::new();
        let path = directory.0.join("dictionary.json");

        let result = initialize_file_from_resource(
            &path,
            || Some(directory.0.join("missing-default-file.json")),
            |_| Ok(()),
        );

        assert!(result.is_err());
        assert!(!path.exists());
    }
}
