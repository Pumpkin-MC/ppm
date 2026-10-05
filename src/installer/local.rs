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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub filename: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    #[serde(skip)]
    pub modified: Option<SystemTime>,
    pub is_active: bool,
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
            .unwrap_or(&self.filename)
    }
}

fn format_system_time(time: SystemTime) -> String {
    if let Ok(duration) = SystemTime::now().duration_since(time) {
        let secs = duration.as_secs();
        if secs < 60 {
            "just now".to_string()
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
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

    let mut list = Vec::new();
    let entries = fs::read_dir(plugins_dir).with_context(|| {
        format!(
            "Failed to read plugins directory: {}",
            plugins_dir.display()
        )
    })?;

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

            list.push(InstalledPlugin {
                filename,
                path,
                size_bytes,
                modified,
                is_active: is_wasm,
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
            "Plugins directory '{}' does not exist.",
            plugins_dir.display()
        );
    }

    let trimmed = plugin.trim();
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
        // Try case-insensitive lookup
        let installed = scan_installed(plugins_dir)?;
        let found = installed.into_iter().find(|p| {
            p.filename.eq_ignore_ascii_case(trimmed)
                || p.display_name().eq_ignore_ascii_case(trimmed)
        });

        match found {
            Some(p) => p.path,
            None => {
                bail!(
                    "Plugin '{}' not found in '{}'",
                    trimmed,
                    plugins_dir.display()
                );
            }
        }
    };

    fs::remove_file(&target_path)
        .with_context(|| format!("Failed to remove plugin file: {}", target_path.display()))?;

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
        };
        assert_eq!(p1.display_name(), "MyPlugin");

        let p2 = InstalledPlugin {
            filename: "DeactPlugin.wasm.deactivated".into(),
            path: PathBuf::from("plugins/DeactPlugin.wasm.deactivated"),
            size_bytes: 100,
            modified: None,
            is_active: false,
        };
        assert_eq!(p2.display_name(), "DeactPlugin");
    }
}
