// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::TemplateContext;

pub fn generate(dir: &Path, ctx: &TemplateContext) -> Result<()> {
    fs::create_dir_all(dir.join("src"))?;

    // build.zig
    let build_zig = format!(
        r#"const std = @import("std");

pub fn build(b: *std.Build) void {{
    const target = b.resolveTargetQuery(.{{
        .cpu_arch = .wasm32,
        .os_tag = .wasi,
    }});
    const optimize = b.standardOptimizeOption(.{{}});

    const lib = b.addSharedLibrary(.{{
        .name = "{name}",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    }});

    lib.rdynamic = true;
    b.installArtifact(lib);
}}
"#,
        name = ctx.name
    );
    fs::write(dir.join("build.zig"), build_zig).context("Failed to write build.zig")?;

    // build.zig.zon
    let build_zon = format!(
        r#".{{
    .name = "{name}",
    .version = "0.1.0",
    .dependencies = .{{}},
    .paths = .{{
        "build.zig",
        "build.zig.zon",
        "src",
        "README.md",
    }},
}}
"#,
        name = ctx.name
    );
    fs::write(dir.join("build.zig.zon"), build_zon).context("Failed to write build.zig.zon")?;

    // .gitignore
    let gitignore = r#".zig-cache/
zig-out/
*.wasm
*.o
"#;
    fs::write(dir.join(".gitignore"), gitignore).context("Failed to write .gitignore")?;

    // src/main.zig
    let main_zig = format!(
        r#"//! {name} - Pumpkin Server Plugin in Zig
//!
//! Author: {author}
//! Description: {desc}

const std = @import("std");

pub const PluginMetadata = extern struct {{
    name: [*:0]const u8,
    version: [*:0]const u8,
    authors: [*]const [*:0]const u8,
    authors_count: usize,
    description: [*:0]const u8,
    dependencies_count: usize,
    permissions_count: usize,
}};

pub const Plugin = extern struct {{
    get_metadata: *const fn () callconv(.C) PluginMetadata,
    on_load: *const fn (?*anyopaque) callconv(.C) void,
    on_unload: *const fn (?*anyopaque) callconv(.C) void,
}};

extern fn pumpkin_register_plugin(plugin: Plugin) void;

const authors = [_][*:0]const u8{{"{author}"}};

export fn get_metadata() callconv(.C) PluginMetadata {{
    return .{{
        .name = "{name}",
        .version = "0.1.0",
        .authors = &authors,
        .authors_count = authors.len,
        .description = "{desc}",
        .dependencies_count = 0,
        .permissions_count = 0,
    }};
}}

export fn on_load(ctx: ?*anyopaque) callconv(.C) void {{
    _ = ctx;
    std.debug.print("{name} Zig plugin loaded!\n", .{{}});
}}

export fn on_unload(ctx: ?*anyopaque) callconv(.C) void {{
    _ = ctx;
    std.debug.print("{name} Zig plugin unloaded.\n", .{{}});
}}

export fn exports_plugin_init_plugin() callconv(.C) void {{
    pumpkin_register_plugin(.{{
        .get_metadata = get_metadata,
        .on_load = on_load,
        .on_unload = on_unload,
    }});
}}
"#,
        name = ctx.name,
        author = ctx.author,
        desc = ctx.description
    );
    fs::write(dir.join("src/main.zig"), main_zig).context("Failed to write src/main.zig")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A WebAssembly plugin for the Pumpkin Minecraft Server written in Zig.

## Prerequisites

- [Zig](https://ziglang.org/) compiler (0.13+)

## Building

Build the WebAssembly binary:

```bash
zig build -Doptimize=ReleaseSmall
```

The compiled WebAssembly binary will be generated at:
```
zig-out/lib/{name}.wasm
```

Copy `{name}.wasm` to your Pumpkin server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme).context("Failed to write README.md")?;

    Ok(())
}
