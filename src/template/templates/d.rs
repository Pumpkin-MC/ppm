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
    fs::create_dir_all(dir.join("source"))?;

    // dub.json
    let dub_json = format!(
        r#"{{
  "name": "{name}",
  "description": "{desc}",
  "authors": ["{author}"],
  "license": "GPL-3.0-or-later",
  "targetType": "dynamicLibrary",
  "targetPath": "dist",
  "targetName": "{name}",
  "dflags-ldc": ["-mtriple=wasm32-wasi"]
}}
"#,
        name = ctx.name,
        desc = ctx.description,
        author = ctx.author
    );
    fs::write(dir.join("dub.json"), dub_json).context("Failed to write dub.json")?;

    // Makefile
    let makefile = format!(
        r#"LDC ?= ldc2
LDCFLAGS ?= -mtriple=wasm32-wasi -Oz

all: {name}.wasm

{name}.wasm: source/app.d
	$(LDC) $(LDCFLAGS) -shared -of=$@ source/app.d

clean:
	rm -f *.wasm *.o

.PHONY: all clean
"#,
        name = ctx.name
    );
    fs::write(dir.join("Makefile"), makefile).context("Failed to write Makefile")?;

    // .gitignore
    let gitignore = r#".dub/
dist/
*.wasm
*.o
"#;
    fs::write(dir.join(".gitignore"), gitignore).context("Failed to write .gitignore")?;

    // source/app.d
    let app_d = format!(
        r#"// {name} - Pumpkin Server Plugin in D (dlang)
// Author: {author}
// Description: {desc}

module app;

import core.stdc.stdio : printf;

extern(C) struct PluginMetadata {{
    const(char)* name;
    const(char)* version_;
    const(char)** authors;
    size_t authors_count;
    const(char)* description;
    size_t dependencies_count;
    size_t permissions_count;
}}

extern(C) struct Plugin {{
    PluginMetadata function() get_metadata;
    void function(void* ctx) on_load;
    void function(void* ctx) on_unload;
}}

extern(C) void pumpkin_register_plugin(Plugin plugin);

__gshared immutable const(char)*[1] authors = ["{author}\0".ptr];

extern(C) PluginMetadata get_meta() {{
    PluginMetadata meta;
    meta.name = "{name}\0".ptr;
    meta.version_ = "0.1.0\0".ptr;
    meta.authors = authors.ptr;
    meta.authors_count = authors.length;
    meta.description = "{desc}\0".ptr;
    meta.dependencies_count = 0;
    meta.permissions_count = 0;
    return meta;
}}

extern(C) void on_load(void* ctx) {{
    printf("{name} D plugin loaded!\n");
}}

extern(C) void on_unload(void* ctx) {{
    printf("{name} D plugin unloaded.\n");
}}

extern(C) void exports_plugin_init_plugin() {{
    Plugin p;
    p.get_metadata = &get_meta;
    p.on_load = &on_load;
    p.on_unload = &on_unload;
    pumpkin_register_plugin(p);
}}
"#,
        name = ctx.name,
        author = ctx.author,
        desc = ctx.description
    );
    fs::write(dir.join("source/app.d"), app_d).context("Failed to write source/app.d")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A WebAssembly plugin for the Pumpkin Minecraft Server written in D.

## Prerequisites

- [LDC](https://github.com/ldc-developers/ldc) (LLVM D Compiler) with WASI target support:
  ```bash
  # Check if ldc2 supports wasm32-wasi
  ldc2 -mtriple=wasm32-wasi --version
  ```

## Building

Using Makefile:
```bash
make
```

Or using DUB:
```bash
dub build --compiler=ldc2
```

Copy `{name}.wasm` to your Pumpkin server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme).context("Failed to write README.md")?;

    Ok(())
}
