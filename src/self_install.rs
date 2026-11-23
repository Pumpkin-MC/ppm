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
use clap_complete::{Shell, generate};

use crate::cli;
use crate::tr;
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

    bail!("{}", tr!("self_install.no_home_dir"))
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

    // If already running from the target path, ensure completions exist and return
    if let (Ok(canon_cur), Ok(canon_target)) =
        (current_exe.canonicalize(), target_exe_path.canonicalize())
        && canon_cur == canon_target
    {
        ensure_completions_if_missing();
        return Ok(());
    }

    // If ppm is already installed at the destination, ensure completions exist
    if target_exe_path.exists() {
        ensure_completions_if_missing();
        return Ok(());
    }

    // Install to PATH
    install_to(&current_exe, &target_exe_path, &target_bin_dir)?;
    install_all_completions();

    Ok(())
}

/// Explicitly installs the current executable to the specified or default PATH bin directory.
pub fn install_self(custom_dir: Option<&Path>, _force: bool) -> Result<PathBuf> {
    let current_exe = env::current_exe().with_context(|| tr!("self_install.get_exe_failed"))?;

    let target_bin_dir = match custom_dir {
        Some(d) => d.to_path_buf(),
        None => get_install_bin_dir()?,
    };

    let target_exe_name = if cfg!(windows) { "ppm.exe" } else { "ppm" };
    let target_exe_path = target_bin_dir.join(target_exe_name);

    // Never copy the running executable onto itself (would fail with ETXTBSY or
    // truncate the binary even when `--force` is passed); still refresh completions.
    if target_exe_path.exists()
        && let (Ok(c1), Ok(c2)) = (current_exe.canonicalize(), target_exe_path.canonicalize())
        && c1 == c2
    {
        ui::info(tr!(
            "self_install.already_installed",
            path = target_exe_path.display()
        ));
        let shells = install_all_completions();
        if !shells.is_empty() {
            ui::success(tr!(
                "self_install.completions_configured",
                shells = shells.join(", ")
            ));
        }
        return Ok(target_exe_path);
    }

    install_to(&current_exe, &target_exe_path, &target_bin_dir)?;

    let shells = install_all_completions();
    if !shells.is_empty() {
        ui::success(tr!(
            "self_install.completions_configured",
            shells = shells.join(", ")
        ));
    }

    Ok(target_exe_path)
}

fn install_to(current_exe: &Path, target_exe_path: &Path, target_bin_dir: &Path) -> Result<()> {
    if !target_bin_dir.exists() {
        fs::create_dir_all(target_bin_dir).with_context(|| {
            tr!(
                "self_install.create_dir_failed",
                path = target_bin_dir.display()
            )
        })?;
    }

    // Stage via a sibling temp file and rename so replacing a running binary
    // succeeds atomically on Unix instead of failing with ETXTBSY.
    let temp_path = target_bin_dir.join(format!(".ppm.install.{}.tmp", std::process::id()));
    if let Err(err) = fs::copy(current_exe, &temp_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err)
            .with_context(|| tr!("self_install.copy_failed", path = target_exe_path.display()));
    }

    // Make executable on Unix before moving into place
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&temp_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&temp_path, perms);
        }
    }

    #[cfg(windows)]
    if target_exe_path.exists() {
        let _ = fs::remove_file(target_exe_path);
    }

    if let Err(err) = fs::rename(&temp_path, target_exe_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(err)
            .with_context(|| tr!("self_install.copy_failed", path = target_exe_path.display()));
    }

    let path_var = env::var("PATH").unwrap_or_default();
    let is_on_path = env::split_paths(&path_var).any(|p| p == target_bin_dir);

    if is_on_path {
        ui::success(tr!(
            "self_install.installed_on_path",
            path = target_exe_path.display()
        ));
    } else {
        ui::success(tr!(
            "self_install.installed",
            path = target_exe_path.display()
        ));
        ui::warn(tr!(
            "self_install.not_on_path",
            dir = target_bin_dir.display()
        ));
        println!("  export PATH=\"{}:$PATH\"", target_bin_dir.display());
    }

    Ok(())
}

/// Generates shell completion script string for the given shell.
pub fn generate_completion_string(shell: Shell) -> String {
    let mut buf = Vec::new();
    let mut cmd = cli::localized_command();
    generate(shell, &mut cmd, "ppm", &mut buf);
    String::from_utf8(buf).unwrap_or_default()
}

/// Ensures shell completions are installed if any standard files are missing.
fn ensure_completions_if_missing() {
    let home = match env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        Ok(h) => PathBuf::from(h),
        Err(_) => return,
    };

    let bash_file = home.join(".local/share/bash-completion/completions/ppm");
    let zsh_file = home.join(".zfunc/_ppm");
    let fish_file = home.join(".config/fish/completions/ppm.fish");

    if !bash_file.exists() || !zsh_file.exists() || !fish_file.exists() {
        let _ = install_all_completions();
    }
}

/// Automatically installs shell completion scripts into standard user shell directories.
/// Returns a list of shells that were successfully configured.
pub fn install_all_completions() -> Vec<String> {
    let home = match env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };

    let mut configured = Vec::new();

    // 1. Bash
    let bash_dir = home.join(".local/share/bash-completion/completions");
    if fs::create_dir_all(&bash_dir).is_ok() {
        let bash_file = bash_dir.join("ppm");
        let script = generate_completion_string(Shell::Bash);
        if fs::write(&bash_file, script).is_ok() {
            configured.push("Bash".to_string());
        }
    }

    // Ensure ~/.bashrc sources the completion file
    let bashrc = home.join(".bashrc");
    if bashrc.is_file()
        && let Ok(content) = fs::read_to_string(&bashrc)
    {
        let marker = "# ppm shell completion";
        if !content.contains(marker) && !content.contains("bash-completion/completions/ppm") {
            let snippet = format!(
                "\n{marker}\n[[ -r ~/.local/share/bash-completion/completions/ppm ]] && source ~/.local/share/bash-completion/completions/ppm\n"
            );
            let _ = fs::OpenOptions::new()
                .append(true)
                .open(&bashrc)
                .and_then(|mut f| std::io::Write::write_all(&mut f, snippet.as_bytes()));
        }
    }

    // 2. Fish
    let fish_dir = home.join(".config/fish/completions");
    if fs::create_dir_all(&fish_dir).is_ok() {
        let fish_file = fish_dir.join("ppm.fish");
        let script = generate_completion_string(Shell::Fish);
        if fs::write(&fish_file, script).is_ok() {
            configured.push("Fish".to_string());
        }
    }

    // 3. Zsh
    let zsh_script = generate_completion_string(Shell::Zsh);
    let zfunc_dir = home.join(".zfunc");
    if fs::create_dir_all(&zfunc_dir).is_ok() {
        let zsh_file = zfunc_dir.join("_ppm");
        if fs::write(&zsh_file, &zsh_script).is_ok() {
            configured.push("Zsh".to_string());
        }
    }

    let zsh_site_dir = home.join(".local/share/zsh/site-functions");
    if fs::create_dir_all(&zsh_site_dir).is_ok() {
        let zsh_site_file = zsh_site_dir.join("_ppm");
        let _ = fs::write(&zsh_site_file, &zsh_script);
    }

    // Ensure ~/.zshrc includes the completion
    let zshrc = home.join(".zshrc");
    if zshrc.is_file()
        && let Ok(content) = fs::read_to_string(&zshrc)
    {
        let marker = "# ppm shell completion";
        if !content.contains(marker)
            && !content.contains(".zfunc/_ppm")
            && !content.contains("_ppm")
        {
            let snippet = format!(
                "\n{marker}\n[[ -f ~/.zfunc/_ppm ]] && fpath=(~/.zfunc $fpath) && autoload -Uz _ppm && compdef _ppm ppm 2>/dev/null || true\n"
            );
            let _ = fs::OpenOptions::new()
                .append(true)
                .open(&zshrc)
                .and_then(|mut f| std::io::Write::write_all(&mut f, snippet.as_bytes()));
        }
    }

    configured
}
