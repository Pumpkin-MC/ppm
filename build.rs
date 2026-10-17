// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

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

    // A fresh clone keeps branch refs in packed-refs, so the heads directory is watched too.
    for path in [".git/HEAD", ".git/packed-refs", ".git/refs/heads"] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
