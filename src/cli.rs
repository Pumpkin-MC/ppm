// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "ppm",
    author = "Alexander Medvedev <lilalexmed@proton.me> & Pumpkin-MC Contributors",
    version,
    about = "Pumpkin Package Manager - Discover, download, and scaffold plugins for Pumpkin Minecraft server",
    long_about = "ppm is the official CLI tool and package manager for Pumpkin Minecraft server plugins.\n\
                  It allows searching and downloading WASM plugins from the Pumpkin Marketplace,\n\
                  managing locally installed plugins, and bootstrapping new plugins in Rust, TypeScript, Python, Go, C#, or C."
)]
pub struct Cli {
    /// Pumpkin Marketplace API base URL
    #[arg(
        long,
        global = true,
        env = "PUMPKIN_MARKET_URL",
        default_value = "https://market.pumpkinmc.org"
    )]
    pub market_url: String,

    /// Directory where plugins are installed
    #[arg(
        long,
        global = true,
        env = "PUMPKIN_PLUGINS_DIR",
        default_value = "./plugins"
    )]
    pub plugins_dir: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search plugins on the Pumpkin Marketplace
    #[command(visible_alias = "find")]
    Search(SearchArgs),

    /// List recent or popular plugins from the marketplace
    List(ListArgs),

    /// View detailed information about a plugin
    #[command(visible_alias = "show")]
    Info(InfoArgs),

    /// Download and install a plugin (.wasm) into the plugins directory
    #[command(visible_alias = "add", visible_alias = "get")]
    Install(InstallArgs),

    /// Remove an installed plugin from the plugins directory
    #[command(visible_alias = "remove", visible_alias = "rm")]
    Uninstall(UninstallArgs),

    /// List plugins currently installed in the local plugins directory
    #[command(visible_alias = "list-installed", visible_alias = "ls")]
    Installed(InstalledArgs),

    /// Update installed plugin(s) to the latest version from the marketplace
    #[command(visible_alias = "upgrade")]
    Update(UpdateArgs),

    /// Scaffold a new plugin project template (interactive TUI or flag-driven)
    #[command(visible_alias = "init", visible_alias = "create")]
    New(NewArgs),
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Search query (plugin name, keyword, or author)
    pub query: String,

    /// Filter by category (e.g., Utilities, Fun, Admin Tools, Economy)
    #[arg(short, long)]
    pub category: Option<String>,

    /// Filter by plugin type (free, paid, adwall)
    #[arg(short = 't', long = "type")]
    pub type_: Option<String>,

    /// Maximum number of plugins to return
    #[arg(short, long, default_value_t = 20)]
    pub limit: i64,

    /// Page number (starting at 1)
    #[arg(short, long, default_value_t = 1)]
    pub page: i64,

    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Filter by category
    #[arg(short, long)]
    pub category: Option<String>,

    /// Filter by plugin type (free, paid, adwall)
    #[arg(short = 't', long = "type")]
    pub type_: Option<String>,

    /// Maximum number of plugins to return
    #[arg(short, long, default_value_t = 20)]
    pub limit: i64,

    /// Page number (starting at 1)
    #[arg(short, long, default_value_t = 1)]
    pub page: i64,

    /// Output results in JSON format
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Plugin name, database ID, or public ID
    pub plugin: String,

    /// Output plugin details as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Plugin name, database ID, or public ID to install
    pub plugin: String,

    /// Custom output file name or path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Overwrite existing plugin file if already present
    #[arg(short, long)]
    pub force: bool,

    /// Account access token for paid or private marketplace plugins
    #[arg(long, env = "PPM_TOKEN")]
    pub token: Option<String>,
}

#[derive(Args, Debug)]
pub struct UninstallArgs {
    /// Plugin name or filename to remove
    pub plugin: String,

    /// Skip confirmation prompt
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct InstalledArgs {
    /// Output installed plugins as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Specific plugin name or ID to update (updates all installed plugins if omitted)
    pub plugin: Option<String>,

    /// Only check for updates without downloading
    #[arg(short = 'c', long = "check")]
    pub check: bool,

    /// Account access token for paid plugins
    #[arg(long, env = "PPM_TOKEN")]
    pub token: Option<String>,
}

#[derive(Args, Debug)]
pub struct NewArgs {
    /// Plugin name (e.g. my-plugin)
    pub name: Option<String>,

    /// Programming language: rust, typescript (ts), python (py), go, csharp (cs), c
    #[arg(short, long)]
    pub lang: Option<String>,

    /// Author name
    #[arg(short, long)]
    pub author: Option<String>,

    /// Description of the plugin
    #[arg(long)]
    pub description: Option<String>,

    /// Target directory to generate the project in
    #[arg(short = 'd', long = "dir")]
    pub dir: Option<PathBuf>,

    /// Do not prompt interactively; use defaults for unspecified options
    #[arg(short = 'y', long = "non-interactive")]
    pub non_interactive: bool,
}
