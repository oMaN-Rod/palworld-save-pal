//! Registering PalStudio as the OS handler for `nxm://` links: Windows via
//! `reg.exe` under HKCU, Linux via a `.desktop` file plus `xdg-mime`, macOS
//! unsupported.
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HandlerStatus {
    pub supported: bool,
    pub registered: bool,
    pub foreign: bool,
    pub current: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("opening nxm:// links is not supported on this platform yet")]
    UnsupportedPlatform,
    #[error("the PalStudio executable could not be located")]
    NoExecutable,
    #[error("registering the nxm:// handler failed: {0}")]
    Failed(String),
}

impl RegistryError {
    pub fn code(&self) -> &'static str {
        match self {
            RegistryError::UnsupportedPlatform => "unsupported_platform",
            RegistryError::NoExecutable => "no_executable",
            RegistryError::Failed(_) => "register_failed",
        }
    }
}

pub trait ProtocolRegistry: Send + Sync {
    fn status(&self) -> Result<HandlerStatus, RegistryError>;
    fn register(&self) -> Result<HandlerStatus, RegistryError>;
}

pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub type Runner = Box<dyn Fn(&str, &[String]) -> std::io::Result<CommandOutput> + Send + Sync>;

pub fn system_runner() -> Runner {
    Box::new(|program, args| {
        let mut command = std::process::Command::new(program);
        command.args(args).stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let output = command.output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    })
}

fn run(runner: &Runner, program: &str, args: Vec<String>) -> Result<CommandOutput, RegistryError> {
    runner(program, &args).map_err(|error| RegistryError::Failed(format!("{program}: {error}")))
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

pub const WINDOWS_KEY: &str = r"HKCU\Software\Classes\nxm";

pub fn windows_command(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// `reg query` separates the value name, type and data with four spaces.
pub fn parse_reg_default(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let mut parts = line.trim().splitn(3, "    ");
        let _name = parts.next()?;
        let kind = parts.next()?.trim();
        if kind != "REG_SZ" && kind != "REG_EXPAND_SZ" {
            return None;
        }
        Some(parts.next().unwrap_or_default().trim().to_string()).filter(|value| !value.is_empty())
    })
}

pub struct WindowsRegistry {
    exe: Option<PathBuf>,
    key: String,
    runner: Runner,
}

impl WindowsRegistry {
    pub fn new(exe: Option<PathBuf>, key: impl Into<String>, runner: Runner) -> Self {
        Self {
            exe,
            key: key.into(),
            runner,
        }
    }

    fn command_key(&self) -> String {
        format!(r"{}\shell\open\command", self.key)
    }
}

impl ProtocolRegistry for WindowsRegistry {
    fn status(&self) -> Result<HandlerStatus, RegistryError> {
        let output = run(
            &self.runner,
            "reg",
            vec!["query".to_string(), self.command_key(), "/ve".to_string()],
        )?;
        let current = if output.success {
            parse_reg_default(&output.stdout)
        } else {
            None
        };
        let ours = self.exe.as_deref().map(windows_command);
        let registered = matches!((&current, &ours), (Some(current), Some(ours)) if current.trim().eq_ignore_ascii_case(ours));
        Ok(HandlerStatus {
            supported: true,
            registered,
            foreign: current.is_some() && !registered,
            current,
        })
    }

    fn register(&self) -> Result<HandlerStatus, RegistryError> {
        let exe = self.exe.as_deref().ok_or(RegistryError::NoExecutable)?;
        let writes = vec![
            strings(&[
                "add",
                self.key.as_str(),
                "/ve",
                "/d",
                "URL:NXM Protocol",
                "/f",
            ]),
            strings(&[
                "add",
                self.key.as_str(),
                "/v",
                "URL Protocol",
                "/d",
                "",
                "/f",
            ]),
            vec![
                "add".to_string(),
                self.command_key(),
                "/ve".to_string(),
                "/d".to_string(),
                windows_command(exe),
                "/f".to_string(),
            ],
        ];
        for args in writes {
            let output = run(&self.runner, "reg", args)?;
            if !output.success {
                return Err(RegistryError::Failed(output.stderr.trim().to_string()));
            }
        }
        self.status()
    }
}

pub const LINUX_DESKTOP_FILE: &str = "palstudio-nxm.desktop";
const NXM_MIME: &str = "x-scheme-handler/nxm";

/// Escapes an executable path for a quoted Exec argument. The Desktop Entry
/// spec applies its general string escaping (where a literal `\` becomes
/// `\\`) before Exec's quoting rules run over the result (which escape each
/// of those backslashes again, plus `"`, `` ` `` and `$`) — so one literal
/// backslash in the path needs four backslashes in the file.
pub fn desktop_exec_quote(exe: &Path) -> String {
    let mut quoted = String::from("\"");
    for c in exe.to_string_lossy().chars() {
        match c {
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                quoted.push('\\');
                quoted.push(c);
            }
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

pub fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=PalStudio\nComment=Opens Nexus Mods downloads in PalStudio\nExec={} %u\nTerminal=false\nNoDisplay=true\nMimeType={NXM_MIME};\n",
        desktop_exec_quote(exe)
    )
}

pub struct LinuxRegistry {
    exe: Option<PathBuf>,
    applications_dir: Option<PathBuf>,
    runner: Runner,
}

impl LinuxRegistry {
    pub fn new(exe: Option<PathBuf>, applications_dir: Option<PathBuf>, runner: Runner) -> Self {
        Self {
            exe,
            applications_dir,
            runner,
        }
    }

    fn entry_matches(&self) -> bool {
        let (Some(exe), Some(dir)) = (self.exe.as_deref(), self.applications_dir.as_deref()) else {
            return false;
        };
        std::fs::read_to_string(dir.join(LINUX_DESKTOP_FILE))
            .is_ok_and(|text| text.contains(&format!("Exec={} %u", desktop_exec_quote(exe))))
    }
}

impl ProtocolRegistry for LinuxRegistry {
    fn status(&self) -> Result<HandlerStatus, RegistryError> {
        let output = run(
            &self.runner,
            "xdg-mime",
            strings(&["query", "default", NXM_MIME]),
        )?;
        let current = Some(output.stdout.trim().to_string())
            .filter(|current| output.success && !current.is_empty());
        let ours = current.as_deref() == Some(LINUX_DESKTOP_FILE);
        Ok(HandlerStatus {
            supported: true,
            registered: ours && self.entry_matches(),
            foreign: current.is_some() && !ours,
            current,
        })
    }

    fn register(&self) -> Result<HandlerStatus, RegistryError> {
        let exe = self.exe.as_deref().ok_or(RegistryError::NoExecutable)?;
        let dir = self
            .applications_dir
            .as_deref()
            .ok_or_else(|| RegistryError::Failed("no applications directory".to_string()))?;
        std::fs::create_dir_all(dir).map_err(|error| RegistryError::Failed(error.to_string()))?;
        std::fs::write(dir.join(LINUX_DESKTOP_FILE), desktop_entry(exe))
            .map_err(|error| RegistryError::Failed(error.to_string()))?;
        let output = run(
            &self.runner,
            "xdg-mime",
            strings(&["default", LINUX_DESKTOP_FILE, NXM_MIME]),
        )?;
        if !output.success {
            return Err(RegistryError::Failed(output.stderr.trim().to_string()));
        }
        let _ = run(
            &self.runner,
            "update-desktop-database",
            vec![dir.to_string_lossy().into_owned()],
        );
        self.status()
    }
}

pub struct UnsupportedRegistry;

impl ProtocolRegistry for UnsupportedRegistry {
    fn status(&self) -> Result<HandlerStatus, RegistryError> {
        Ok(HandlerStatus {
            supported: false,
            registered: false,
            foreign: false,
            current: None,
        })
    }

    fn register(&self) -> Result<HandlerStatus, RegistryError> {
        Err(RegistryError::UnsupportedPlatform)
    }
}

pub fn current_executable() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        if let Some(appimage) = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            return Some(appimage);
        }
    }
    std::env::current_exe().ok()
}

pub fn linux_applications_dir() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(data_home.join("applications"))
}

pub fn system_registry() -> Arc<dyn ProtocolRegistry> {
    if cfg!(windows) {
        Arc::new(WindowsRegistry::new(
            current_executable(),
            WINDOWS_KEY,
            system_runner(),
        ))
    } else if cfg!(target_os = "linux") {
        Arc::new(LinuxRegistry::new(
            current_executable(),
            linux_applications_dir(),
            system_runner(),
        ))
    } else {
        Arc::new(UnsupportedRegistry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;

    fn scripted(
        calls: Calls,
        respond: impl Fn(&str, &[String]) -> CommandOutput + Send + Sync + 'static,
    ) -> Runner {
        Box::new(move |program, args| {
            calls
                .lock()
                .unwrap()
                .push((program.to_string(), args.to_vec()));
            Ok(respond(program, args))
        })
    }

    fn ok(stdout: &str) -> CommandOutput {
        CommandOutput {
            success: true,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    fn failed(stderr: &str) -> CommandOutput {
        CommandOutput {
            success: false,
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    const VORTEX: &str = r#""C:\Program Files\Black Tree Gaming Ltd\Vortex\Vortex.exe" -d "%1""#;

    fn reg_output(value: &str) -> String {
        format!("\r\nHKEY_CURRENT_USER\\Software\\Classes\\nxm\\shell\\open\\command\r\n    (Default)    REG_SZ    {value}\r\n\r\n")
    }

    fn exe() -> PathBuf {
        PathBuf::from(r"C:\Program Files\PalStudio\palstudio.exe")
    }

    #[test]
    fn reg_query_output_yields_the_default_value() {
        assert_eq!(
            parse_reg_default(&reg_output(VORTEX)).as_deref(),
            Some(VORTEX)
        );
        assert_eq!(
            parse_reg_default("\r\nHKEY_CURRENT_USER\\x\r\n    (Default)    REG_SZ\r\n"),
            None
        );
        assert_eq!(parse_reg_default(""), None);
    }

    #[test]
    fn windows_status_tells_ours_from_a_foreign_handler_and_none() {
        let calls: Calls = Default::default();
        let none = WindowsRegistry::new(
            Some(exe()),
            WINDOWS_KEY,
            scripted(calls.clone(), |_, _| failed("not found")),
        );
        assert_eq!(
            none.status().unwrap(),
            HandlerStatus {
                supported: true,
                registered: false,
                foreign: false,
                current: None
            }
        );
        assert_eq!(
            calls.lock().unwrap()[0].1,
            vec![
                "query".to_string(),
                format!(r"{WINDOWS_KEY}\shell\open\command"),
                "/ve".to_string()
            ]
        );

        let foreign = WindowsRegistry::new(
            Some(exe()),
            WINDOWS_KEY,
            scripted(calls.clone(), |_, _| ok(&reg_output(VORTEX))),
        );
        let status = foreign.status().unwrap();
        assert!(status.foreign && !status.registered);
        assert_eq!(status.current.as_deref(), Some(VORTEX));

        let ours_text = windows_command(&exe()).to_ascii_uppercase();
        let ours = WindowsRegistry::new(
            Some(exe()),
            WINDOWS_KEY,
            scripted(calls, move |_, _| ok(&reg_output(&ours_text))),
        );
        let status = ours.status().unwrap();
        assert!(status.registered && !status.foreign);
    }

    #[test]
    fn windows_registration_writes_three_values_then_reads_back() {
        let calls: Calls = Default::default();
        let command = windows_command(&exe());
        let answer = command.clone();
        let registry = WindowsRegistry::new(
            Some(exe()),
            WINDOWS_KEY,
            scripted(calls.clone(), move |_, args| {
                if args[0] == "query" {
                    ok(&reg_output(&answer))
                } else {
                    ok("The operation completed successfully.")
                }
            }),
        );
        assert!(registry.register().unwrap().registered);
        let calls = calls.lock().unwrap();
        let strings = |items: &[&str]| {
            items
                .iter()
                .map(|item| item.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            calls[0],
            (
                "reg".to_string(),
                strings(&["add", WINDOWS_KEY, "/ve", "/d", "URL:NXM Protocol", "/f"])
            )
        );
        assert_eq!(
            calls[1],
            (
                "reg".to_string(),
                strings(&["add", WINDOWS_KEY, "/v", "URL Protocol", "/d", "", "/f"])
            )
        );
        assert_eq!(
            calls[2],
            (
                "reg".to_string(),
                vec![
                    "add".to_string(),
                    format!(r"{WINDOWS_KEY}\shell\open\command"),
                    "/ve".to_string(),
                    "/d".to_string(),
                    command.clone(),
                    "/f".to_string()
                ]
            )
        );
        assert_eq!(calls[3].1[0], "query");
        assert_eq!(
            command,
            r#""C:\Program Files\PalStudio\palstudio.exe" "%1""#
        );
    }

    #[test]
    fn windows_registration_failures_are_reported() {
        let calls: Calls = Default::default();
        let failing = WindowsRegistry::new(
            Some(exe()),
            WINDOWS_KEY,
            scripted(calls.clone(), |_, _| failed("ERROR: Access is denied.")),
        );
        let error = failing.register().unwrap_err();
        assert_eq!(error.code(), "register_failed");
        assert!(error.to_string().contains("Access is denied"));
        let no_exe = WindowsRegistry::new(None, WINDOWS_KEY, scripted(calls, |_, _| ok("")));
        assert_eq!(no_exe.register().unwrap_err().code(), "no_executable");
    }

    #[test]
    fn desktop_entries_quote_the_executable() {
        let entry = desktop_entry(Path::new("/opt/Pal Studio/palstudio"));
        assert!(
            entry.contains("Exec=\"/opt/Pal Studio/palstudio\" %u\n"),
            "{entry}"
        );
        assert!(entry.contains("MimeType=x-scheme-handler/nxm;\n"));
        assert!(entry.contains("NoDisplay=true\n"));
        assert_eq!(
            desktop_exec_quote(Path::new("/a/$b\"c`d\\e")),
            r#""/a/\$b\"c\`d\\\\e""#
        );
    }

    #[test]
    fn linux_registration_writes_the_entry_and_sets_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let applications = dir.path().join("applications");
        let calls: Calls = Default::default();
        let registered = Arc::new(Mutex::new(false));
        let flag = registered.clone();
        let registry = LinuxRegistry::new(
            Some(PathBuf::from("/opt/palstudio/palstudio")),
            Some(applications.clone()),
            scripted(calls.clone(), move |program, args| {
                match (program, args.first().map(String::as_str)) {
                    ("xdg-mime", Some("query")) => {
                        if *flag.lock().unwrap() {
                            ok("palstudio-nxm.desktop\n")
                        } else {
                            ok("vortex.desktop\n")
                        }
                    }
                    ("xdg-mime", Some("default")) => {
                        *flag.lock().unwrap() = true;
                        ok("")
                    }
                    _ => ok(""),
                }
            }),
        );

        let before = registry.status().unwrap();
        assert!(before.foreign && !before.registered);
        assert_eq!(before.current.as_deref(), Some("vortex.desktop"));

        let after = registry.register().unwrap();
        assert!(after.registered && !after.foreign, "{after:?}");
        let written = std::fs::read_to_string(applications.join(LINUX_DESKTOP_FILE)).unwrap();
        assert!(written.contains("Exec=\"/opt/palstudio/palstudio\" %u"));
        let calls = calls.lock().unwrap();
        assert!(calls.iter().any(|(program, args)| program == "xdg-mime"
            && args
                == &vec![
                    "default".to_string(),
                    LINUX_DESKTOP_FILE.to_string(),
                    "x-scheme-handler/nxm".to_string()
                ]));
    }

    #[test]
    fn linux_default_failures_and_unsupported_platforms_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let calls: Calls = Default::default();
        let registry = LinuxRegistry::new(
            Some(PathBuf::from("/opt/palstudio/palstudio")),
            Some(dir.path().to_path_buf()),
            scripted(calls, |program, args| {
                if program == "xdg-mime" && args[0] == "default" {
                    failed("xdg-mime: no method")
                } else {
                    ok("")
                }
            }),
        );
        assert_eq!(registry.register().unwrap_err().code(), "register_failed");

        let unsupported = UnsupportedRegistry;
        assert!(!unsupported.status().unwrap().supported);
        assert_eq!(
            unsupported.register().unwrap_err().code(),
            "unsupported_platform"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "writes and deletes HKCU\\Software\\PalStudioTests"]
    fn registers_under_a_throwaway_windows_key() {
        const PARENT: &str = r"HKCU\Software\PalStudioTests";
        let key = format!(r"{PARENT}\nxm");
        let registry = WindowsRegistry::new(Some(exe()), key, system_runner());
        let result = registry.register();
        let _ = (system_runner())(
            "reg",
            &["delete".to_string(), PARENT.to_string(), "/f".to_string()],
        );
        let status = result.unwrap();
        assert!(status.registered, "{status:?}");
        assert_eq!(
            status.current.as_deref(),
            Some(windows_command(&exe()).as_str())
        );
    }
}
