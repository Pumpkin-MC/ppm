// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use colored::Colorize;
use comfy_table::presets;
use comfy_table::{Cell, CellAlignment, Color, ContentArrangement, Table};

use crate::installer::InstalledPlugin;
use crate::market::ExternalPluginMetadata;

/// Prints the stylized Pumpkin Package Manager banner.
pub fn print_logo() {
    println!(
        "{}",
        r#"
   ____  ____  ____ ___
  / __ \/ __ \/ __ `__ \
 / /_/ / /_/ / / / / / /
/ .___/ .___/_/ /_/ /_/
/_/   /_/   Pumpkin Package Manager
"#
        .bright_yellow()
        .bold()
    );
}

/// Print a table of plugins from the marketplace.
pub fn print_plugins_table(plugins: &[ExternalPluginMetadata]) {
    if plugins.is_empty() {
        println!("{}", "No plugins found matching criteria.".yellow());
        return;
    }

    let mut table = Table::new();
    table
        .load_style(presets::UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("ID").fg(Color::DarkGrey),
            Cell::new("Name").fg(Color::Yellow),
            Cell::new("Version").fg(Color::Cyan),
            Cell::new("Category").fg(Color::Magenta),
            Cell::new("Author").fg(Color::White),
            Cell::new("Type").fg(Color::Green),
            Cell::new("Downloads").fg(Color::Blue),
        ]);

    for p in plugins {
        let version_str = match &p.version {
            Some(v) if !v.trim().is_empty() => v.as_str(),
            _ => "-",
        };

        let category_str = match &p.category {
            Some(c) if !c.trim().is_empty() => c.as_str(),
            _ => "Other",
        };

        let type_cell = match p.type_.as_str() {
            "free" => Cell::new("Free").fg(Color::Green),
            "paid" => Cell::new(format!("${:.2}", p.price)).fg(Color::Yellow),
            "adwall" => Cell::new("Ad-supported").fg(Color::Cyan),
            other => Cell::new(other),
        };

        table.add_row(vec![
            Cell::new(&p.public_id).fg(Color::DarkGrey),
            Cell::new(&p.name).fg(Color::White),
            Cell::new(version_str).fg(Color::Cyan),
            Cell::new(category_str).fg(Color::Magenta),
            Cell::new(&p.dev_name).fg(Color::White),
            type_cell,
            Cell::new(p.downloads.to_string())
                .set_alignment(CellAlignment::Right)
                .fg(Color::Blue),
        ]);
    }

    println!("{table}");
    println!(
        "Total results: {} | Run '{}' for details",
        plugins.len().to_string().bold(),
        "ppm info <NAME>".cyan()
    );
}

/// Print comprehensive details of a single plugin.
pub fn print_plugin_details(p: &ExternalPluginMetadata) {
    let version_str = match &p.version {
        Some(v) if !v.trim().is_empty() => v.as_str(),
        _ => "Latest / Unspecified",
    };

    let category_str = match &p.category {
        Some(c) if !c.trim().is_empty() => c.as_str(),
        _ => "Other",
    };

    let type_str = match p.type_.as_str() {
        "free" => "Free".green().bold(),
        "paid" => format!("Paid (${:.2})", p.price).yellow().bold(),
        "adwall" => "Ad-Supported".cyan().bold(),
        other => other.white().bold(),
    };

    println!();
    println!("  {}", format!("📦 {}", p.name).bright_yellow().bold());
    println!("  {}", "─".repeat(50).dimmed());
    println!("  {:<16} {}", "Version:".bold(), version_str.cyan());
    println!("  {:<16} {}", "Author:".bold(), p.dev_name.white());
    println!("  {:<16} {}", "Category:".bold(), category_str.magenta());
    println!("  {:<16} {}", "Type:".bold(), type_str);
    println!(
        "  {:<16} {}",
        "Downloads:".bold(),
        p.downloads.to_string().blue()
    );
    println!("  {:<16} {}", "Public ID:".bold(), p.public_id.dimmed());
    println!(
        "  {:<16} {}",
        "Database ID:".bold(),
        p.id.to_string().dimmed()
    );
    println!("  {:<16} {}", "Last Updated:".bold(), p.updated_at.dimmed());

    if let Some(preview) = &p.preview_url {
        println!("  {:<16} {}", "Preview:".bold(), preview.underline().blue());
    }

    println!("  {}", "─".repeat(50).dimmed());
    println!(
        "  Install command: {}",
        format!("ppm install {}", p.name).bright_cyan().bold()
    );
    println!();
}

/// Print local installed plugins.
pub fn print_installed_table(plugins: &[InstalledPlugin]) {
    if plugins.is_empty() {
        println!("{}", "No plugins installed in this directory.".yellow());
        println!(
            "Tip: Search plugins with '{}' or install with '{}'",
            "ppm search <QUERY>".cyan(),
            "ppm install <PLUGIN>".cyan()
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_style(presets::UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Plugin File").fg(Color::Yellow),
            Cell::new("Status").fg(Color::Green),
            Cell::new("Version").fg(Color::Cyan),
            Cell::new("Author").fg(Color::White),
            Cell::new("Market ID").fg(Color::DarkGrey),
            Cell::new("Size").fg(Color::Blue),
            Cell::new("Modified").fg(Color::Cyan),
        ]);

    for p in plugins {
        let status = if p.is_active {
            if p.is_managed {
                Cell::new("Active").fg(Color::Green)
            } else {
                Cell::new("Active (unmanaged)").fg(Color::DarkGreen)
            }
        } else {
            Cell::new("Deactivated").fg(Color::DarkGrey)
        };

        let ver = p.version.as_deref().unwrap_or("-");
        let author = p.author.as_deref().unwrap_or("-");
        let id = p.public_id.as_deref().unwrap_or("-");

        table.add_row(vec![
            Cell::new(&p.filename).fg(Color::White),
            status,
            Cell::new(ver).fg(Color::Cyan),
            Cell::new(author).fg(Color::White),
            Cell::new(id).fg(Color::DarkGrey),
            Cell::new(p.formatted_size())
                .set_alignment(CellAlignment::Right)
                .fg(Color::Blue),
            Cell::new(p.formatted_modified()).fg(Color::Cyan),
        ]);
    }

    println!("{table}");
    println!("Total installed: {}", plugins.len().to_string().bold());
}

pub fn success(msg: impl std::fmt::Display) {
    println!("{} {}", "✔".bright_green().bold(), msg);
}

pub fn info(msg: impl std::fmt::Display) {
    println!("{} {}", "ℹ".bright_blue().bold(), msg);
}

pub fn warn(msg: impl std::fmt::Display) {
    println!("{} {}", "⚠".bright_yellow().bold(), msg);
}

pub fn error(msg: impl std::fmt::Display) {
    eprintln!("{} {}", "✖".bright_red().bold(), msg);
}
