// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use inquire::{Confirm, MultiSelect, Select, Text};

use super::languages::PluginLanguage;
use super::templates::{self, TemplateContext};
use crate::tr;

pub struct WizardConfig {
    pub name: Option<String>,
    pub lang: Option<PluginLanguage>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub target_dir: Option<PathBuf>,
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateFeature {
    Command,
    Event,
}

impl fmt::Display for TemplateFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command => write!(f, "{}", tr!("wizard.feature_command")),
            Self::Event => write!(f, "{}", tr!("wizard.feature_event")),
        }
    }
}

fn validate_plugin_name(input: &str) -> std::result::Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(tr!("wizard.name_empty"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(tr!("wizard.name_invalid"));
    }
    Ok(())
}

fn validate_metadata_text(input: &str, empty_key: &str) -> std::result::Result<(), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(crate::i18n::t(empty_key));
    }
    if trimmed
        .chars()
        .any(|c| c == '"' || c == '\\' || c.is_control())
    {
        return Err(tr!("wizard.special_chars_invalid"));
    }
    Ok(())
}

/// Run the plugin generation wizard (either interactive TUI or flag-driven).
pub fn run_wizard(config: WizardConfig) -> Result<PathBuf> {
    let name = match config.name {
        Some(n) if !n.trim().is_empty() => {
            if let Err(msg) = validate_plugin_name(&n) {
                bail!("{msg}");
            }
            n.trim().to_string()
        }
        _ => {
            if config.non_interactive {
                bail!("{}", tr!("wizard.name_required"));
            }
            let prompt_name = tr!("wizard.prompt_name");
            Text::new(&prompt_name)
                .with_default("my-pumpkin-plugin")
                .with_validator(|input: &str| match validate_plugin_name(input) {
                    Ok(()) => Ok(inquire::validator::Validation::Valid),
                    Err(msg) => Ok(inquire::validator::Validation::Invalid(msg.into())),
                })
                .prompt()
                .with_context(|| tr!("wizard.read_name_failed"))?
                .trim()
                .to_string()
        }
    };

    let default_author = get_default_author();
    let author = match config.author {
        Some(a) if !a.trim().is_empty() => {
            if let Err(msg) = validate_metadata_text(&a, "wizard.author_empty") {
                bail!("{msg}");
            }
            a.trim().to_string()
        }
        _ => {
            if config.non_interactive {
                default_author
            } else {
                let prompt_author = tr!("wizard.prompt_author");
                Text::new(&prompt_author)
                    .with_default(&default_author)
                    .with_validator(|input: &str| {
                        match validate_metadata_text(input, "wizard.author_empty") {
                            Ok(()) => Ok(inquire::validator::Validation::Valid),
                            Err(msg) => Ok(inquire::validator::Validation::Invalid(msg.into())),
                        }
                    })
                    .prompt()
                    .with_context(|| tr!("wizard.read_author_failed"))?
                    .trim()
                    .to_string()
            }
        }
    };

    let default_desc = tr!("wizard.default_description");
    let description = match config.description {
        Some(d) if !d.trim().is_empty() => {
            if let Err(msg) = validate_metadata_text(&d, "wizard.description_empty") {
                bail!("{msg}");
            }
            d.trim().to_string()
        }
        _ => {
            if config.non_interactive {
                default_desc
            } else {
                let prompt_desc = tr!("wizard.prompt_description");
                Text::new(&prompt_desc)
                    .with_default(&default_desc)
                    .with_validator(|input: &str| {
                        match validate_metadata_text(input, "wizard.description_empty") {
                            Ok(()) => Ok(inquire::validator::Validation::Valid),
                            Err(msg) => Ok(inquire::validator::Validation::Invalid(msg.into())),
                        }
                    })
                    .prompt()
                    .with_context(|| tr!("wizard.read_description_failed"))?
                    .trim()
                    .to_string()
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
                let prompt_lang = tr!("wizard.prompt_language");
                Select::new(&prompt_lang, options)
                    .prompt()
                    .with_context(|| tr!("wizard.select_language_failed"))?
            }
        }
    };

    let (include_command, include_event) = if config.non_interactive {
        (true, true)
    } else {
        let features = vec![TemplateFeature::Command, TemplateFeature::Event];
        let prompt_features = tr!("wizard.prompt_features");
        let selected = MultiSelect::new(&prompt_features, features)
            .with_default(&[0, 1])
            .prompt()
            .unwrap_or_default();

        (
            selected.contains(&TemplateFeature::Command),
            selected.contains(&TemplateFeature::Event),
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
                    "{}",
                    tr!("wizard.dir_not_empty_error", path = target_dir.display())
                );
            }

            let confirm_msg = tr!("wizard.dir_not_empty_confirm", path = target_dir.display());
            let proceed = Confirm::new(&confirm_msg)
                .with_default(false)
                .prompt()
                .with_context(|| tr!("wizard.prompt_failed"))?;

            if !proceed {
                bail!("{}", tr!("wizard.cancelled"));
            }
        }
    } else {
        fs::create_dir_all(&target_dir)
            .with_context(|| tr!("wizard.create_dir_failed", path = target_dir.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plugin_name() {
        assert!(validate_plugin_name("my-plugin_1").is_ok());
        assert!(validate_plugin_name("  ").is_err());
        assert!(validate_plugin_name("../evil").is_err());
        assert!(validate_plugin_name("bad/name").is_err());
        assert!(validate_plugin_name("has space").is_err());
    }

    #[test]
    fn test_validate_metadata_text_rejects_embedded_quotes_and_escapes() {
        assert!(validate_metadata_text("Normal Author", "wizard.author_empty").is_ok());
        assert!(validate_metadata_text("  ", "wizard.author_empty").is_err());
        assert!(validate_metadata_text("Evil\"Author", "wizard.author_empty").is_err());
        assert!(validate_metadata_text("Back\\slash", "wizard.description_empty").is_err());
        assert!(validate_metadata_text("Line\nbreak", "wizard.description_empty").is_err());
    }
}
