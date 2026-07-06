use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub const APP_NAME: &str = "ScreenWatchOCR";
pub const LEGACY_DATA_DIR_NAME: &str = "app_data";

#[derive(Debug, Clone, Default)]
pub struct DataDirEnv {
    pub local_app_data: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

pub fn user_data_dir() -> PathBuf {
    user_data_dir_from_env(DataDirEnv {
        local_app_data: std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
        xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
        home: home_dir(),
    })
}

pub fn user_data_dir_from_env(env: DataDirEnv) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = env
            .local_app_data
            .or_else(|| env.home.map(|home| home.join("AppData").join("Local")))
            .unwrap_or_else(|| PathBuf::from("."));
        return base.join(APP_NAME);
    }

    #[cfg(target_os = "macos")]
    {
        let base = env
            .home
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("Application Support");
        return base.join(APP_NAME);
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let base = env
            .xdg_data_home
            .or_else(|| env.home.map(|home| home.join(".local").join("share")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join(APP_NAME)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyDataMigrationResult {
    pub migrated: bool,
    pub copied_files: usize,
    pub copied_dirs: usize,
    pub skipped_existing_files: usize,
}

pub fn legacy_data_dir_from_app_root(app_root: impl AsRef<Path>) -> PathBuf {
    app_root.as_ref().join(LEGACY_DATA_DIR_NAME)
}

pub fn migrate_legacy_data_at(
    legacy_dir: impl AsRef<Path>,
    data_dir: impl AsRef<Path>,
) -> io::Result<LegacyDataMigrationResult> {
    let legacy_dir = legacy_dir.as_ref();
    let data_dir = data_dir.as_ref();
    if !legacy_dir.exists() {
        return Ok(LegacyDataMigrationResult::default());
    }
    if same_existing_path(legacy_dir, data_dir) {
        return Ok(LegacyDataMigrationResult::default());
    }

    fs::create_dir_all(data_dir)?;
    let mut result = LegacyDataMigrationResult {
        migrated: true,
        ..LegacyDataMigrationResult::default()
    };
    for name in ["profiles", "templates"] {
        copy_dir_contents_if_exists(&legacy_dir.join(name), &data_dir.join(name), &mut result)?;
    }
    for name in ["state.json", "alerts.jsonl"] {
        copy_file_if_exists(&legacy_dir.join(name), &data_dir.join(name), &mut result)?;
    }
    for name in ["alerts", "screenshots"] {
        copy_dir_contents_if_exists(
            &legacy_dir.join(name),
            &data_dir.join("screenshots"),
            &mut result,
        )?;
    }
    Ok(result)
}

fn same_existing_path(left: &Path, right: &Path) -> bool {
    let Ok(left) = left.canonicalize() else {
        return false;
    };
    let Ok(right) = right.canonicalize() else {
        return false;
    };
    left == right
}

fn copy_file_if_exists(
    source: &Path,
    destination: &Path,
    result: &mut LegacyDataMigrationResult,
) -> io::Result<()> {
    if !source.is_file() {
        return Ok(());
    }
    if destination.exists() {
        result.skipped_existing_files += 1;
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    result.copied_files += 1;
    Ok(())
}

fn copy_dir_contents_if_exists(
    source: &Path,
    destination: &Path,
    result: &mut LegacyDataMigrationResult,
) -> io::Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    result.copied_dirs += 1;
    copy_dir_contents(source, destination, result)
}

fn copy_dir_contents(
    source: &Path,
    destination: &Path,
    result: &mut LegacyDataMigrationResult,
) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&destination_path)?;
            result.copied_dirs += 1;
            copy_dir_contents(&source_path, &destination_path, result)?;
        } else if file_type.is_file() {
            copy_file_if_exists(&source_path, &destination_path, result)?;
        }
    }
    Ok(())
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{
        legacy_data_dir_from_app_root, migrate_legacy_data_at, user_data_dir_from_env, DataDirEnv,
    };
    use std::{fs, path::PathBuf};

    #[test]
    fn data_dir_uses_same_app_name_as_python() {
        let path = user_data_dir_from_env(DataDirEnv {
            local_app_data: Some(PathBuf::from(r"C:\Users\Wes\AppData\Local")),
            home: Some(PathBuf::from(r"C:\Users\Wes")),
            xdg_data_home: Some(PathBuf::from("/tmp/xdg")),
        });

        assert_eq!(path.file_name().unwrap(), "ScreenWatchOCR");
    }

    #[test]
    fn legacy_data_dir_matches_python_app_data_name() {
        assert_eq!(
            legacy_data_dir_from_app_root(PathBuf::from(r"C:\Tools\ScreenWatchOCR")),
            PathBuf::from(r"C:\Tools\ScreenWatchOCR\app_data")
        );
    }

    #[test]
    fn migrate_legacy_data_copies_python_layout_without_deleting_source() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        let data = tmp.path().join("data");
        fs::create_dir_all(legacy.join("profiles")).unwrap();
        fs::create_dir_all(legacy.join("templates").join("nested")).unwrap();
        fs::create_dir_all(legacy.join("alerts")).unwrap();
        fs::create_dir_all(legacy.join("screenshots")).unwrap();
        fs::write(legacy.join("profiles").join("profile_1.json"), b"profile").unwrap();
        fs::write(legacy.join("templates").join("target.png"), b"template").unwrap();
        fs::write(
            legacy.join("templates").join("nested").join("target2.png"),
            b"nested",
        )
        .unwrap();
        fs::write(legacy.join("state.json"), b"state").unwrap();
        fs::write(legacy.join("alerts.jsonl"), b"jsonl").unwrap();
        fs::write(legacy.join("alerts").join("old.png"), b"old").unwrap();
        fs::write(legacy.join("screenshots").join("new.png"), b"new").unwrap();

        let result = migrate_legacy_data_at(&legacy, &data).unwrap();

        assert!(result.migrated);
        assert_eq!(result.copied_files, 7);
        assert_eq!(result.skipped_existing_files, 0);
        assert!(result.copied_dirs >= 5);
        assert_eq!(
            fs::read(data.join("profiles").join("profile_1.json")).unwrap(),
            b"profile"
        );
        assert_eq!(
            fs::read(data.join("templates").join("nested").join("target2.png")).unwrap(),
            b"nested"
        );
        assert_eq!(fs::read(data.join("state.json")).unwrap(), b"state");
        assert_eq!(fs::read(data.join("alerts.jsonl")).unwrap(), b"jsonl");
        assert_eq!(
            fs::read(data.join("screenshots").join("old.png")).unwrap(),
            b"old"
        );
        assert_eq!(
            fs::read(data.join("screenshots").join("new.png")).unwrap(),
            b"new"
        );
        assert!(legacy.join("profiles").join("profile_1.json").exists());
        assert!(legacy.join("alerts").join("old.png").exists());
    }

    #[test]
    fn migrate_legacy_data_does_not_overwrite_existing_shared_files() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join("legacy");
        let data = tmp.path().join("data");
        fs::create_dir_all(legacy.join("profiles")).unwrap();
        fs::create_dir_all(legacy.join("templates").join("nested")).unwrap();
        fs::create_dir_all(legacy.join("alerts")).unwrap();
        fs::create_dir_all(data.join("profiles")).unwrap();
        fs::create_dir_all(data.join("templates").join("nested")).unwrap();
        fs::create_dir_all(data.join("screenshots")).unwrap();
        fs::write(legacy.join("profiles").join("profile_1.json"), b"legacy").unwrap();
        fs::write(legacy.join("profiles").join("profile_2.json"), b"new").unwrap();
        fs::write(
            legacy.join("templates").join("nested").join("target.png"),
            b"legacy-template",
        )
        .unwrap();
        fs::write(legacy.join("state.json"), b"legacy-state").unwrap();
        fs::write(legacy.join("alerts").join("hit.png"), b"legacy-alert").unwrap();
        fs::write(data.join("profiles").join("profile_1.json"), b"current").unwrap();
        fs::write(
            data.join("templates").join("nested").join("target.png"),
            b"current-template",
        )
        .unwrap();
        fs::write(data.join("state.json"), b"current-state").unwrap();
        fs::write(data.join("screenshots").join("hit.png"), b"current-alert").unwrap();

        let result = migrate_legacy_data_at(&legacy, &data).unwrap();

        assert!(result.migrated);
        assert_eq!(result.copied_files, 1);
        assert_eq!(result.skipped_existing_files, 4);
        assert_eq!(
            fs::read(data.join("profiles").join("profile_1.json")).unwrap(),
            b"current"
        );
        assert_eq!(
            fs::read(data.join("profiles").join("profile_2.json")).unwrap(),
            b"new"
        );
        assert_eq!(
            fs::read(data.join("templates").join("nested").join("target.png")).unwrap(),
            b"current-template"
        );
        assert_eq!(fs::read(data.join("state.json")).unwrap(), b"current-state");
        assert_eq!(
            fs::read(data.join("screenshots").join("hit.png")).unwrap(),
            b"current-alert"
        );
        assert_eq!(
            fs::read(legacy.join("profiles").join("profile_1.json")).unwrap(),
            b"legacy"
        );
    }

    #[test]
    fn migrate_legacy_data_skips_missing_or_same_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");
        let data = tmp.path().join("data");

        let missing_result = migrate_legacy_data_at(&missing, &data).unwrap();

        assert!(!missing_result.migrated);
        assert!(!data.exists());

        fs::create_dir_all(&data).unwrap();
        let same_result = migrate_legacy_data_at(&data, &data).unwrap();

        assert!(!same_result.migrated);
    }
}
