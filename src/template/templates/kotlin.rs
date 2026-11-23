// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fs;
use std::path::Path;

use anyhow::Result;

use super::{TemplateContext, write_file};

pub fn generate(dir: &Path, ctx: &TemplateContext) -> Result<()> {
    let pkg_name = ctx.name.replace(['-', '_'], "").to_lowercase();
    let src_dir = dir.join("src/wasmWasiMain/kotlin").join(&pkg_name);
    fs::create_dir_all(&src_dir)?;

    // build.gradle.kts
    let build_gradle = r#"plugins {
    kotlin("multiplatform") version "2.4.0"
}

repositories {
    mavenCentral()
}

kotlin {
    @OptIn(org.jetbrains.kotlin.gradle.ExperimentalWasmDsl::class)
    wasmWasi {
        binaries.executable()
    }
}
"#;
    write_file(
        &dir.join("build.gradle.kts"),
        "build.gradle.kts",
        build_gradle,
    )?;

    // settings.gradle.kts
    let settings_gradle = format!(
        r#"rootProject.name = "{name}"
"#,
        name = ctx.name
    );
    write_file(
        &dir.join("settings.gradle.kts"),
        "settings.gradle.kts",
        settings_gradle,
    )?;

    // gradle.properties
    let gradle_props = r#"kotlin.wasm.componentModel.enabled=true
"#;
    write_file(
        &dir.join("gradle.properties"),
        "gradle.properties",
        gradle_props,
    )?;

    // .gitignore
    let gitignore = r#".gradle/
build/
*.wasm
"#;
    write_file(&dir.join(".gitignore"), ".gitignore", gitignore)?;

    // Main.kt
    let struct_name = ctx.struct_name();
    let main_kt = format!(
        r#"package {pkg_name}

/**
 * {name} - Pumpkin Server Plugin in Kotlin
 * Author: {author}
 * Description: {desc}
 */

class {struct_name}Plugin {{
    fun onLoad() {{
        println("{name} Kotlin plugin loaded!")
    }}

    fun onUnload() {{
        println("{name} Kotlin plugin unloaded.")
    }}
}}

fun main() {{
    val plugin = {struct_name}Plugin()
    plugin.onLoad()
}}
"#,
        pkg_name = pkg_name,
        name = ctx.name,
        author = ctx.author,
        desc = ctx.description,
        struct_name = struct_name
    );
    write_file(&src_dir.join("Main.kt"), "Main.kt", main_kt)?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A Kotlin plugin for the Pumpkin Minecraft Server compiled to WebAssembly (WASI).

## Prerequisites

- JDK 17+ or JDK 21+
- Gradle (or Gradle Wrapper)

## Building

Build the WebAssembly executable using Gradle:

```bash
gradle wasmWasiReleaseExecutable
```

The resulting `.wasm` plugin will be located under:
```
build/bin/wasmWasi/releaseExecutable/{name}.wasm
```

Copy the `.wasm` file into your Pumpkin server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    write_file(&dir.join("README.md"), "README.md", readme)?;

    Ok(())
}
