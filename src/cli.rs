// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::{Args, Command, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::tr;

mod i18n_output;

#[derive(Parser, Debug)]
#[command(
    name = "ppm",
    author = "Alexander Medvedev <lilalexmed@proton.me> & Pumpkin-MC Contributors",
    version,
    about = "Pumpkin Package Manager - Discover, download, and scaffold plugins for Pumpkin Minecraft server",
    long_about = "ppm is the official CLI tool and package manager for Pumpkin Minecraft server plugins.\n\
                  It allows searching and downloading WASM plugins from the Pumpkin Marketplace,\n\
                  managing locally installed plugins, and bootstrapping new plugins in Rust, Python, C#, C, Go, Kotlin, D, Zig, or TypeScript."
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

    /// Directory where plugins are installed (auto-detected if inside server root or plugins folder)
    #[arg(long, global = true, env = "PUMPKIN_PLUGINS_DIR")]
    pub plugins_dir: Option<PathBuf>,

    /// Override display language/locale (e.g. en_US, zh_CN, zh_TW); auto-detected from system if omitted
    #[arg(long, global = true, env = "PPM_LOCALE")]
    pub locale: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Parses CLI arguments, exiting with localized help or error output.
    pub fn parse_localized() -> Self {
        let mut command = localized_command();
        match command.try_get_matches_from_mut(std::env::args_os()) {
            Ok(matches) => Self::from_arg_matches(&matches).unwrap_or_else(|e| e.exit()),
            Err(error) => {
                let to_stderr = error.use_stderr();
                let styled = should_style_stream(to_stderr);
                let rendered = i18n_output::render(&error, &command, styled);
                exit_with(&rendered, to_stderr, error.exit_code())
            }
        }
    }

    /// Resolves the effective plugins directory.
    ///
    /// - If explicitly set via CLI or `PUMPKIN_PLUGINS_DIR`, uses that value.
    /// - If executed inside a `plugins/` directory, uses `.` (the current directory).
    /// - If executed inside a Pumpkin server root (with `pumpkin.toml` or `plugins/`), uses `./plugins`.
    /// - Otherwise defaults to `./plugins`.
    pub fn resolved_plugins_dir(&self) -> PathBuf {
        resolve_plugins_dir(self.plugins_dir.as_deref())
    }
}

/// Pre-scans raw CLI arguments for `--locale <VALUE>` or `--locale=<VALUE>` so that
/// i18n can be initialized before `clap` renders `--help` or validation messages.
pub fn extract_locale_from_args<I, T>(args: I) -> Option<String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut found = None;
    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        let s = arg.into();
        let Some(text) = s.to_str() else {
            continue;
        };
        if text == "--" {
            break;
        }
        if text == "--locale" {
            if let Some(val) = iter.next()
                && let Some(val_str) = val.into().to_str()
            {
                found = Some(val_str.to_string());
            }
        } else if let Some(val) = text.strip_prefix("--locale=") {
            found = Some(val.to_string());
        }
    }
    found
}

/// Builds the `clap::Command` for `ppm` with localized descriptions and help strings.
pub fn localized_command() -> Command {
    let command = Cli::command()
        .about(tr!("cli.about"))
        .long_about(tr!("cli.long_about"))
        .mut_arg("market_url", |a| a.help(tr!("cli.arg.market_url")))
        .mut_arg("plugins_dir", |a| a.help(tr!("cli.arg.plugins_dir")))
        .mut_arg("locale", |a| {
            a.help(tr!(
                "cli.arg.locale",
                locales = crate::i18n::available_locales().join(", ")
            ))
        })
        .mut_subcommand("search", |sub| {
            sub.about(tr!("cli.cmd.search"))
                .mut_arg("query", |a| a.help(tr!("cli.search.query")))
                .mut_arg("category", |a| a.help(tr!("cli.search.category")))
                .mut_arg("type_", |a| a.help(tr!("cli.search.type")))
                .mut_arg("limit", |a| a.help(tr!("cli.search.limit")))
                .mut_arg("page", |a| a.help(tr!("cli.search.page")))
                .mut_arg("json", |a| a.help(tr!("cli.search.json")))
        })
        .mut_subcommand("info", |sub| {
            sub.about(tr!("cli.cmd.info"))
                .mut_arg("plugin", |a| a.help(tr!("cli.info.plugin")))
                .mut_arg("json", |a| a.help(tr!("cli.info.json")))
        })
        .mut_subcommand("install", |sub| {
            sub.about(tr!("cli.cmd.install"))
                .mut_arg("plugins", |a| a.help(tr!("cli.install.plugins")))
                .mut_arg("output", |a| a.help(tr!("cli.install.output")))
                .mut_arg("force", |a| a.help(tr!("cli.install.force")))
                .mut_arg("token", |a| a.help(tr!("cli.install.token")))
        })
        .mut_subcommand("uninstall", |sub| {
            sub.about(tr!("cli.cmd.uninstall"))
                .mut_arg("plugins", |a| a.help(tr!("cli.uninstall.plugins")))
                .mut_arg("yes", |a| a.help(tr!("cli.uninstall.yes")))
        })
        .mut_subcommand("installed", |sub| {
            sub.about(tr!("cli.cmd.installed"))
                .mut_arg("json", |a| a.help(tr!("cli.installed.json")))
        })
        .mut_subcommand("update", |sub| {
            sub.about(tr!("cli.cmd.update"))
                .mut_arg("plugin", |a| a.help(tr!("cli.update.plugin")))
                .mut_arg("check", |a| a.help(tr!("cli.update.check")))
                .mut_arg("token", |a| a.help(tr!("cli.update.token")))
        })
        .mut_subcommand("new", |sub| {
            sub.about(tr!("cli.cmd.new"))
                .mut_arg("name", |a| a.help(tr!("cli.new.name")))
                .mut_arg("lang", |a| a.help(tr!("cli.new.lang")))
                .mut_arg("author", |a| a.help(tr!("cli.new.author")))
                .mut_arg("description", |a| a.help(tr!("cli.new.description")))
                .mut_arg("dir", |a| a.help(tr!("cli.new.dir")))
                .mut_arg("non_interactive", |a| {
                    a.help(tr!("cli.new.non_interactive"))
                })
        })
        .mut_subcommand("self-install", |sub| {
            sub.about(tr!("cli.cmd.self_install"))
                .mut_arg("dir", |a| a.help(tr!("cli.self_install.dir")))
                .mut_arg("force", |a| a.help(tr!("cli.self_install.force")))
        });

    localize_builtin_args(command)
}

fn localized_help_arg() -> clap::Arg {
    clap::Arg::new("help")
        .short('h')
        .long("help")
        .action(clap::ArgAction::Help)
        .help(tr!("cli.arg.help"))
        .long_help(tr!("cli.arg.help_long"))
}

fn localized_version_arg() -> clap::Arg {
    clap::Arg::new("version")
        .short('V')
        .long("version")
        .action(clap::ArgAction::Version)
        .help(tr!("cli.arg.version"))
}

/// Replaces the descriptions `clap` injects for its own `--help`, `--version`
/// and `help` entries.
///
/// clap adds those arguments while building the command tree, after all
/// `mut_arg` calls have run, so their help text can only be set by inserting
/// our own arguments and switching the built-in ones off.
fn localize_builtin_args(mut command: Command) -> Command {
    command = command
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(localized_help_arg())
        .arg(localized_version_arg());

    // Subcommands carry their own copy of the help/version flags.
    let names: Vec<String> = command
        .get_subcommands()
        .filter(|subcommand| subcommand.get_name() != "help")
        .map(|subcommand| subcommand.get_name().to_string())
        .collect();

    for name in names {
        command = command.mut_subcommand(name, localize_subcommand_builtins);
    }

    command
}

/// Adds the translated `-h`/`--help` flags to a subcommand.
fn localize_subcommand_builtins(subcommand: Command) -> Command {
    subcommand
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(localized_help_arg())
}

/// Decides whether `clap`'s ANSI styling should be emitted on stdout/stderr,
/// honoring `CLICOLOR_FORCE`, `NO_COLOR`, `CLICOLOR`, `TERM=dumb`, and TTY status.
fn should_style_stream(to_stderr: bool) -> bool {
    use std::io::IsTerminal;

    let is_tty = if to_stderr {
        std::io::stderr().is_terminal()
    } else {
        std::io::stdout().is_terminal()
    };

    should_style_with_env(is_tty, |name| std::env::var(name).ok())
}

fn should_style_with_env(is_tty: bool, env_get: impl Fn(&str) -> Option<String>) -> bool {
    if env_get("CLICOLOR_FORCE").is_some_and(|v| !v.is_empty() && v != "0") {
        return true;
    }
    if env_get("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    if env_get("CLICOLOR").as_deref() == Some("0") {
        return false;
    }
    if env_get("TERM").as_deref() == Some("dumb") {
        return false;
    }
    is_tty
}

/// Writes output to the stream `clap` would have used and exits with `code`.
fn exit_with(text: &str, to_stderr: bool, code: i32) -> ! {
    if to_stderr {
        let mut stderr = std::io::stderr();
        let _ = std::io::Write::write_all(&mut stderr, text.as_bytes());
        let _ = std::io::Write::flush(&mut stderr);
    } else {
        let mut stdout = std::io::stdout();
        let _ = std::io::Write::write_all(&mut stdout, text.as_bytes());
        let _ = std::io::Write::flush(&mut stdout);
    }
    std::process::exit(code)
}

/// Automatically resolves the target plugins directory based on current working directory context.
pub fn resolve_plugins_dir(explicit: Option<&Path>) -> PathBuf {
    if let Some(dir) = explicit {
        return dir.to_path_buf();
    }

    if let Ok(env_dir) = std::env::var("PUMPKIN_PLUGINS_DIR")
        && !env_dir.trim().is_empty()
    {
        return PathBuf::from(env_dir);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Check if we are directly inside the plugins directory
    let is_plugins_dir_name = cwd
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("plugins"))
        .unwrap_or(false);

    let parent_is_pumpkin_root = cwd
        .parent()
        .map(|p| p.join("pumpkin.toml").exists())
        .unwrap_or(false);

    if is_plugins_dir_name || parent_is_pumpkin_root {
        // We are already inside the plugins folder
        return PathBuf::from(".");
    }

    // Check if we are in pumpkin root directory
    if cwd.join("pumpkin.toml").exists() || cwd.join("plugins").is_dir() {
        return PathBuf::from("./plugins");
    }

    // Default fallback
    PathBuf::from("./plugins")
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Search plugins on the Pumpkin Marketplace
    #[command(visible_alias = "find")]
    Search(SearchArgs),

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

    /// Install ppm into your user/system PATH (~/.local/bin)
    #[command(visible_alias = "setup")]
    SelfInstall(SelfInstallArgs),
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
pub struct InfoArgs {
    /// Plugin name, database ID, public ID, author/plugin, or marketplace URL
    pub plugin: String,

    /// Output plugin details as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Plugin name(s), database ID(s), public ID(s), author/plugin, marketplace URL(s), or direct .wasm URL(s).
    /// If omitted, restores all marketplace plugins from .ppm.lock
    #[arg(num_args = 0..)]
    pub plugins: Vec<String>,

    /// Custom output file name or path (only valid when installing a single plugin)
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
    /// Plugin name(s), filename(s), ID(s), or marketplace URL(s) to remove
    #[arg(required = true, num_args = 1..)]
    pub plugins: Vec<String>,

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

    /// Programming language: rust, python (py), csharp (cs), c, go, kotlin (kt), d, zig, typescript (ts)
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

#[derive(Args, Debug)]
pub struct SelfInstallArgs {
    /// Custom target directory (defaults to ~/.local/bin)
    #[arg(short, long)]
    pub dir: Option<PathBuf>,

    /// Force overwrite existing binary if already present
    #[arg(short, long)]
    pub force: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_plugins_dir_explicit() {
        let explicit = Path::new("/custom/plugins/path");
        let resolved = resolve_plugins_dir(Some(explicit));
        assert_eq!(resolved, PathBuf::from("/custom/plugins/path"));
    }

    #[test]
    fn test_localized_command_valid() {
        localized_command().debug_assert();
    }

    #[test]
    fn test_extract_locale_from_args() {
        assert_eq!(
            extract_locale_from_args(["ppm", "--locale", "zh_CN", "installed"]),
            Some("zh_CN".into())
        );
        assert_eq!(
            extract_locale_from_args(["ppm", "search", "--locale=zh_TW", "foo"]),
            Some("zh_TW".into())
        );
        assert_eq!(
            extract_locale_from_args(["ppm", "search", "--", "--locale", "zh_CN"]),
            None
        );
        assert_eq!(extract_locale_from_args(["ppm", "installed"]), None);
    }

    #[test]
    fn test_should_style_with_env() {
        assert!(should_style_with_env(true, |_| None));
        assert!(!should_style_with_env(false, |_| None));

        // NO_COLOR disables ANSI even on a terminal (empty NO_COLOR is ignored per spec).
        assert!(!should_style_with_env(true, |k| {
            (k == "NO_COLOR").then(|| "1".into())
        }));
        assert!(should_style_with_env(true, |k| {
            (k == "NO_COLOR").then(String::new)
        }));

        // CLICOLOR=0 and TERM=dumb disable ANSI on a terminal.
        assert!(!should_style_with_env(true, |k| {
            (k == "CLICOLOR").then(|| "0".into())
        }));
        assert!(!should_style_with_env(true, |k| {
            (k == "TERM").then(|| "dumb".into())
        }));

        // CLICOLOR_FORCE forces ANSI even when piped.
        assert!(should_style_with_env(false, |k| {
            (k == "CLICOLOR_FORCE").then(|| "1".into())
        }));
        assert!(!should_style_with_env(false, |k| {
            (k == "CLICOLOR_FORCE").then(|| "0".into())
        }));
    }
}
