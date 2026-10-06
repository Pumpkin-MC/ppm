// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use inquire::{Confirm, MultiSelect, Select, Text};

use super::languages::PluginLanguage;
use super::templates::{self, TemplateContext};

pub struct WizardConfig {
    pub name: Option<String>,
    pub lang: Option<PluginLanguage>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub target_dir: Option<PathBuf>,
    pub non_interactive: bool,
}

/// Run the plugin generation wizard (either interactive TUI or flag-driven).
pub fn run_wizard(config: WizardConfig) -> Result<PathBuf> {
    // Create validator to reuse in each scene
    // TODO: Optimize for each scene
    let validator = |input: &str| {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(inquire::validator::Validation::Invalid(
                "Input content cannot be empty".into(),
            ));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Ok(inquire::validator::Validation::Invalid(
                "Input content must only contain alphanumeric characters, hyphens, and underscores".into(),
            ));
        }
        Ok(inquire::validator::Validation::Valid)
    };

    let name = match config.name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            if config.non_interactive {
                bail!("Plugin name is required in non-interactive mode. Usage: ppm new <NAME>");
            }
            Text::new("Plugin Name:")
                .with_default("my-pumpkin-plugin")
                .with_validator(validator)
                .prompt()
                .context("Failed to read plugin name")?
        }
    };

    let default_author = get_default_author();
    let author = match config.author {
        Some(a) if !a.trim().is_empty() => a.trim().to_string(),
        _ => {
            if config.non_interactive {
                default_author
            } else {
                Text::new("Author:")
                    .with_default(&default_author)
                    .with_validator(validator)
                    .prompt()
                    .context("Failed to read author")?
            }
        }
    };

    let default_desc = "A Pumpkin server plugin".to_string();
    let description = match config.description {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => {
            if config.non_interactive {
                default_desc
            } else {
                Text::new("Description:")
                    .with_default(&default_desc)
                    .with_validator(validator)
                    .prompt()
                    .context("Failed to read description")?
            }
        }
    };

    let lang = match config.lang {
        Some(l) => l,
        None => {
            if config.non_interactive {
                PluginLanguage::Rust
            } else {
                let options = PluginLanguage::ALL.to_vec();
                Select::new("Select programming language:", options)
                    .prompt()
                    .context("Failed to select language")?
            }
        }
    };

    let (include_command, include_event) = if config.non_interactive {
        (true, true)
    } else {
        let features = vec![
            "Sample custom command handler",
            "Sample player join event listener",
        ];
        let selected = MultiSelect::new("Select template features to include:", features)
            .with_default(&[0, 1])
            .prompt()
            .unwrap_or_else(|_| vec![]);

        (
            selected.iter().any(|s| s.contains("command")),
            selected.iter().any(|s| s.contains("join")),
        )
    };

    let target_dir = match config.target_dir {
        Some(d) => d,
        None => PathBuf::from(&name),
    };

    if target_dir.exists() {
        let is_empty = fs::read_dir(&target_dir)
            .map(|mut i| i.next().is_none())
            .unwrap_or(false);

        if !is_empty {
            if config.non_interactive {
                bail!(
                    "Target directory '{}' already exists and is not empty.",
                    target_dir.display()
                );
            }

            let proceed = Confirm::new(&format!(
                "Directory '{}' is not empty. Continue and possibly overwrite files?",
                target_dir.display()
            ))
            .with_default(false)
            .prompt()
            .context("Prompt failed")?;

            if !proceed {
                bail!("Plugin creation cancelled by user.");
            }
        }
    } else {
        fs::create_dir_all(&target_dir)
            .with_context(|| format!("Failed to create directory '{}'", target_dir.display()))?;
    }

    let ctx = TemplateContext {
        name: name.clone(),
        author,
        description,
        include_command,
        include_event,
    };

    match lang {
        PluginLanguage::Rust => templates::rust::generate(&target_dir, &ctx)?,
        PluginLanguage::TypeScript => templates::typescript::generate(&target_dir, &ctx)?,
        PluginLanguage::Python => templates::python::generate(&target_dir, &ctx)?,
        PluginLanguage::CSharp => templates::csharp::generate(&target_dir, &ctx)?,
        PluginLanguage::C => templates::c::generate(&target_dir, &ctx)?,
        PluginLanguage::Go => templates::go::generate(&target_dir, &ctx)?,
        PluginLanguage::Kotlin => templates::kotlin::generate(&target_dir, &ctx)?,
        PluginLanguage::D => templates::d::generate(&target_dir, &ctx)?,
        PluginLanguage::Zig => templates::zig::generate(&target_dir, &ctx)?,
    }

    Ok(target_dir)
}

fn get_default_author() -> String {
    let git_user = Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !name.is_empty() { Some(name) } else { None }
            } else {
                None
            }
        });

    if let Some(user) = git_user {
        return user;
    }

    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "Pumpkin Developer".to_string())
}
