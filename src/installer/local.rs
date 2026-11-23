// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::tr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    #[serde(skip)]
    pub modified: Option<SystemTime>,
    pub is_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_id: Option<String>,
    #[serde(default)]
    pub is_managed: bool,
}

impl InstalledPlugin {
    pub fn formatted_size(&self) -> String {
        format_size(self.size_bytes)
    }

    pub fn formatted_modified(&self) -> String {
        self.modified
            .map(format_system_time)
            .unwrap_or_else(|| "-".to_string())
    }

    pub fn display_name(&self) -> &str {
        self.filename
            .strip_suffix(".wasm.deactivated")
            .or_else(|| self.filename.strip_suffix(".wasm"))
            .or_else(|| self.filename.strip_suffix(".deactivated"))
            .unwrap_or(&self.filename)
    }
}

fn format_system_time(time: SystemTime) -> String {
    if let Ok(duration) = SystemTime::now().duration_since(time) {
        let secs = duration.as_secs();
        if secs < 60 {
            tr!("time.just_now")
        } else if secs < 3600 {
            tr!("time.minutes_ago", n = secs / 60)
        } else if secs < 86400 {
            tr!("time.hours_ago", n = secs / 3600)
        } else {
            tr!("time.days_ago", n = secs / 86400)
        }
    } else {
        "-".to_string()
    }
}

/// Scan a directory for installed `.wasm` or deactivated plugins.
pub fn scan_installed(plugins_dir: &Path) -> Result<Vec<InstalledPlugin>> {
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    let lockfile = super::lockfile::Lockfile::load_from_dir(plugins_dir).ok();
    let mut list = Vec::new();
    let entries = fs::read_dir(plugins_dir)
        .with_context(|| tr!("local.read_dir_failed", path = plugins_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let filename = entry.file_name().to_string_lossy().to_string();
        let is_wasm = filename.ends_with(".wasm");
        let is_deactivated =
            filename.ends_with(".wasm.deactivated") || filename.ends_with(".deactivated");

        if is_wasm || is_deactivated {
            let metadata = entry.metadata().ok();
            let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = metadata.and_then(|m| m.modified().ok());

            let (version, author, public_id, is_managed) = if let Some(ref lock) = lockfile
                && let Some(locked) = lock.find_plugin(&filename)
            {
                (
                    locked.version.clone(),
                    Some(locked.author.clone()),
                    Some(locked.public_id.clone()),
                    true,
                )
            } else {
                (None, None, None, false)
            };

            list.push(InstalledPlugin {
                filename,
                path,
                size_bytes,
                modified,
                is_active: is_wasm,
                version,
                author,
                public_id,
                is_managed,
            });
        }
    }

    list.sort_by_key(|a| a.filename.to_lowercase());
    Ok(list)
}

/// Remove a plugin file from the plugins directory.
pub fn remove_plugin(plugins_dir: &Path, plugin: &str) -> Result<PathBuf> {
    if !plugins_dir.exists() {
        bail!(
            "{}",
            tr!("local.dir_not_found", path = plugins_dir.display())
        );
    }

    let trimmed = plugin.trim();
    // Guard against path traversal (`../...`, `/etc/...`) escaping `plugins_dir`.
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed.contains(['/', '\\']) {
        bail!(
            "{}",
            tr!(
                "local.plugin_not_found",
                plugin = trimmed,
                dir = plugins_dir.display()
            )
        );
    }

    let direct_path = plugins_dir.join(trimmed);
    let wasm_path = plugins_dir.join(format!("{trimmed}.wasm"));
    let deact_path = plugins_dir.join(format!("{trimmed}.wasm.deactivated"));

    let target_path = if direct_path.exists() && direct_path.is_file() {
        direct_path
    } else if wasm_path.exists() && wasm_path.is_file() {
        wasm_path
    } else if deact_path.exists() && deact_path.is_file() {
        deact_path
    } else {
        // Try case-insensitive lookup by filename, display name, or marketplace public_id
        let installed = scan_installed(plugins_dir)?;
        let found = installed.into_iter().find(|p| {
            p.filename.eq_ignore_ascii_case(trimmed)
                || p.display_name().eq_ignore_ascii_case(trimmed)
                || p.public_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(trimmed))
        });

        match found {
            Some(p) => p.path,
            None => {
                bail!(
                    "{}",
                    tr!(
                        "local.plugin_not_found",
                        plugin = trimmed,
                        dir = plugins_dir.display()
                    )
                );
            }
        }
    };

    fs::remove_file(&target_path)
        .with_context(|| tr!("local.remove_failed", path = target_path.display()))?;

    // Also clean up entry from .ppm.lock if present
    if let Ok(mut lockfile) = super::lockfile::Lockfile::load_from_dir(plugins_dir) {
        let filename = target_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        if lockfile.remove_plugin(trimmed).is_some() || lockfile.remove_plugin(&filename).is_some()
        {
            let _ = lockfile.save_to_dir(plugins_dir);
        }
    }

    Ok(target_path)
}

/// Format bytes into human-readable size (KB, MB, GB).
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_display_name() {
        let p1 = InstalledPlugin {
            filename: "MyPlugin.wasm".into(),
            path: PathBuf::from("plugins/MyPlugin.wasm"),
            size_bytes: 100,
            modified: None,
            is_active: true,
            version: None,
            author: None,
            public_id: None,
            is_managed: false,
        };
        assert_eq!(p1.display_name(), "MyPlugin");

        let p2 = InstalledPlugin {
            filename: "DeactPlugin.wasm.deactivated".into(),
            path: PathBuf::from("plugins/DeactPlugin.wasm.deactivated"),
            size_bytes: 100,
            modified: None,
            is_active: false,
            version: None,
            author: None,
            public_id: None,
            is_managed: false,
        };
        assert_eq!(p2.display_name(), "DeactPlugin");

        let p3 = InstalledPlugin {
            filename: "Legacy.deactivated".into(),
            ..p2
        };
        assert_eq!(p3.display_name(), "Legacy");
    }

    #[test]
    fn test_remove_plugin_rejects_traversal_and_matches_public_id() {
        let base = std::env::temp_dir().join(format!("ppm-local-test-{}", std::process::id()));
        let plugins_dir = base.join("plugins");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&plugins_dir).unwrap();

        // A file outside plugins_dir must never be removed via path traversal.
        let outside = base.join("keep.txt");
        fs::write(&outside, b"keep").unwrap();
        assert!(remove_plugin(&plugins_dir, "../keep.txt").is_err());
        assert!(outside.exists());

        // A lockfile-tracked plugin can be removed by its marketplace public_id.
        let wasm = plugins_dir.join("EpicRTP.wasm");
        fs::write(&wasm, b"\0asm").unwrap();
        let mut lock = super::super::lockfile::Lockfile::default();
        lock.plugins.insert(
            "S1zisQxa".into(),
            super::super::lockfile::LockedPlugin {
                id: 1,
                public_id: "S1zisQxa".into(),
                name: "EpicRTP".into(),
                author: "Alex".into(),
                version: Some("1.2.0".into()),
                file: "EpicRTP.wasm".into(),
                updated_at: Some("2026-10-01".into()),
            },
        );
        lock.save_to_dir(&plugins_dir).unwrap();

        let removed = remove_plugin(&plugins_dir, "S1zisQxa").unwrap();
        assert_eq!(removed, wasm);
        assert!(!wasm.exists());

        let reloaded = super::super::lockfile::Lockfile::load_from_dir(&plugins_dir).unwrap();
        assert!(reloaded.plugins.is_empty());

        let _ = fs::remove_dir_all(&base);
    }
}
