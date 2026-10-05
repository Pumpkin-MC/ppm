// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::ui;

/// Determines the standard target directory for installing ppm as a normal CLI app (~/.local/bin or /usr/local/bin).
pub fn get_install_bin_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local_appdata).join("ppm").join("bin");
            return Ok(p);
        }
        if let Ok(userprofile) = env::var("USERPROFILE") {
            let p = PathBuf::from(userprofile).join(".local").join("bin");
            return Ok(p);
        }
    }

    #[cfg(not(windows))]
    {
        // If running as root, install system-wide into /usr/local/bin
        if let Ok(user) = env::var("USER")
            && user == "root"
            && Path::new("/usr/local/bin").is_dir()
        {
            return Ok(PathBuf::from("/usr/local/bin"));
        }

        // Standard user-level binary directory for normal CLI applications: ~/.local/bin
        if let Ok(home) = env::var("HOME") {
            return Ok(PathBuf::from(home).join(".local").join("bin"));
        }
    }

    bail!("Could not determine user home directory to install ppm.")
}

/// Automatically install ppm on first run if it is not already in PATH.
pub fn ensure_installed_in_path() -> Result<()> {
    // Respect flags and environments that disallow self installation
    if env::var("PPM_NO_SELF_INSTALL").is_ok()
        || env::var("CI").is_ok()
        || env::var("GITHUB_ACTIONS").is_ok()
    {
        return Ok(());
    }

    let current_exe = match env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return Ok(()),
    };

    // If running in development target/ folder, don't auto-install unless forced
    let exe_str = current_exe.to_string_lossy();
    if (exe_str.contains("/target/debug/") || exe_str.contains("/target/release/"))
        && env::var("PPM_FORCE_SELF_INSTALL").is_err()
    {
        return Ok(());
    }

    let target_bin_dir = match get_install_bin_dir() {
        Ok(dir) => dir,
        Err(_) => return Ok(()),
    };

    let target_exe_name = if cfg!(windows) { "ppm.exe" } else { "ppm" };
    let target_exe_path = target_bin_dir.join(target_exe_name);

    // If already running from the target path, nothing to do
    if let (Ok(canon_cur), Ok(canon_target)) =
        (current_exe.canonicalize(), target_exe_path.canonicalize())
        && canon_cur == canon_target
    {
        return Ok(());
    }

    // If ppm is already installed at the destination, don't overwrite on normal runs
    if target_exe_path.exists() {
        return Ok(());
    }

    // Install to PATH
    install_to(&current_exe, &target_exe_path, &target_bin_dir)?;

    Ok(())
}

/// Explicitly installs the current executable to the specified or default PATH bin directory.
pub fn install_self(custom_dir: Option<&Path>, force: bool) -> Result<PathBuf> {
    let current_exe = env::current_exe().context("Failed to get current executable path")?;

    let target_bin_dir = match custom_dir {
        Some(d) => d.to_path_buf(),
        None => get_install_bin_dir()?,
    };

    let target_exe_name = if cfg!(windows) { "ppm.exe" } else { "ppm" };
    let target_exe_path = target_bin_dir.join(target_exe_name);

    if target_exe_path.exists()
        && !force
        && let (Ok(c1), Ok(c2)) = (current_exe.canonicalize(), target_exe_path.canonicalize())
        && c1 == c2
    {
        ui::info(format!(
            "ppm is already running from the installed location: {}",
            target_exe_path.display()
        ));
        return Ok(target_exe_path);
    }

    install_to(&current_exe, &target_exe_path, &target_bin_dir)?;
    Ok(target_exe_path)
}

fn install_to(current_exe: &Path, target_exe_path: &Path, target_bin_dir: &Path) -> Result<()> {
    if !target_bin_dir.exists() {
        fs::create_dir_all(target_bin_dir).with_context(|| {
            format!(
                "Failed to create bin directory: {}",
                target_bin_dir.display()
            )
        })?;
    }

    // Copy executable
    fs::copy(current_exe, target_exe_path).with_context(|| {
        format!(
            "Failed to copy ppm executable to '{}'",
            target_exe_path.display()
        )
    })?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(target_exe_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(target_exe_path, perms);
        }
    }

    let path_var = env::var("PATH").unwrap_or_default();
    let is_on_path = env::split_paths(&path_var).any(|p| p == target_bin_dir);

    if is_on_path {
        ui::success(format!(
            "Installed ppm to {} (now available globally in your PATH)!",
            target_exe_path.display()
        ));
    } else {
        ui::success(format!("Installed ppm to {}", target_exe_path.display()));
        ui::warn(format!(
            "Note: '{}' is not currently in your PATH. Please add it to your shell profile (~/.bashrc or ~/.zshrc):",
            target_bin_dir.display()
        ));
        println!("  export PATH=\"{}:$PATH\"", target_bin_dir.display());
    }

    Ok(())
}
