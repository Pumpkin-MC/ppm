// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

mod cli;
mod i18n;
mod installer;
mod market;
mod self_install;
mod template;
mod ui;

use std::str::FromStr;

use anyhow::{Result, bail};
use colored::Colorize;
use inquire::Confirm;

use cli::{Cli, Commands};
use market::{ListPluginsParams, MarketClient};
use template::{PluginLanguage, WizardConfig};

#[tokio::main]
async fn main() {
    // The locale is initialized before clap parses so that `--help` is localized too.
    let scanned_locale = cli::extract_locale_from_args(std::env::args_os());
    i18n::init(scanned_locale.as_deref());

    let cli = Cli::parse_localized();

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
                    ui::warn(tr!(
                        "install.no_plugins_no_lock",
                        path = installer::Lockfile::locate(&plugins_dir).display()
                    ));
                    println!("{}", tr!("install.usage"));
                    return Ok(());
                }

                ui::info(tr!(
                    "install.lockfile_found",
                    count = lockfile.plugins.len()
                ));

                let mut installed_count = 0;
                let mut failed = Vec::new();
                let total = lockfile.plugins.len();
                let latest_label = tr!("common.latest");

                for (public_id, locked) in &lockfile.plugins {
                    let ver_display = locked.version.as_deref().unwrap_or(latest_label.as_str());
                    ui::info(tr!(
                        "install.restoring",
                        name = locked.name.cyan(),
                        version = ver_display.bright_green(),
                        id = public_id.dimmed()
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
                                .unwrap_or_else(|_| tr!("common.unknown_size"));

                            let version_str = outcome
                                .metadata
                                .as_ref()
                                .and_then(|m| m.version.as_deref())
                                .map(|v| format!(" v{v}"))
                                .unwrap_or_default();

                            ui::success(tr!(
                                "install.restored",
                                name = outcome.display_name.bright_green(),
                                version = version_str.bright_cyan(),
                                size = file_size.bright_blue()
                            ));
                            installed_count += 1;
                        }
                        Err(e) => {
                            ui::error(tr!("install.restore_failed", name = locked.name, error = e));
                            failed.push((locked.name.clone(), e.to_string()));
                        }
                    }
                }

                println!();
                if failed.is_empty() {
                    ui::success(tr!(
                        "install.restore_all_success",
                        installed = installed_count,
                        total = total
                    ));
                } else {
                    ui::warn(tr!(
                        "install.restore_with_issues",
                        installed = installed_count,
                        total = total,
                        failed = failed.len()
                    ));
                }
                return Ok(());
            }

            if args.plugins.len() > 1 && args.output.is_some() {
                bail!("{}", tr!("install.output_single_only"));
            }

            let mut installed_count = 0;
            let mut failed: Vec<(String, String)> = Vec::new();
            let total = args.plugins.len();

            for plugin_req in &args.plugins {
                let target_desc = match market::parse_plugin_input(plugin_req) {
                    market::PluginTarget::DirectUrl(ref url) => {
                        tr!("install.target_url", url = url.cyan())
                    }
                    market::PluginTarget::Marketplace(ref id) => format!("'{}'", id.cyan()),
                };

                ui::info(tr!("install.fetching", target = target_desc));

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
                            .unwrap_or_else(|_| tr!("common.unknown_size"));

                        let version_str = outcome
                            .metadata
                            .as_ref()
                            .and_then(|m| m.version.as_deref())
                            .map(|v| format!(" v{v}"))
                            .unwrap_or_default();

                        ui::success(tr!(
                            "install.success",
                            name = outcome.display_name.bright_green().bold(),
                            version = version_str.bright_cyan(),
                            size = file_size.bright_blue(),
                            path = outcome.path.display().to_string().cyan()
                        ));
                        installed_count += 1;
                    }
                    Err(e) => {
                        ui::error(tr!("install.failed_detail", plugin = plugin_req, error = e));
                        failed.push((plugin_req.clone(), e.to_string()));
                    }
                }
            }

            if total > 1 {
                println!();
                if failed.is_empty() {
                    ui::success(tr!(
                        "install.batch_success",
                        installed = installed_count,
                        total = total
                    ));
                } else {
                    ui::warn(tr!(
                        "install.batch_with_issues",
                        installed = installed_count,
                        total = total,
                        failed = failed.len()
                    ));
                }
            }

            if installed_count > 0 {
                println!("  {}", tr!("install.server_load_hint").dimmed());
            }

            if !failed.is_empty() && total == 1 {
                bail!("{}", tr!("install.failed", plugin = args.plugins[0]));
            }
        }

        Commands::Uninstall(args) => {
            if !args.yes {
                let prompt_text = if args.plugins.len() == 1 {
                    let target_name = match market::parse_plugin_input(&args.plugins[0]) {
                        market::PluginTarget::Marketplace(id) => id,
                        market::PluginTarget::DirectUrl(url) => url,
                    };
                    tr!("uninstall.confirm_single", plugin = target_name)
                } else {
                    tr!(
                        "uninstall.confirm_multiple",
                        count = args.plugins.len(),
                        plugins = args.plugins.join(", ")
                    )
                };

                let confirm = Confirm::new(&prompt_text).with_default(false).prompt()?;

                if !confirm {
                    ui::warn(tr!("uninstall.aborted"));
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
                        ui::success(tr!(
                            "uninstall.removed",
                            path = removed_path.display().to_string().cyan()
                        ));
                        removed_count += 1;
                    }
                    Err(e) => {
                        ui::error(tr!(
                            "uninstall.failed_detail",
                            plugin = plugin_req,
                            error = e
                        ));
                        failed.push((plugin_req.clone(), e.to_string()));
                    }
                }
            }

            if args.plugins.len() > 1 {
                println!();
                if failed.is_empty() {
                    ui::success(tr!("uninstall.batch_success", count = removed_count));
                } else {
                    ui::warn(tr!(
                        "uninstall.batch_with_issues",
                        removed = removed_count,
                        total = args.plugins.len()
                    ));
                }
            }

            if !failed.is_empty() && args.plugins.len() == 1 {
                bail!("{}", tr!("uninstall.failed", plugin = args.plugins[0]));
            }
        }

        Commands::Installed(args) => {
            let installed = installer::scan_installed(&plugins_dir)?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&installed)?);
            } else {
                println!(
                    "{}",
                    tr!(
                        "installed.header",
                        dir = plugins_dir.display().to_string().cyan()
                    )
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
                    ui::info(tr!(
                        "update.tracked_found",
                        name = locked.name.cyan(),
                        id = locked.public_id.dimmed(),
                        version = current_ver.bright_blue()
                    ));

                    if args.check {
                        match client.check_update(&locked.name, current_ver).await {
                            Ok(info) => {
                                let latest =
                                    info.latest_version.unwrap_or_else(|| tr!("common.latest"));
                                if info.update_available {
                                    ui::info(tr!(
                                        "update.available",
                                        name = locked.name.cyan(),
                                        current = current_ver.yellow(),
                                        latest = latest.bright_green().bold()
                                    ));
                                } else {
                                    ui::info(tr!(
                                        "update.up_to_date",
                                        name = locked.name.cyan(),
                                        version = current_ver.dimmed()
                                    ));
                                }
                            }
                            Err(_) => {
                                if let Ok(meta) = client.get_plugin(&locked.public_id).await {
                                    let latest =
                                        meta.version.unwrap_or_else(|| tr!("common.latest"));
                                    ui::info(tr!(
                                        "update.latest_version",
                                        name = locked.name.cyan(),
                                        version = latest.bright_green()
                                    ));
                                } else {
                                    ui::warn(tr!("update.not_found", name = locked.name));
                                }
                            }
                        }
                        return Ok(());
                    }

                    ui::info(tr!(
                        "update.updating_with_id",
                        name = locked.name.cyan(),
                        id = locked.public_id.dimmed()
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

                    ui::success(tr!(
                        "update.updated",
                        name = outcome.display_name.bright_green(),
                        path = outcome.path.display().to_string().cyan()
                    ));
                    return Ok(());
                }

                // If not in lockfile, resolve on marketplace
                ui::info(tr!("update.not_tracked", plugin = target.cyan()));

                if args.check {
                    if let Ok(meta) = client.get_plugin(target).await {
                        let ver = meta.version.unwrap_or_else(|| tr!("common.latest"));
                        ui::info(tr!(
                            "update.latest_version",
                            name = meta.name.cyan(),
                            version = ver.bright_green()
                        ));
                    } else {
                        ui::warn(tr!("update.not_found", name = target));
                    }
                    return Ok(());
                }

                ui::info(tr!("update.updating", name = target));
                let outcome = installer::install_plugin(
                    &client,
                    target,
                    &plugins_dir,
                    None,
                    true,
                    args.token.as_deref(),
                )
                .await?;

                ui::success(tr!(
                    "update.updated",
                    name = outcome.display_name.bright_green(),
                    path = outcome.path.display().to_string().cyan()
                ));
                return Ok(());
            }

            // Case 2: Update all plugins
            // Prioritize marketplace plugins recorded in .ppm.lock
            if !lockfile.plugins.is_empty() {
                ui::info(tr!(
                    "update.checking_lockfile",
                    count = lockfile.plugins.len()
                ));

                let mut updated_count = 0;
                let total = lockfile.plugins.len();

                for (public_id, locked) in &lockfile.plugins {
                    let current_ver = locked.version.as_deref().unwrap_or("0.0.0");

                    if args.check {
                        match client.check_update(&locked.name, current_ver).await {
                            Ok(info) => {
                                let latest =
                                    info.latest_version.unwrap_or_else(|| tr!("common.latest"));
                                if info.update_available {
                                    ui::info(tr!(
                                        "update.available_with_id",
                                        name = locked.name.cyan(),
                                        id = public_id.dimmed(),
                                        current = current_ver.yellow(),
                                        latest = latest.bright_green().bold()
                                    ));
                                } else {
                                    ui::info(tr!(
                                        "update.up_to_date",
                                        name = locked.name.cyan(),
                                        version = current_ver.dimmed()
                                    ));
                                }
                            }
                            Err(_) => {
                                if let Ok(meta) = client.get_plugin(public_id).await {
                                    let latest =
                                        meta.version.unwrap_or_else(|| tr!("common.latest"));
                                    ui::info(tr!(
                                        "update.latest_version_with_id",
                                        name = locked.name.cyan(),
                                        id = public_id.dimmed(),
                                        version = latest.bright_green()
                                    ));
                                } else {
                                    ui::warn(tr!("update.query_failed", name = locked.name));
                                }
                            }
                        }
                        continue;
                    }

                    ui::info(tr!(
                        "update.updating_with_id",
                        name = locked.name.cyan(),
                        id = public_id.dimmed()
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
                            ui::success(tr!(
                                "update.updated",
                                name = outcome.display_name.bright_green(),
                                path = outcome.path.display().to_string().cyan()
                            ));
                            updated_count += 1;
                        }
                        Err(e) => {
                            ui::error(tr!("update.failed", name = locked.name, error = e));
                        }
                    }
                }

                if !args.check {
                    println!();
                    ui::success(tr!(
                        "update.batch_complete",
                        updated = updated_count,
                        total = total
                    ));
                }

                return Ok(());
            }

            // Fallback if no .ppm.lock exists: scan directory
            let installed = installer::scan_installed(&plugins_dir)?;
            if installed.is_empty() {
                ui::warn(tr!("update.none_found", dir = plugins_dir.display()));
                return Ok(());
            }

            ui::info(tr!("update.scanning_disk", count = installed.len()));

            for p in installed {
                let name = p.display_name().to_string();
                if args.check {
                    if let Ok(meta) = client.get_plugin(&name).await {
                        let ver = meta.version.unwrap_or_else(|| tr!("common.latest"));
                        ui::info(tr!(
                            "update.latest_version",
                            name = name.cyan(),
                            version = ver.bright_green()
                        ));
                    } else {
                        ui::warn(tr!("update.not_found", name = name));
                    }
                    continue;
                }

                ui::info(tr!("update.updating", name = name));
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
                        ui::success(tr!(
                            "update.updated",
                            name = outcome.display_name.bright_green(),
                            path = outcome.path.display().to_string().cyan()
                        ));
                    }
                    Err(e) => {
                        ui::error(tr!("update.failed", name = name, error = e));
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
                println!("{}\n", tr!("new.welcome").bright_cyan());
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

            ui::success(tr!(
                "new.scaffolded",
                path = project_path.display().to_string().bright_green().bold()
            ));

            println!();
            println!("{}", tr!("new.next_steps").bright_yellow().bold());
            println!("{}", tr!("new.step_cd"));
            println!("     {}", format!("cd {}", project_path.display()).cyan());
            println!("{}", tr!("new.step_readme"));
            println!("     {}", "cat README.md".cyan());
            println!("{}", tr!("new.step_build"));
            println!();
        }

        Commands::SelfInstall(args) => {
            self_install::install_self(args.dir.as_deref(), args.force)?;
        }
    }

    Ok(())
}
