// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use futures_util::StreamExt;
use reqwest::{Client, RequestBuilder, StatusCode, header};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use tokio::io::AsyncWriteExt;

use crate::install_source;
use crate::installer::download_progress_bar;
use crate::ui;

const REPO: &str = "Pumpkin-MC/ppm";
const GITHUB_API: &str = "https://api.github.com";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CURRENT_COMMIT: &str = env!("PPM_GIT_COMMIT");

/// Release channel ppm can update itself from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Nightly,
}

impl Channel {
    fn label(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Nightly => "nightly",
        }
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    /// Asset digest reported by GitHub, e.g. `sha256:<hex>`.
    digest: Option<String>,
}

/// Checks GitHub for a newer ppm build on the given channel and replaces the running executable with it.
///
/// Refuses to replace executables owned by an external package manager unless `break_system_package`
/// is set. Checking for updates is always allowed.
pub async fn self_update(
    channel: Channel,
    check_only: bool,
    force: bool,
    break_system_package: bool,
) -> Result<()> {
    let current_exe = env::current_exe().context("Failed to locate the running ppm executable")?;

    let manager = install_source::detect(&current_exe);

    if let Some(manager) = &manager
        && !check_only
    {
        if !break_system_package {
            bail!(
                "ppm at '{}' was installed by {}, so it won't update itself.\n  {}\n  Use '--break-system-package' to replace it anyway (this can confuse your package manager).",
                current_exe.display(),
                manager.describe(),
                manager.update_hint()
            );
        }
        ui::warn(format!(
            "ppm was installed by {}, replacing it anyway because of --break-system-package.",
            manager.describe()
        ));
    }

    let http = Client::builder()
        .user_agent(format!("ppm/{CURRENT_VERSION} (Pumpkin Package Manager)"))
        .timeout(Duration::from_secs(300))
        .build()
        .context("Failed to initialize HTTP client")?;

    let release = fetch_release(&http, channel).await?;
    let release_commit = fetch_tag_commit(&http, &release.tag_name).await?;

    ui::info(format!(
        "Current version: {}",
        describe(CURRENT_VERSION, CURRENT_COMMIT).cyan()
    ));
    ui::info(format!(
        "Latest {} build: {}",
        channel.label(),
        describe(&release.tag_name, &release_commit).bright_green()
    ));

    let update_available = needs_update(
        channel,
        CURRENT_VERSION,
        CURRENT_COMMIT,
        &release.tag_name,
        &release_commit,
    );

    if !update_available && !force {
        ui::success("ppm is already up to date.");
        return Ok(());
    }

    if check_only {
        match &manager {
            Some(manager) => {
                let how = match &manager.update_command {
                    Some(command) => format!("update it with `{command}`"),
                    None => "update it through your package manager".to_string(),
                };
                ui::info(format!(
                    "A newer release is available on GitHub. ppm was installed by {}, {how} once the new version is packaged.",
                    manager.describe()
                ));
            }
            None => {
                let command = match channel {
                    Channel::Stable => "ppm self-update",
                    Channel::Nightly => "ppm self-update --nightly",
                };
                ui::info(format!(
                    "An update is available. Run '{}' to install it.",
                    command.cyan()
                ));
            }
        }
        return Ok(());
    }

    let asset_name = asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .with_context(|| {
            format!(
                "Release '{}' has no '{asset_name}' binary. Download it manually from https://github.com/{REPO}/releases",
                release.tag_name
            )
        })?;

    let temp_file = tempfile::Builder::new()
        .prefix("ppm-self-update-")
        .tempfile()
        .context("Failed to create temporary file")?;

    download_and_replace(&http, asset, &temp_file, &current_exe).await?;

    ui::success(format!(
        "Updated ppm to {} at {}",
        release.tag_name.bright_green().bold(),
        current_exe.display().to_string().cyan()
    ));

    Ok(())
}

async fn download_and_replace(
    http: &Client,
    asset: &ReleaseAsset,
    temp_file: &NamedTempFile,
    current_exe: &Path,
) -> Result<()> {
    ui::info(format!("Downloading {}...", asset.name.cyan()));

    let resp = http
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Failed to connect to GitHub")?;
    if !resp.status().is_success() {
        bail!(
            "Failed to download '{}' ({})",
            asset.browser_download_url,
            resp.status()
        );
    }

    let pb = download_progress_bar(resp.content_length());
    let temp_path = temp_file.path();
    let mut file = tokio::fs::File::from_std(
        temp_file
            .as_file()
            .try_clone()
            .context("Failed to open temporary file")?,
    );
    let mut hasher = Sha256::new();
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading file stream")?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .context("Error writing chunk to file")?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    file.flush().await.context("Failed to flush file")?;
    drop(file);
    pb.finish_and_clear();

    match asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        Some(expected) => {
            let actual = to_hex(&hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                bail!(
                    "Checksum mismatch for '{}': expected {expected}, got {actual}",
                    asset.name
                );
            }
        }
        None => bail!(
            "GitHub did not report a SHA-256 checksum for '{}', refusing to install an unverified binary",
            asset.name
        ),
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(temp_path, fs::Permissions::from_mode(0o755))
            .context("Failed to mark the downloaded binary as executable")?;
    }

    self_replace::self_replace(temp_path).with_context(|| {
        format!(
            "Failed to replace '{}'. Check that you have write permission to it (it may need elevated privileges)",
            current_exe.display()
        )
    })?;

    Ok(())
}

async fn fetch_release(http: &Client, channel: Channel) -> Result<Release> {
    let url = match channel {
        Channel::Stable => format!("{GITHUB_API}/repos/{REPO}/releases/latest"),
        Channel::Nightly => format!("{GITHUB_API}/repos/{REPO}/releases/tags/nightly"),
    };

    let resp = send_api(http.get(&url)).await?;
    resp.json()
        .await
        .context("Failed to parse GitHub release response")
}

/// Resolves the commit SHA a tag points to.
async fn fetch_tag_commit(http: &Client, tag: &str) -> Result<String> {
    let url = format!("{GITHUB_API}/repos/{REPO}/commits/{tag}");
    let resp = send_api(
        http.get(&url)
            .header(header::ACCEPT, "application/vnd.github.sha"),
    )
    .await?;
    let sha = resp
        .text()
        .await
        .context("Failed to read commit SHA from GitHub")?;
    Ok(sha.trim().to_string())
}

/// Sends a GitHub API request, authenticating with `GITHUB_TOKEN` when it is set.
async fn send_api(request: RequestBuilder) -> Result<reqwest::Response> {
    let request = match env::var("GITHUB_TOKEN") {
        Ok(token) if !token.trim().is_empty() => request.bearer_auth(token.trim()),
        _ => request,
    };

    let resp = request
        .send()
        .await
        .context("Failed to connect to the GitHub API")?;

    match resp.status() {
        status if status.is_success() => Ok(resp),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => bail!(
            "GitHub API rate limit reached ({}). Try again later or set GITHUB_TOKEN to raise the limit.",
            resp.status()
        ),
        StatusCode::NOT_FOUND => bail!("Release not found on https://github.com/{REPO}/releases"),
        status => {
            let text = resp.text().await.unwrap_or_default();
            bail!("GitHub API error ({status}): {text}")
        }
    }
}

/// Returns the release asset name matching the platform this binary was built for.
fn asset_name() -> Result<&'static str> {
    let name = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") if cfg!(target_env = "musl") => "ppm-X64-Linux-musl",
        ("linux", "x86_64") => "ppm-X64-Linux",
        ("linux", "aarch64") => "ppm-ARM64-Linux",
        ("macos", "aarch64") => "ppm-ARM64-macOS",
        ("windows", "x86_64") => "ppm-X64-Windows.exe",
        (os, arch) => bail!(
            "No prebuilt ppm binary is published for {os}/{arch}. Build it from source: https://github.com/{REPO}"
        ),
    };
    Ok(name)
}

/// Decides whether the release is newer than the running build.
///
/// Builds from the same commit are always up to date. Otherwise a nightly is always considered
/// newer, while a stable release must have a higher version than the running one.
fn needs_update(
    channel: Channel,
    current_version: &str,
    current_commit: &str,
    release_tag: &str,
    release_commit: &str,
) -> bool {
    if !current_commit.is_empty() && current_commit.eq_ignore_ascii_case(release_commit) {
        return false;
    }

    match channel {
        Channel::Nightly => true,
        Channel::Stable => {
            let release_version = release_tag.trim_start_matches('v');
            match (
                Version::parse(current_version),
                Version::parse(release_version),
            ) {
                (Ok(current), Ok(release)) => release > current,
                _ => current_version != release_version,
            }
        }
    }
}

fn describe(version: &str, commit: &str) -> String {
    match commit.get(..7) {
        Some(short) => format!("{version} ({short})"),
        None => version.to_string(),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_A: &str = "12e3f5bc431e538db3e5aa8c8b99f3f8ecc2fa54";
    const SHA_B: &str = "5bdf690aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn same_commit_is_up_to_date() {
        assert!(!needs_update(
            Channel::Stable,
            "0.1.0",
            SHA_A,
            "0.1.1",
            SHA_A
        ));
        assert!(!needs_update(
            Channel::Nightly,
            "0.1.0",
            SHA_A,
            "nightly",
            SHA_A
        ));
    }

    #[test]
    fn nightly_with_different_commit_needs_update() {
        assert!(needs_update(
            Channel::Nightly,
            "0.1.0",
            SHA_A,
            "nightly",
            SHA_B
        ));
        assert!(needs_update(
            Channel::Nightly,
            "0.1.0",
            "",
            "nightly",
            SHA_B
        ));
    }

    #[test]
    fn stable_compares_versions() {
        assert!(needs_update(
            Channel::Stable,
            "0.1.0",
            SHA_A,
            "0.1.1",
            SHA_B
        ));
        assert!(needs_update(
            Channel::Stable,
            "0.1.0",
            SHA_A,
            "v0.2.0",
            SHA_B
        ));
        assert!(!needs_update(
            Channel::Stable,
            "0.2.0",
            SHA_A,
            "0.1.1",
            SHA_B
        ));
        assert!(!needs_update(
            Channel::Stable,
            "0.1.1",
            SHA_A,
            "0.1.1",
            SHA_B
        ));
    }

    #[test]
    fn stable_falls_back_to_tag_comparison() {
        assert!(needs_update(
            Channel::Stable,
            "0.1.0",
            "",
            "release-2",
            SHA_B
        ));
        assert!(!needs_update(Channel::Stable, "0.1.0", "", "0.1.0", SHA_B));
    }

    #[test]
    fn describe_shortens_commit() {
        assert_eq!(describe("0.1.1", SHA_A), "0.1.1 (12e3f5b)");
        assert_eq!(describe("0.1.1", ""), "0.1.1");
    }
}
