// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Embeds the git commit the binary is built from as `PPM_GIT_COMMIT` (empty when unavailable).
fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .unwrap_or_default();

    println!("cargo:rustc-env=PPM_GIT_COMMIT={commit}");
    println!("cargo:rerun-if-changed=build.rs");

    let head = Path::new(".git/HEAD");
    if !head.is_file() {
        return;
    }

    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(content) = fs::read_to_string(head)
        && let Some(reference) = content.strip_prefix("ref: ")
    {
        let ref_path = Path::new(".git").join(reference.trim());
        if ref_path.is_file() {
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
    }
    if Path::new(".git/packed-refs").is_file() {
        println!("cargo:rerun-if-changed=.git/packed-refs");
    }
}
