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
    // go.mod
    let go_mod = format!(
        r#"module {name}

go 1.23

require (
	github.com/Pumpkin-MC/pumpkin-api-go v0.1.0
)
"#,
        name = ctx.name
    );
    fs::write(dir.join("go.mod"), go_mod).context("Failed to write go.mod")?;

    // .gitignore
    let gitignore = r#"*.wasm
/bin
/build
"#;
    fs::write(dir.join(".gitignore"), gitignore).context("Failed to write .gitignore")?;

    // main.go
    let struct_name = format!("{}Plugin", ctx.struct_name());
    let main_go = format!(
        r#"package main

import (
	"github.com/Pumpkin-MC/pumpkin-api-go/api"
	"github.com/Pumpkin-MC/pumpkin-api-go/pkg/pumpkin_plugin_context"
	_ "github.com/Pumpkin-MC/pumpkin-api-go/pkg/wit_exports"
)

type {struct_name} struct {{
	api.DefaultPlugin
}}

func (p *{struct_name}) Metadata() api.Metadata {{
	return api.Metadata{{
		Name:        "{name}",
		Version:     "0.1.0",
		Authors:     []string{{"{author}"}},
		Description: "{desc}",
	}}
}}

func (p *{struct_name}) OnLoad(ctx *pumpkin_plugin_context.Context) {{
	// Plugin initialization logic here
}}

func init() {{
	api.RegisterPlugin(&{struct_name}{{}})
}}

func main() {{}}
"#,
        struct_name = struct_name,
        name = ctx.name,
        author = ctx.author,
        desc = ctx.description
    );
    fs::write(dir.join("main.go"), main_go).context("Failed to write main.go")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A Go plugin for the Pumpkin Minecraft Server.

## Building

Ensure you have Go and the WebAssembly tools installed.
Build the plugin to WASM and copy the `.wasm` file to your Pumpkin server's `plugins/` folder.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme).context("Failed to write README.md")?;

    Ok(())
}
