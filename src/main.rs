// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

mod cli;
mod installer;
mod market;
mod self_install;
mod template;
mod ui;

use std::str::FromStr;

use anyhow::{Result, bail};
use clap::Parser;
use colored::Colorize;
use inquire::Confirm;

use cli::{Cli, Commands};
use market::{ListPluginsParams, MarketClient};
use template::{PluginLanguage, WizardConfig};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        ui::error(format!("{err:#}"));
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    // Attempt automatic self-install to user PATH on first run (like cargo/rustup)
    let _ = self_install::ensure_installed_in_path();

    let plugins_dir = cli.resolved_plugins_dir();
    let client = MarketClient::new(cli.market_url);

    match cli.command {
        Commands::Search(args) => {
            let query = match market::parse_plugin_input(&args.query) {
                market::PluginTarget::Marketplace(q) => q,
                market::PluginTarget::DirectUrl(url) => url,
            };

            let params = ListPluginsParams {
                q: Some(query),
                category: args.category,
                type_: args.type_,
                page: Some(args.page),
                limit: Some(args.limit),
            };

            let plugins = client.list_plugins(&params).await?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&plugins)?);
            } else {
                ui::print_plugins_table(&plugins);
            }
        }

        Commands::Info(args) => {
            let plugin = client.get_plugin(&args.plugin).await?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&plugin)?);
            } else {
                ui::print_plugin_details(&plugin);
            }
        }

        Commands::Install(args) => {
            if args.plugins.is_empty() {
                let lockfile = installer::Lockfile::load_from_dir(&plugins_dir)?;
                if lockfile.plugins.is_empty() {
                    ui::warn(format!(
                        "No plugins specified and no marketplace plugins found in '{}'.",
                        installer::Lockfile::locate(&plugins_dir).display()
                    ));
                    println!("  Usage: ppm install <PLUGINS>...");
                    return Ok(());
                }

                ui::info(format!(
                    "Found .ppm.lock with {} marketplace plugin(s). Restoring...",
                    lockfile.plugins.len()
                ));

                let mut installed_count = 0;
                let mut failed = Vec::new();
                let total = lockfile.plugins.len();

                for (public_id, locked) in &lockfile.plugins {
                    let ver_display = locked.version.as_deref().unwrap_or("latest");
                    ui::info(format!(
                        "Restoring '{}' (v{}, ID: {}) from marketplace...",
                        locked.name.cyan(),
                        ver_display.bright_green(),
                        public_id.dimmed()
                    ));

                    match installer::install_plugin(
                        &client,
                        public_id,
                        &plugins_dir,
                        None,
                        args.force,
                        args.token.as_deref(),
                    )
                    .await
                    {
                        Ok(outcome) => {
                            let file_size = std::fs::metadata(&outcome.path)
                                .map(|m| installer::format_size(m.len()))
                                .unwrap_or_else(|_| "unknown size".into());

                            let version_str = outcome
                                .metadata
                                .as_ref()
                                .and_then(|m| m.version.as_deref())
                                .map(|v| format!(" v{v}"))
                                .unwrap_or_default();

                            ui::success(format!(
                                "Restored '{}{}' ({})",
                                outcome.display_name.bright_green(),
                                version_str.bright_cyan(),
                                file_size.bright_blue()
                            ));
                            installed_count += 1;
                        }
                        Err(e) => {
                            ui::error(format!("Failed to restore '{}': {e}", locked.name));
                            failed.push((locked.name.clone(), e.to_string()));
                        }
                    }
                }

                println!();
                if failed.is_empty() {
                    ui::success(format!(
                        "Successfully restored all {installed_count}/{total} plugin(s) from .ppm.lock."
                    ));
                } else {
                    ui::warn(format!(
                        "Restoration finished with issues: {installed_count}/{total} succeeded, {} failed.",
                        failed.len()
                    ));
                }
                return Ok(());
            }

            if args.plugins.len() > 1 && args.output.is_some() {
                bail!(
                    "The '--output' / '-o' option can only be used when installing a single plugin."
                );
            }

            let mut installed_count = 0;
            let mut failed: Vec<(String, String)> = Vec::new();
            let total = args.plugins.len();

            for plugin_req in &args.plugins {
                let target_desc = match market::parse_plugin_input(plugin_req) {
                    market::PluginTarget::DirectUrl(ref url) => format!("URL '{}'", url.cyan()),
                    market::PluginTarget::Marketplace(ref id) => format!("'{}'", id.cyan()),
                };

                ui::info(format!("Fetching and installing {target_desc}..."));

                match installer::install_plugin(
                    &client,
                    plugin_req,
                    &plugins_dir,
                    args.output.as_deref(),
                    args.force,
                    args.token.as_deref(),
                )
                .await
                {
                    Ok(outcome) => {
                        let file_size = std::fs::metadata(&outcome.path)
                            .map(|m| installer::format_size(m.len()))
                            .unwrap_or_else(|_| "unknown size".into());

                        let version_str = outcome
                            .metadata
                            .as_ref()
                            .and_then(|m| m.version.as_deref())
                            .map(|v| format!(" v{v}"))
                            .unwrap_or_default();

                        ui::success(format!(
                            "Successfully installed '{}{}' ({}) to {}",
                            outcome.display_name.bright_green().bold(),
                            version_str.bright_cyan(),
                            file_size.bright_blue(),
                            outcome.path.display().to_string().cyan()
                        ));
                        installed_count += 1;
                    }
                    Err(e) => {
                        ui::error(format!("Failed to install '{plugin_req}': {e}"));
                        failed.push((plugin_req.clone(), e.to_string()));
                    }
                }
            }

            if total > 1 {
                println!();
                if failed.is_empty() {
                    ui::success(format!(
                        "Batch installation complete! {installed_count}/{total} plugin(s) installed."
                    ));
                } else {
                    ui::warn(format!(
                        "Batch installation finished with issues: {installed_count}/{total} succeeded, {} failed.",
                        failed.len()
                    ));
                }
            }

            if installed_count > 0 {
                println!(
                    "  {}",
                    "Your Pumpkin server will load installed plugins on next startup or reload."
                        .dimmed()
                );
            }

            if !failed.is_empty() && total == 1 {
                bail!("Failed to install '{}'", args.plugins[0]);
            }
        }

        Commands::Uninstall(args) => {
            if !args.yes {
                let prompt_text = if args.plugins.len() == 1 {
                    let target_name = match market::parse_plugin_input(&args.plugins[0]) {
                        market::PluginTarget::Marketplace(id) => id,
                        market::PluginTarget::DirectUrl(url) => url,
                    };
                    format!("Are you sure you want to remove plugin '{target_name}'?")
                } else {
                    format!(
                        "Are you sure you want to remove {} plugins ({})?",
                        args.plugins.len(),
                        args.plugins.join(", ")
                    )
                };

                let confirm = Confirm::new(&prompt_text).with_default(false).prompt()?;

                if !confirm {
                    ui::warn("Plugin removal aborted.");
                    return Ok(());
                }
            }

            let mut removed_count = 0;
            let mut failed = Vec::new();

            for plugin_req in &args.plugins {
                let target_name = match market::parse_plugin_input(plugin_req) {
                    market::PluginTarget::Marketplace(id) => id,
                    market::PluginTarget::DirectUrl(url) => url,
                };

                match installer::remove_plugin(&plugins_dir, &target_name) {
                    Ok(removed_path) => {
                        ui::success(format!(
                            "Removed plugin file: {}",
                            removed_path.display().to_string().cyan()
                        ));
                        removed_count += 1;
                    }
                    Err(e) => {
                        ui::error(format!("Failed to remove '{plugin_req}': {e}"));
                        failed.push((plugin_req.clone(), e.to_string()));
                    }
                }
            }

            if args.plugins.len() > 1 {
                println!();
                if failed.is_empty() {
                    ui::success(format!("Successfully removed {removed_count} plugin(s)."));
                } else {
                    ui::warn(format!(
                        "Removal finished with issues: {removed_count}/{} succeeded.",
                        args.plugins.len()
                    ));
                }
            }

            if !failed.is_empty() && args.plugins.len() == 1 {
                bail!("Failed to remove '{}'", args.plugins[0]);
            }
        }

        Commands::Installed(args) => {
            let installed = installer::scan_installed(&plugins_dir)?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&installed)?);
            } else {
                println!(
                    "Installed plugins in '{}':",
                    plugins_dir.display().to_string().cyan()
                );
                ui::print_installed_table(&installed);
            }
        }

        Commands::Update(args) => {
            let lockfile = installer::Lockfile::load_from_dir(&plugins_dir)?;

            // Case 1: Specific plugin specified to update
            if let Some(ref target) = args.plugin {
                // If it exists in lockfile, use its exact public_id!
                if let Some(locked) = lockfile.find_plugin(target) {
                    let current_ver = locked.version.as_deref().unwrap_or("0.0.0");
                    ui::info(format!(
                        "Found tracked plugin '{}' in .ppm.lock (ID: {}, version: {})",
                        locked.name.cyan(),
                        locked.public_id.dimmed(),
                        current_ver.bright_blue()
                    ));

                    if args.check {
                        match client.check_update(&locked.name, current_ver).await {
                            Ok(info) => {
                                let latest = info.latest_version.unwrap_or_else(|| "latest".into());
                                if info.update_available {
                                    ui::info(format!(
                                        "Plugin '{}': update available ({} -> {})",
                                        locked.name.cyan(),
                                        current_ver.yellow(),
                                        latest.bright_green().bold()
                                    ));
                                } else {
                                    ui::info(format!(
                                        "Plugin '{}': up to date ({})",
                                        locked.name.cyan(),
                                        current_ver.dimmed()
                                    ));
                                }
                            }
                            Err(_) => {
                                if let Ok(meta) = client.get_plugin(&locked.public_id).await {
                                    let latest = meta.version.unwrap_or_else(|| "latest".into());
                                    ui::info(format!(
                                        "Plugin '{}': latest marketplace version is {}",
                                        locked.name.cyan(),
                                        latest.bright_green()
                                    ));
                                } else {
                                    ui::warn(format!(
                                        "Plugin '{}' not found on marketplace.",
                                        locked.name
                                    ));
                                }
                            }
                        }
                        return Ok(());
                    }

                    ui::info(format!(
                        "Updating '{}' (ID: {})...",
                        locked.name.cyan(),
                        locked.public_id.dimmed()
                    ));

                    let outcome = installer::install_plugin(
                        &client,
                        &locked.public_id,
                        &plugins_dir,
                        None,
                        true,
                        args.token.as_deref(),
                    )
                    .await?;

                    ui::success(format!(
                        "Updated '{}' -> {}",
                        outcome.display_name.bright_green(),
                        outcome.path.display().to_string().cyan()
                    ));
                    return Ok(());
                }

                // If not in lockfile, resolve on marketplace
                ui::info(format!(
                    "'{}' not tracked in .ppm.lock; resolving on marketplace...",
                    target.cyan()
                ));

                if args.check {
                    if let Ok(meta) = client.get_plugin(target).await {
                        let ver = meta.version.unwrap_or_else(|| "latest".into());
                        ui::info(format!(
                            "Plugin '{}': latest marketplace version is {}",
                            meta.name.cyan(),
                            ver.bright_green()
                        ));
                    } else {
                        ui::warn(format!("Plugin '{target}' not found on marketplace."));
                    }
                    return Ok(());
                }

                ui::info(format!("Updating '{target}'..."));
                let outcome = installer::install_plugin(
                    &client,
                    target,
                    &plugins_dir,
                    None,
                    true,
                    args.token.as_deref(),
                )
                .await?;

                ui::success(format!(
                    "Updated '{}' -> {}",
                    outcome.display_name.bright_green(),
                    outcome.path.display().to_string().cyan()
                ));
                return Ok(());
            }

            // Case 2: Update all plugins
            // Prioritize marketplace plugins recorded in .ppm.lock
            if !lockfile.plugins.is_empty() {
                ui::info(format!(
                    "Checking updates for {} marketplace plugin(s) from .ppm.lock...",
                    lockfile.plugins.len()
                ));

                let mut updated_count = 0;
                let total = lockfile.plugins.len();

                for (public_id, locked) in &lockfile.plugins {
                    let current_ver = locked.version.as_deref().unwrap_or("0.0.0");

                    if args.check {
                        match client.check_update(&locked.name, current_ver).await {
                            Ok(info) => {
                                let latest = info.latest_version.unwrap_or_else(|| "latest".into());
                                if info.update_available {
                                    ui::info(format!(
                                        "Plugin '{}' (ID: {}): update available ({} -> {})",
                                        locked.name.cyan(),
                                        public_id.dimmed(),
                                        current_ver.yellow(),
                                        latest.bright_green().bold()
                                    ));
                                } else {
                                    ui::info(format!(
                                        "Plugin '{}': up to date ({})",
                                        locked.name.cyan(),
                                        current_ver.dimmed()
                                    ));
                                }
                            }
                            Err(_) => {
                                if let Ok(meta) = client.get_plugin(public_id).await {
                                    let latest = meta.version.unwrap_or_else(|| "latest".into());
                                    ui::info(format!(
                                        "Plugin '{}' (ID: {}): latest marketplace version is {}",
                                        locked.name.cyan(),
                                        public_id.dimmed(),
                                        latest.bright_green()
                                    ));
                                } else {
                                    ui::warn(format!(
                                        "Could not query update info for '{}'.",
                                        locked.name
                                    ));
                                }
                            }
                        }
                        continue;
                    }

                    ui::info(format!(
                        "Updating '{}' (ID: {})...",
                        locked.name.cyan(),
                        public_id.dimmed()
                    ));

                    match installer::install_plugin(
                        &client,
                        public_id,
                        &plugins_dir,
                        None,
                        true,
                        args.token.as_deref(),
                    )
                    .await
                    {
                        Ok(outcome) => {
                            ui::success(format!(
                                "Updated '{}' -> {}",
                                outcome.display_name.bright_green(),
                                outcome.path.display().to_string().cyan()
                            ));
                            updated_count += 1;
                        }
                        Err(e) => {
                            ui::error(format!("Failed to update '{}': {e}", locked.name));
                        }
                    }
                }

                if !args.check {
                    println!();
                    ui::success(format!(
                        "Update process complete! {updated_count}/{total} plugin(s) updated."
                    ));
                }

                return Ok(());
            }

            // Fallback if no .ppm.lock exists: scan directory
            let installed = installer::scan_installed(&plugins_dir)?;
            if installed.is_empty() {
                ui::warn(format!(
                    "No plugins found in '{}'. Nothing to update.",
                    plugins_dir.display()
                ));
                return Ok(());
            }

            ui::info(format!(
                "No .ppm.lock found. Scanning {} local plugin(s) on disk...",
                installed.len()
            ));

            for p in installed {
                let name = p.display_name().to_string();
                if args.check {
                    if let Ok(meta) = client.get_plugin(&name).await {
                        let ver = meta.version.unwrap_or_else(|| "latest".into());
                        ui::info(format!(
                            "Plugin '{}': latest marketplace version is {}",
                            name.cyan(),
                            ver.bright_green()
                        ));
                    } else {
                        ui::warn(format!("Plugin '{name}' not found on marketplace."));
                    }
                    continue;
                }

                ui::info(format!("Updating '{name}'..."));
                match installer::install_plugin(
                    &client,
                    &name,
                    &plugins_dir,
                    None,
                    true,
                    args.token.as_deref(),
                )
                .await
                {
                    Ok(outcome) => {
                        ui::success(format!(
                            "Updated '{}' -> {}",
                            outcome.display_name.bright_green(),
                            outcome.path.display().to_string().cyan()
                        ));
                    }
                    Err(e) => {
                        ui::error(format!("Failed to update '{name}': {e}"));
                    }
                }
            }
        }

        Commands::New(args) => {
            let lang = match args.lang {
                Some(l) => Some(PluginLanguage::from_str(&l)?),
                None => None,
            };

            if !args.non_interactive && args.name.is_none() {
                ui::print_logo();
                println!(
                    "{}",
                    "Welcome to the Pumpkin Plugin Generator Wizard!\n".bright_cyan()
                );
            }

            let config = WizardConfig {
                name: args.name,
                lang,
                author: args.author,
                description: args.description,
                target_dir: args.dir,
                non_interactive: args.non_interactive,
            };

            let project_path = template::run_wizard(config)?;

            ui::success(format!(
                "Scaffolded new Pumpkin plugin project at '{}'!",
                project_path.display().to_string().bright_green().bold()
            ));

            println!();
            println!("{}", "Next steps:".bright_yellow().bold());
            println!("  1. Navigate to your plugin:");
            println!("     {}", format!("cd {}", project_path.display()).cyan());
            println!("  2. Read the instructions:");
            println!("     {}", "cat README.md".cyan());
            println!(
                "  3. Build your WebAssembly plugin and copy to Pumpkin's plugins/ directory!"
            );
            println!();
        }

        Commands::SelfInstall(args) => {
            self_install::install_self(args.dir.as_deref(), args.force)?;
        }
    }

    Ok(())
}
