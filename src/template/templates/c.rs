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
    // pumpkin_api.h
    let pumpkin_api_h = r#"#ifndef PUMPKIN_API_H
#define PUMPKIN_API_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* plugin_own_context_t;

typedef struct {
    const char* name;
    const char* version;
    const char** authors;
    size_t authors_count;
    const char* description;
    const char** dependencies;
    size_t dependencies_count;
    const char** permissions;
    size_t permissions_count;
} pumpkin_metadata_t;

typedef void (*pumpkin_on_load_t)(plugin_own_context_t ctx);
typedef void (*pumpkin_on_unload_t)(plugin_own_context_t ctx);
typedef pumpkin_metadata_t (*pumpkin_get_metadata_t)(void);

typedef struct {
    pumpkin_get_metadata_t get_metadata;
    pumpkin_on_load_t on_load;
    pumpkin_on_unload_t on_unload;
} pumpkin_plugin_t;

void pumpkin_register_plugin(pumpkin_plugin_t plugin);

#define REGISTER_PUMPKIN_PLUGIN(plugin) \
    void exports_plugin_init_plugin(void) { \
        pumpkin_register_plugin(plugin); \
    }

#ifdef __cplusplus
}
#endif

#endif
"#;
    fs::write(dir.join("pumpkin_api.h"), pumpkin_api_h).context("Failed to write pumpkin_api.h")?;

    // plugin.c
    let plugin_c = format!(
        r#"#include "pumpkin_api.h"
#include <stdio.h>

pumpkin_metadata_t get_meta(void) {{
    static const char* authors[] = {{"{author}"}};
    return (pumpkin_metadata_t) {{
        .name = "{name}",
        .version = "0.1.0",
        .authors = authors,
        .authors_count = 1,
        .description = "{desc}",
        .dependencies_count = 0,
        .permissions_count = 0
    }};
}}

void on_load(plugin_own_context_t ctx) {{
    (void)ctx;
    printf("{name} C plugin loaded!\n");
}}

void on_unload(plugin_own_context_t ctx) {{
    (void)ctx;
    printf("{name} C plugin unloaded.\n");
}}

REGISTER_PUMPKIN_PLUGIN(((pumpkin_plugin_t){{
    .get_metadata = get_meta,
    .on_load = on_load,
    .on_unload = on_unload
}}))
"#,
        name = ctx.name,
        author = ctx.author,
        desc = ctx.description
    );
    fs::write(dir.join("plugin.c"), plugin_c).context("Failed to write plugin.c")?;

    // Makefile
    let makefile = format!(
        r#"CC ?= wasm32-wasip2-clang
CFLAGS ?= -Oz -Wall -Wextra

all: {name}.wasm

{name}.wasm: plugin.c pumpkin_api.h
	$(CC) $(CFLAGS) -o $@ plugin.c

clean:
	rm -f *.wasm *.o

.PHONY: all clean
"#,
        name = ctx.name
    );
    fs::write(dir.join("Makefile"), makefile).context("Failed to write Makefile")?;

    // .gitignore
    let gitignore = r#"*.wasm
*.o
build/
"#;
    fs::write(dir.join(".gitignore"), gitignore).context("Failed to write .gitignore")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A C plugin for Pumpkin Minecraft Server compiled to WebAssembly.

## Building

```bash
make
```

Copy `{name}.wasm` to your server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme).context("Failed to write README.md")?;

    Ok(())
}
