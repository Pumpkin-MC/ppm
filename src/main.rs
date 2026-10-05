// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

mod cli;
mod installer;
mod market;
mod template;
mod ui;

use std::str::FromStr;

use anyhow::Result;
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
    let client = MarketClient::new(cli.market_url);

    match cli.command {
        Commands::Search(args) => {
            let params = ListPluginsParams {
                q: Some(args.query.clone()),
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

        Commands::List(args) => {
            let params = ListPluginsParams {
                q: None,
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
            ui::info(format!(
                "Fetching and installing '{}' from marketplace...",
                args.plugin.cyan()
            ));

            let installed_path = installer::install_plugin(
                &client,
                &args.plugin,
                &cli.plugins_dir,
                args.output.as_deref(),
                args.force,
                args.token.as_deref(),
            )
            .await?;

            let file_size = std::fs::metadata(&installed_path)
                .map(|m| installer::format_size(m.len()))
                .unwrap_or_else(|_| "unknown size".into());

            ui::success(format!(
                "Successfully installed '{}' ({}) to {}",
                args.plugin.bright_green().bold(),
                file_size.bright_blue(),
                installed_path.display().to_string().cyan()
            ));
            println!(
                "  {}",
                "Your Pumpkin server will load this plugin on next startup or reload."
                    .dimmed()
            );
        }

        Commands::Uninstall(args) => {
            if !args.yes {
                let confirm = Confirm::new(&format!(
                    "Are you sure you want to remove plugin '{}'?",
                    args.plugin
                ))
                .with_default(false)
                .prompt()?;

                if !confirm {
                    ui::warn("Plugin removal aborted.");
                    return Ok(());
                }
            }

            let removed_path = installer::remove_plugin(&cli.plugins_dir, &args.plugin)?;
            ui::success(format!(
                "Removed plugin file: {}",
                removed_path.display().to_string().cyan()
            ));
        }

        Commands::Installed(args) => {
            let installed = installer::scan_installed(&cli.plugins_dir)?;

            if args.json {
                println!("{}", serde_json::to_string_pretty(&installed)?);
            } else {
                println!(
                    "Installed plugins in '{}':",
                    cli.plugins_dir.display().to_string().cyan()
                );
                ui::print_installed_table(&installed);
            }
        }

        Commands::Update(args) => {
            let plugins_to_update = match args.plugin {
                Some(p) => vec![p],
                None => {
                    let installed = installer::scan_installed(&cli.plugins_dir)?;
                    if installed.is_empty() {
                        ui::warn(format!(
                            "No plugins installed in '{}'. Nothing to update.",
                            cli.plugins_dir.display()
                        ));
                        return Ok(());
                    }
                    installed
                        .into_iter()
                        .map(|p| p.display_name().to_string())
                        .collect()
                }
            };

            for plugin_name in plugins_to_update {
                if args.check {
                    match client.check_update(&plugin_name, "0.0.0").await {
                        Ok(info) => {
                            let latest = info.latest_version.unwrap_or_else(|| "latest".into());
                            ui::info(format!(
                                "Plugin '{}': latest version on marketplace is {}",
                                plugin_name.cyan(),
                                latest.bright_green()
                            ));
                        }
                        Err(_) => {
                            if let Ok(meta) = client.get_plugin(&plugin_name).await {
                                let ver = meta.version.unwrap_or_else(|| "latest".into());
                                ui::info(format!(
                                    "Plugin '{}': latest version on marketplace is {}",
                                    plugin_name.cyan(),
                                    ver.bright_green()
                                ));
                            } else {
                                ui::warn(format!("Plugin '{}' not found on marketplace.", plugin_name));
                            }
                        }
                    }
                    continue;
                }

                ui::info(format!(
                    "Checking and updating '{}'...",
                    plugin_name.cyan()
                ));

                match installer::install_plugin(
                    &client,
                    &plugin_name,
                    &cli.plugins_dir,
                    None,
                    true,
                    args.token.as_deref(),
                )
                .await
                {
                    Ok(path) => {
                        ui::success(format!(
                            "Updated '{}' -> {}",
                            plugin_name.bright_green(),
                            path.display().to_string().cyan()
                        ));
                    }
                    Err(e) => {
                        ui::error(format!("Failed to update '{}': {e}", plugin_name));
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
            println!("  3. Build your WebAssembly plugin and copy to Pumpkin's plugins/ directory!");
            println!();
        }
    }

    Ok(())
}
