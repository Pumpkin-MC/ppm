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
    // pyproject.toml
    let pyproject = format!(
        r#"[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "{name}"
version = "0.1.0"
description = "{desc}"
authors = [{{ name = "{author}" }}]
readme = "README.md"
license = "GPL-3.0-or-later"
dependencies = [
    "pumpkin-api",
]
"#,
        name = ctx.name,
        desc = ctx.description,
        author = ctx.author
    );
    fs::write(dir.join("pyproject.toml"), pyproject)
        .context("Failed to write pyproject.toml")?;

    // .gitignore
    let gitignore = r#"__pycache__/
*.py[cod]
*$py.class
dist/
build/
*.wasm
.venv/
"#;
    fs::write(dir.join(".gitignore"), gitignore)
        .context("Failed to write .gitignore")?;

    // plugin.py
    let struct_name = format!("{}Plugin", ctx.struct_name());
    let mut methods = String::new();
    let mut on_load_lines = vec![
        format!("        logging.log(logging.Level.INFO, \"{} plugin loaded!\")", ctx.name),
    ];

    if ctx.include_command {
        on_load_lines.push(format!(
            r#"        # Register command '/{name}'
        cmd = command.Command(["{name}"], "A greeting command")
        self.register_command(ctx, cmd, self.command_handler, "{name}")"#,
            name = ctx.name
        ));

        methods.push_str(&format!(
            r#"    def command_handler(
        self,
        sender: command.CommandSender,
        srv: server.Server,
        args: command.ConsumedArgs,
    ) -> int:
        sender.send_message(text.TextComponent.text("Hello from {name} plugin!"))
        return 1

"#,
            name = ctx.name
        ));
    }

    if ctx.include_event {
        on_load_lines.push(
            r#"        # Register player join event handler
        self.register_event(ctx, event.EventType.PLAYER_JOIN_EVENT, self.on_player_join)"#
                .to_string(),
        );

        methods.push_str(
            r#"    def on_player_join(
        self, srv: server.Server, evt: event.PlayerJoinEventData
    ) -> event.PlayerJoinEventData:
        logging.log(logging.Level.INFO, f"Player {evt.player.get_name()} joined!")
        evt.player.send_message(text.TextComponent.text("Welcome to the server!"))
        return evt

"#,
        );
    }

    let plugin_py = format!(
        r#"# {name} - Pumpkin Server Plugin in Python
from pumpkin_api import (
    Plugin,
    command,
    context,
    event,
    logging,
    metadata,
    register_plugin,
    server,
    text,
)

PluginMetadata = metadata.PluginMetadata


class {struct_name}(Plugin):
    def metadata(self) -> PluginMetadata:
        return PluginMetadata(
            name="{name}",
            version="0.1.0",
            authors=["{author}"],
            description="{desc}",
            dependencies=[],
            permissions=[],
        )

    def on_load(self, ctx: context.Context) -> None:
{on_load}

{methods}
register_plugin({struct_name})
"#,
        name = ctx.name,
        struct_name = struct_name,
        author = ctx.author,
        desc = ctx.description,
        on_load = on_load_lines.join("\n"),
        methods = methods
    );

    fs::write(dir.join("plugin.py"), plugin_py)
        .context("Failed to write plugin.py")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A Python plugin for the Pumpkin Minecraft Server.

## Getting Started

1. Create a virtual environment and install dependencies:
   ```bash
   python -m venv .venv
   source .venv/bin/activate
   pip install -e .
   ```

2. Compile or package your plugin to WebAssembly for deployment to Pumpkin.
3. Place the resulting `.wasm` plugin into your Pumpkin server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme)
        .context("Failed to write README.md")?;

    Ok(())
}
