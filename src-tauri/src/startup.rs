use serde::Serialize;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const STARTUP_LINK_NAME: &str = "屏幕监控OCR Tauri.lnk";
pub const START_MINIMIZED_ARG: &str = "--start-minimized";
const POWERSHELL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartupStatus {
    pub supported: bool,
    pub enabled: bool,
    pub link_path: String,
    pub target_path: String,
    pub arguments: String,
    pub working_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShortcutInfo {
    target: PathBuf,
    arguments: String,
    working_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StartupManager {
    link_path: PathBuf,
    target_path: PathBuf,
    arguments: String,
    working_dir: PathBuf,
}

impl StartupManager {
    pub fn current() -> Result<Option<Self>, String> {
        if !cfg!(windows) {
            return Ok(None);
        }
        let appdata = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "APPDATA is not set".to_string())?;
        let target_path = std::env::current_exe().map_err(|err| err.to_string())?;
        Ok(Some(Self::new(
            startup_link_path_from_appdata(appdata),
            target_path,
            true,
            None,
        )))
    }

    fn new(
        link_path: PathBuf,
        target_path: PathBuf,
        packaged: bool,
        app_dir: Option<PathBuf>,
    ) -> Self {
        let arguments = startup_arguments_for_target(&target_path, &target_path, packaged);
        let working_dir = startup_working_dir_for_target(
            &target_path,
            &target_path,
            packaged,
            app_dir.as_deref(),
        );
        Self {
            link_path,
            target_path,
            arguments,
            working_dir,
        }
    }

    pub fn status(&self) -> Result<StartupStatus, String> {
        Ok(self.status_from_info(read_shortcut_info(&self.link_path)?))
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<StartupStatus, String> {
        if enabled {
            if let Some(parent) = self.link_path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            write_shortcut(
                &self.link_path,
                &self.target_path,
                &self.arguments,
                &self.working_dir,
            )?;
        } else if self.is_enabled_from_info(read_shortcut_info(&self.link_path)?) {
            fs::remove_file(&self.link_path).map_err(|err| err.to_string())?;
        }
        self.status()
    }

    fn status_from_info(&self, info: Option<ShortcutInfo>) -> StartupStatus {
        StartupStatus {
            supported: true,
            enabled: self.is_enabled_from_info(info),
            link_path: self.link_path.display().to_string(),
            target_path: self.target_path.display().to_string(),
            arguments: self.arguments.clone(),
            working_dir: self.working_dir.display().to_string(),
        }
    }

    fn is_enabled_from_info(&self, info: Option<ShortcutInfo>) -> bool {
        info.map(|item| same_path(&item.target, &self.target_path))
            .unwrap_or(false)
    }
}

pub fn unsupported_status() -> StartupStatus {
    StartupStatus {
        supported: false,
        enabled: false,
        link_path: String::new(),
        target_path: String::new(),
        arguments: START_MINIMIZED_ARG.to_string(),
        working_dir: String::new(),
    }
}

pub fn startup_link_path_from_appdata(appdata: impl AsRef<Path>) -> PathBuf {
    appdata
        .as_ref()
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(STARTUP_LINK_NAME)
}

pub fn startup_arguments_for_target(
    target: impl AsRef<Path>,
    current_exe: impl AsRef<Path>,
    packaged: bool,
) -> String {
    if packaged {
        return START_MINIMIZED_ARG.to_string();
    }
    if same_path(target.as_ref(), current_exe.as_ref()) {
        return "-m screen_watch app --start-minimized".to_string();
    }
    START_MINIMIZED_ARG.to_string()
}

pub fn startup_working_dir_for_target(
    target: impl AsRef<Path>,
    current_exe: impl AsRef<Path>,
    packaged: bool,
    app_dir: Option<&Path>,
) -> PathBuf {
    let target = target.as_ref();
    if !packaged && same_path(target, current_exe.as_ref()) {
        return app_dir.map(Path::to_path_buf).unwrap_or_else(|| {
            current_exe
                .as_ref()
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf()
        });
    }
    target
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf()
}

fn read_shortcut_info(path: &Path) -> Result<Option<ShortcutInfo>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let script = format!(
        "$s=New-Object -ComObject WScript.Shell; \
         $l=$s.CreateShortcut({}); \
         Write-Output $l.TargetPath; \
         Write-Output $l.Arguments; \
         Write-Output $l.WorkingDirectory",
        ps_quote(path)
    );
    let output = run_powershell(&script)?;
    let lines = output.lines().collect::<Vec<_>>();
    let target = lines.first().map(|line| line.trim()).unwrap_or_default();
    if target.is_empty() {
        return Ok(None);
    }
    Ok(Some(ShortcutInfo {
        target: PathBuf::from(target),
        arguments: lines
            .get(1)
            .map(|line| line.trim())
            .unwrap_or_default()
            .to_string(),
        working_dir: PathBuf::from(lines.get(2).map(|line| line.trim()).unwrap_or_default()),
    }))
}

fn write_shortcut(
    link_path: &Path,
    target_path: &Path,
    arguments: &str,
    working_dir: &Path,
) -> Result<(), String> {
    let script = format!(
        "$s=New-Object -ComObject WScript.Shell; \
         $l=$s.CreateShortcut({}); \
         $l.TargetPath={}; \
         $l.Arguments={}; \
         $l.WorkingDirectory={}; \
         $l.Save()",
        ps_quote(link_path),
        ps_quote(target_path),
        ps_quote(arguments),
        ps_quote(working_dir)
    );
    run_powershell(&script).map(|_| ())
}

fn run_powershell(script: &str) -> Result<String, String> {
    let mut command = Command::new("powershell.exe");
    command
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_no_window_flag(&mut command);
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < POWERSHELL_TIMEOUT => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("PowerShell shortcut command timed out".to_string());
            }
            Err(err) => return Err(err.to_string()),
        }
    }
    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "PowerShell shortcut command failed".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(windows)]
fn apply_no_window_flag(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn apply_no_window_flag(_command: &mut Command) {}

fn ps_quote<T: PathOrStr + ?Sized>(value: &T) -> String {
    let value = value.as_os_string();
    format!("'{}'", value.to_string_lossy().replace('\'', "''"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = normalize_path(left);
    let right = normalize_path(right);
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

trait PathOrStr {
    fn as_os_string(&self) -> OsString;
}

impl PathOrStr for Path {
    fn as_os_string(&self) -> OsString {
        self.as_os_str().to_os_string()
    }
}

impl PathOrStr for PathBuf {
    fn as_os_string(&self) -> OsString {
        self.as_os_str().to_os_string()
    }
}

impl PathOrStr for str {
    fn as_os_string(&self) -> OsString {
        OsString::from(self)
    }
}

impl PathOrStr for String {
    fn as_os_string(&self) -> OsString {
        OsString::from(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ps_quote, read_shortcut_info, same_path, startup_arguments_for_target,
        startup_link_path_from_appdata, startup_working_dir_for_target, unsupported_status,
        ShortcutInfo, StartupManager, STARTUP_LINK_NAME, START_MINIMIZED_ARG,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn startup_link_path_uses_tauri_startup_folder_and_name() {
        let path = startup_link_path_from_appdata(Path::new(r"C:\Users\me\AppData\Roaming"));
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            STARTUP_LINK_NAME
        );
        assert!(path.to_string_lossy().contains("Microsoft"));
        assert!(path.to_string_lossy().contains("Startup"));
    }

    #[test]
    fn startup_arguments_match_tauri_packaged_and_dev_paths() {
        assert_eq!(
            startup_arguments_for_target("screen-watch-ocr-tauri.exe", "python.exe", true),
            START_MINIMIZED_ARG
        );
        assert_eq!(
            startup_arguments_for_target("python.exe", "python.exe", false),
            "-m screen_watch app --start-minimized"
        );
        assert_eq!(
            startup_arguments_for_target("screen-watch-ocr-tauri.vbs", "python.exe", false),
            START_MINIMIZED_ARG
        );
    }

    #[test]
    fn startup_working_dir_matches_python_packaged_and_dev_paths() {
        assert_eq!(
            startup_working_dir_for_target(
                Path::new(r"C:\Apps\screen-watch-ocr-tauri.exe"),
                Path::new(r"C:\Python\python.exe"),
                true,
                None,
            ),
            PathBuf::from(r"C:\Apps")
        );
        assert_eq!(
            startup_working_dir_for_target(
                Path::new(r"C:\Python\python.exe"),
                Path::new(r"C:\Python\python.exe"),
                false,
                Some(Path::new(r"C:\Project\screen-watch-ocr")),
            ),
            PathBuf::from(r"C:\Project\screen-watch-ocr")
        );
    }

    #[test]
    fn powershell_quote_escapes_single_quotes() {
        assert_eq!(ps_quote("C:\\O'Hara\\app.exe"), "'C:\\O''Hara\\app.exe'");
    }

    #[test]
    fn startup_status_only_enables_matching_shortcut_target() {
        let manager = StartupManager::new(
            PathBuf::from(r"C:\Startup\屏幕监控OCR Tauri.lnk"),
            PathBuf::from(r"C:\Apps\screen-watch-ocr-tauri.exe"),
            true,
            None,
        );
        assert!(
            manager
                .status_from_info(Some(ShortcutInfo {
                    target: PathBuf::from(r"C:\Apps\screen-watch-ocr-tauri.exe"),
                    arguments: START_MINIMIZED_ARG.to_string(),
                    working_dir: PathBuf::from(r"C:\Apps"),
                }))
                .enabled
        );
        assert!(
            !manager
                .status_from_info(Some(ShortcutInfo {
                    target: PathBuf::from(r"C:\Other\App.exe"),
                    arguments: START_MINIMIZED_ARG.to_string(),
                    working_dir: PathBuf::from(r"C:\Other"),
                }))
                .enabled
        );
    }

    #[test]
    fn unsupported_status_is_disabled_but_reports_expected_argument() {
        let status = unsupported_status();
        assert!(!status.supported);
        assert!(!status.enabled);
        assert_eq!(status.arguments, START_MINIMIZED_ARG);
    }

    #[cfg(windows)]
    #[test]
    fn startup_manager_writes_reads_and_removes_isolated_shortcut() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("screen-watch-ocr-tauri-startup-{stamp}"));
        let startup_dir = root.join("Microsoft/Windows/Start Menu/Programs/Startup");
        let link_path = startup_dir.join(STARTUP_LINK_NAME);
        let target_path = std::env::current_exe().unwrap();
        let manager = StartupManager::new(link_path.clone(), target_path.clone(), true, None);

        let initial = manager.status().unwrap();
        assert!(initial.supported);
        assert!(!initial.enabled);
        assert_eq!(initial.arguments, START_MINIMIZED_ARG);
        assert_eq!(PathBuf::from(&initial.link_path), link_path);

        let enabled = manager.set_enabled(true).unwrap();
        assert!(enabled.enabled);
        assert!(link_path.exists());
        let info = read_shortcut_info(&link_path).unwrap().unwrap();
        assert!(same_path(&info.target, &target_path));
        assert_eq!(info.arguments, START_MINIMIZED_ARG);
        assert_eq!(info.working_dir, target_path.parent().unwrap());

        let disabled = manager.set_enabled(false).unwrap();
        assert!(!disabled.enabled);
        assert!(!link_path.exists());

        let _ = fs::remove_dir_all(root);
    }
}
