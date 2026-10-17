// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::env;
use std::path::{Path, PathBuf};

/// External package manager that owns the ppm executable and is responsible for updating it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalManager {
    /// Manager name, `None` when the executable sits in a system directory owned by an unknown manager.
    pub name: Option<String>,
    /// Package that ships the executable, when the manager reports it.
    pub package: Option<String>,
    /// Command that updates ppm through the manager.
    pub update_command: Option<String>,
}

impl ExternalManager {
    fn new(name: &str, update_command: Option<&str>) -> Self {
        Self {
            name: Some(name.to_string()),
            package: None,
            update_command: update_command.map(str::to_string),
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn with_package(mut self, package: String) -> Self {
        self.package = Some(package);
        self
    }

    /// Human readable name, e.g. "pacman (package `ppm-bin`)".
    pub fn describe(&self) -> String {
        match (&self.name, &self.package) {
            (Some(name), Some(package)) => format!("{name} (package `{package}`)"),
            (Some(name), None) => name.clone(),
            (None, _) => "a system package manager".to_string(),
        }
    }

    /// Sentence telling the user how to update ppm through the manager.
    pub fn update_hint(&self) -> String {
        match &self.update_command {
            Some(command) => format!("Update it with `{command}` instead."),
            None => "Update it through your package manager instead.".to_string(),
        }
    }
}

/// Detects whether `exe` was installed by an external package manager rather than by ppm itself.
pub fn detect(exe: &Path) -> Option<ExternalManager> {
    let path = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let path_str = path.to_string_lossy().replace('\\', "/");
    let cargo_bin = cargo_bin_dir().map(|p| p.to_string_lossy().replace('\\', "/"));

    if let Some(manager) = detect_by_path(&path_str, cargo_bin.as_deref()) {
        return Some(manager);
    }

    let in_home = env::var_os("HOME").is_some_and(|home| path.starts_with(home));
    if in_home {
        return None;
    }

    #[cfg(target_os = "linux")]
    if let Some(manager) = query_system_owner(&path_str) {
        return Some(manager);
    }

    is_system_dir(&path_str).then_some(ExternalManager {
        name: None,
        package: None,
        update_command: None,
    })
}

fn cargo_bin_dir() -> Option<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))?;
    Some(cargo_home.join("bin"))
}

/// Recognizes managers that install into well-known, manager-specific directories.
fn detect_by_path(path: &str, cargo_bin: Option<&str>) -> Option<ExternalManager> {
    let lower = path.to_ascii_lowercase();

    if lower.starts_with("/nix/store/") {
        Some(ExternalManager::new("Nix", None))
    } else if lower.starts_with("/snap/") {
        Some(ExternalManager::new("Snap", Some("sudo snap refresh ppm")))
    } else if lower.contains("/flatpak/") {
        Some(ExternalManager::new("Flatpak", Some("flatpak update")))
    } else if lower.contains("/cellar/")
        || lower.starts_with("/opt/homebrew/")
        || lower.contains("/.linuxbrew/")
    {
        Some(ExternalManager::new("Homebrew", Some("brew upgrade ppm")))
    } else if lower.contains("/.cargo/bin/")
        || cargo_bin.is_some_and(|bin| path.starts_with(&format!("{bin}/")))
    {
        Some(ExternalManager::new(
            "cargo install",
            Some("cargo install --git https://github.com/Pumpkin-MC/ppm --force"),
        ))
    } else if lower.contains("/scoop/apps/") {
        Some(ExternalManager::new("Scoop", Some("scoop update ppm")))
    } else if lower.contains("/chocolatey/") {
        Some(ExternalManager::new(
            "Chocolatey",
            Some("choco upgrade ppm"),
        ))
    } else if lower.contains("/winget/packages/") {
        Some(ExternalManager::new("winget", Some("winget upgrade ppm")))
    } else {
        None
    }
}

/// System directories where only a package manager is expected to install binaries.
fn is_system_dir(path: &str) -> bool {
    (path.starts_with("/usr/") && !path.starts_with("/usr/local/"))
        || path.starts_with("/bin/")
        || path.starts_with("/sbin/")
}

/// Asks the distro package managers whether one of them owns `path`.
#[cfg(target_os = "linux")]
fn query_system_owner(path: &str) -> Option<ExternalManager> {
    use std::process::Command;

    let owner = |program: &str, args: &[&str]| -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    };

    if let Some(package) = owner("pacman", &["-Qqo", path]).and_then(|out| first_line(&out)) {
        return Some(
            ExternalManager::new("pacman", Some("sudo pacman -Syu")).with_package(package),
        );
    }
    if let Some(package) = owner("dpkg", &["-S", path]).and_then(|out| parse_dpkg_owner(&out)) {
        let command = format!("sudo apt update && sudo apt install --only-upgrade {package}");
        return Some(ExternalManager::new("apt", Some(&command)).with_package(package));
    }
    if let Some(package) =
        owner("rpm", &["-qf", "--qf", "%{NAME}\n", path]).and_then(|out| first_line(&out))
    {
        return Some(ExternalManager::new("rpm", None).with_package(package));
    }
    if let Some(package) =
        owner("apk", &["info", "--who-owns", path]).and_then(|out| parse_apk_owner(&out))
    {
        return Some(ExternalManager::new("apk", Some("sudo apk upgrade")).with_package(package));
    }

    None
}

#[cfg(any(target_os = "linux", test))]
fn first_line(output: &str) -> Option<String> {
    output
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}

/// Parses `dpkg -S` output, e.g. `ppm: /usr/bin/ppm`.
#[cfg(any(target_os = "linux", test))]
fn parse_dpkg_owner(output: &str) -> Option<String> {
    let line = first_line(output)?;
    let (package, _) = line.split_once(": ")?;
    Some(package.trim().to_string())
}

/// Parses `apk info --who-owns` output, e.g. `/usr/bin/ppm is owned by ppm-0.1.1-r0`.
#[cfg(any(target_os = "linux", test))]
fn parse_apk_owner(output: &str) -> Option<String> {
    let line = first_line(output)?;
    let (_, package) = line.split_once(" is owned by ")?;
    Some(package.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_of(path: &str, cargo_bin: Option<&str>) -> Option<String> {
        detect_by_path(path, cargo_bin).and_then(|m| m.name)
    }

    #[test]
    fn detects_managers_by_path() {
        assert_eq!(
            name_of("/opt/homebrew/Cellar/ppm/0.1.1/bin/ppm", None).as_deref(),
            Some("Homebrew")
        );
        assert_eq!(
            name_of("/usr/local/Cellar/ppm/0.1.1/bin/ppm", None).as_deref(),
            Some("Homebrew")
        );
        assert_eq!(
            name_of("/nix/store/abc-ppm-0.1.1/bin/ppm", None).as_deref(),
            Some("Nix")
        );
        assert_eq!(
            name_of("/home/user/.cargo/bin/ppm", None).as_deref(),
            Some("cargo install")
        );
        assert_eq!(
            name_of("/opt/rust/cargo/bin/ppm", Some("/opt/rust/cargo/bin")).as_deref(),
            Some("cargo install")
        );
        assert_eq!(
            name_of("C:/Users/me/scoop/apps/ppm/current/ppm.exe", None).as_deref(),
            Some("Scoop")
        );
        assert_eq!(
            name_of(
                "C:/Users/me/AppData/Local/Microsoft/WinGet/Packages/ppm/ppm.exe",
                None
            )
            .as_deref(),
            Some("winget")
        );
    }

    #[test]
    fn standalone_paths_are_not_managed() {
        assert!(detect_by_path("/home/user/.local/bin/ppm", None).is_none());
        assert!(detect_by_path("/usr/local/bin/ppm", None).is_none());
        assert!(detect_by_path("C:/Users/me/AppData/Local/ppm/bin/ppm.exe", None).is_none());
    }

    #[test]
    fn system_dirs() {
        assert!(is_system_dir("/usr/bin/ppm"));
        assert!(is_system_dir("/bin/ppm"));
        assert!(!is_system_dir("/usr/local/bin/ppm"));
        assert!(!is_system_dir("/opt/ppm/ppm"));
    }

    #[test]
    fn parses_owner_output() {
        assert_eq!(
            parse_dpkg_owner("ppm: /usr/bin/ppm\n").as_deref(),
            Some("ppm")
        );
        assert_eq!(
            parse_apk_owner("/usr/bin/ppm is owned by ppm-0.1.1-r0\n").as_deref(),
            Some("ppm-0.1.1-r0")
        );
        assert_eq!(first_line("ppm-bin\n").as_deref(), Some("ppm-bin"));
        assert_eq!(first_line("\n"), None);
    }

    #[test]
    fn describes_manager() {
        let pacman = ExternalManager::new("pacman", Some("sudo pacman -Syu"))
            .with_package("ppm-bin".to_string());
        assert_eq!(pacman.describe(), "pacman (package `ppm-bin`)");
        assert_eq!(
            pacman.update_hint(),
            "Update it with `sudo pacman -Syu` instead."
        );

        let unknown = ExternalManager {
            name: None,
            package: None,
            update_command: None,
        };
        assert_eq!(unknown.describe(), "a system package manager");
    }
}
