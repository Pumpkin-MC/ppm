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
use crate::tr;

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
        println!("{}", tr!("ui.no_plugins_found").yellow());
        return;
    }

    let mut table = Table::new();
    table
        .load_style(presets::UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(tr!("ui.table.id")).fg(Color::DarkGrey),
            Cell::new(tr!("ui.table.name")).fg(Color::Yellow),
            Cell::new(tr!("ui.table.version")).fg(Color::Cyan),
            Cell::new(tr!("ui.table.category")).fg(Color::Magenta),
            Cell::new(tr!("ui.table.author")).fg(Color::White),
            Cell::new(tr!("ui.table.type")).fg(Color::Green),
            Cell::new(tr!("ui.table.downloads")).fg(Color::Blue),
        ]);

    let other_category = tr!("ui.category.other");

    for p in plugins {
        let version_str = match &p.version {
            Some(v) if !v.trim().is_empty() => v.as_str(),
            _ => "-",
        };

        let category_str = match &p.category {
            Some(c) if !c.trim().is_empty() => c.as_str(),
            _ => other_category.as_str(),
        };

        let type_cell = match p.type_.as_str() {
            "free" => Cell::new(tr!("ui.type.free")).fg(Color::Green),
            "paid" => Cell::new(format!("${:.2}", p.price)).fg(Color::Yellow),
            "adwall" => Cell::new(tr!("ui.type.ad_supported")).fg(Color::Cyan),
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
        "{}",
        tr!(
            "ui.total_results",
            count = plugins.len().to_string().bold(),
            cmd = "ppm info <NAME>".cyan()
        )
    );
}

/// Pads a label to a target display column width (accounting for wide CJK characters).
fn pad_label(label: &str, target_width: usize) -> String {
    let width: usize = label
        .chars()
        .map(|c| if is_wide_char(c) { 2 } else { 1 })
        .sum();
    if width >= target_width {
        label.to_string()
    } else {
        format!("{label}{}", " ".repeat(target_width - width))
    }
}

fn is_wide_char(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x2FFFD
            | 0x30000..=0x3FFFD
    )
}

/// Print comprehensive details of a single plugin.
pub fn print_plugin_details(p: &ExternalPluginMetadata) {
    let default_ver = tr!("ui.version.latest_unspecified");
    let version_str = match &p.version {
        Some(v) if !v.trim().is_empty() => v.as_str(),
        _ => default_ver.as_str(),
    };

    let default_cat = tr!("ui.category.other");
    let category_str = match &p.category {
        Some(c) if !c.trim().is_empty() => c.as_str(),
        _ => default_cat.as_str(),
    };

    let type_str = match p.type_.as_str() {
        "free" => tr!("ui.type.free").green().bold(),
        "paid" => tr!("ui.type.paid", price = format!("{:.2}", p.price))
            .yellow()
            .bold(),
        "adwall" => tr!("ui.type.ad_supported").cyan().bold(),
        other => other.white().bold(),
    };

    println!();
    println!("  {}", format!("📦 {}", p.name).bright_yellow().bold());
    println!("  {}", "─".repeat(50).dimmed());
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.version"), 16).bold(),
        version_str.cyan()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.author"), 16).bold(),
        p.dev_name.white()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.category"), 16).bold(),
        category_str.magenta()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.type"), 16).bold(),
        type_str
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.downloads"), 16).bold(),
        p.downloads.to_string().blue()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.public_id"), 16).bold(),
        p.public_id.dimmed()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.database_id"), 16).bold(),
        p.id.to_string().dimmed()
    );
    println!(
        "  {} {}",
        pad_label(&tr!("ui.detail.last_updated"), 16).bold(),
        p.updated_at.dimmed()
    );

    if let Some(preview) = &p.preview_url {
        println!(
            "  {} {}",
            pad_label(&tr!("ui.detail.preview"), 16).bold(),
            preview.underline().blue()
        );
    }

    println!("  {}", "─".repeat(50).dimmed());
    println!(
        "  {}",
        tr!(
            "ui.detail.install_command",
            cmd = format!("ppm install {}", p.name).bright_cyan().bold()
        )
    );
    println!();
}

/// Print local installed plugins.
pub fn print_installed_table(plugins: &[InstalledPlugin]) {
    if plugins.is_empty() {
        println!("{}", tr!("ui.installed.empty").yellow());
        println!(
            "{}",
            tr!(
                "ui.installed.tip",
                search_cmd = "ppm search <QUERY>".cyan(),
                install_cmd = "ppm install <PLUGIN>".cyan()
            )
        );
        return;
    }

    let mut table = Table::new();
    table
        .load_style(presets::UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new(tr!("ui.installed.table.file")).fg(Color::Yellow),
            Cell::new(tr!("ui.installed.table.status")).fg(Color::Green),
            Cell::new(tr!("ui.installed.table.version")).fg(Color::Cyan),
            Cell::new(tr!("ui.installed.table.author")).fg(Color::White),
            Cell::new(tr!("ui.installed.table.market_id")).fg(Color::DarkGrey),
            Cell::new(tr!("ui.installed.table.size")).fg(Color::Blue),
            Cell::new(tr!("ui.installed.table.modified")).fg(Color::Cyan),
        ]);

    for p in plugins {
        let status = if p.is_active {
            if p.is_managed {
                Cell::new(tr!("ui.status.active")).fg(Color::Green)
            } else {
                Cell::new(tr!("ui.status.active_unmanaged")).fg(Color::DarkGreen)
            }
        } else {
            Cell::new(tr!("ui.status.deactivated")).fg(Color::DarkGrey)
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
    println!(
        "{}",
        tr!(
            "ui.installed.total",
            count = plugins.len().to_string().bold()
        )
    );
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
