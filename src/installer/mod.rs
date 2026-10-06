// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

pub mod local;
pub mod lockfile;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::market::{ExternalPluginMetadata, MarketClient, PluginTarget, parse_plugin_input};
pub use local::*;
pub use lockfile::*;

/// Outcome of installing a plugin.
#[derive(Debug, Clone)]
pub struct InstalledOutcome {
    pub path: PathBuf,
    pub display_name: String,
    pub metadata: Option<ExternalPluginMetadata>,
}

/// Downloads and installs a plugin from the marketplace or a direct URL into the destination plugins directory.
pub async fn install_plugin(
    client: &MarketClient,
    plugin_name_or_url: &str,
    plugins_dir: &Path,
    custom_output: Option<&Path>,
    force: bool,
    token: Option<&str>,
) -> Result<InstalledOutcome> {
    if !plugins_dir.exists() {
        fs::create_dir_all(plugins_dir).with_context(|| {
            format!(
                "Failed to create plugins directory at '{}'",
                plugins_dir.display()
            )
        })?;
    }

    // Initiate download based on target type
    let parsed = parse_plugin_input(plugin_name_or_url);
    let (resp, default_filename, display_name, metadata) = match parsed {
        PluginTarget::DirectUrl(url) => client.download_direct_url(&url, token).await?,
        PluginTarget::Marketplace(target) => client.download_plugin(&target, token).await?,
    };

    let dest_path = if let Some(custom) = custom_output {
        if custom.is_absolute() {
            custom.to_path_buf()
        } else {
            plugins_dir.join(custom)
        }
    } else {
        plugins_dir.join(&default_filename)
    };

    if dest_path.exists() && !force {
        bail!(
            "Target file '{}' already exists. Use '--force' / '-f' to overwrite.",
            dest_path.display()
        );
    }

    let total_size = resp.content_length();

    let pb = if let Some(size) = total_size {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:38.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bytes} downloaded ({bytes_per_sec})",
                )
                .unwrap(),
        );
        pb
    };

    let temp_path = dest_path.with_extension("tmp.download");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .with_context(|| format!("Failed to create temporary file: {}", temp_path.display()))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error while downloading file stream")?;
        file.write_all(&chunk)
            .await
            .context("Error writing chunk to file")?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    file.flush().await.context("Failed to flush file")?;
    drop(file);

    pb.finish_with_message("Download complete!");

    // Rename temp file to final destination
    if dest_path.exists() {
        let _ = fs::remove_file(&dest_path);
    }
    fs::rename(&temp_path, &dest_path).with_context(|| {
        format!(
            "Failed to move downloaded file to '{}'",
            dest_path.display()
        )
    })?;

    let outcome = InstalledOutcome {
        path: dest_path,
        display_name,
        metadata: metadata.clone(),
    };

    // Only for plugins coming from the marketplace: record into .ppm.lock
    if let Some(meta) = &metadata
        && let Ok(mut lockfile) = Lockfile::load_from_dir(plugins_dir)
    {
        let filename = outcome
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        lockfile.record_marketplace_plugin(meta, &filename);
        let _ = lockfile.save_to_dir(plugins_dir);
    }

    Ok(outcome)
}
