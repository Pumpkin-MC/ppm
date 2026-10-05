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

    // package.json
    let package_json = format!(
        r#"{{
  "name": "{name}",
  "version": "0.1.0",
  "description": "{desc}",
  "main": "src/index.ts",
  "scripts": {{
    "build": "pumpkin-plugin-build src/index.ts dist/{name}.wasm"
  }},
  "author": "{author}",
  "license": "GPL-3.0-or-later",
  "dependencies": {{
    "@pumpkinmc/pumpkin-api-ts": "latest"
  }},
  "devDependencies": {{
    "typescript": "^5.0.0"
  }}
}}
"#,
        name = ctx.name,
        desc = ctx.description,
        author = ctx.author
    );
    fs::write(dir.join("package.json"), package_json).context("Failed to write package.json")?;

    // tsconfig.json
    let tsconfig = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "node",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true
  },
  "include": ["src/**/*"]
}
"#;
    fs::write(dir.join("tsconfig.json"), tsconfig).context("Failed to write tsconfig.json")?;

    // .gitignore
    let gitignore = r#"node_modules/
dist/
*.wasm
"#;
    fs::write(dir.join(".gitignore"), gitignore).context("Failed to write .gitignore")?;

    // src/index.ts
    let struct_name = format!("{}Plugin", ctx.struct_name());
    let mut imports = vec![
        r#"import { Plugin, registerPlugin } from "@pumpkinmc/pumpkin-api-ts";"#.to_string(),
        r#"import { PluginMetadata } from "pumpkin:plugin/metadata@0.1.0";"#.to_string(),
        r#"import { Context } from "pumpkin:plugin/context@0.1.0";"#.to_string(),
        r#"import * as logging from "pumpkin:plugin/logging@0.1.0";"#.to_string(),
    ];

    let mut load_body = format!(
        "    super.onLoad(ctx);\n    logging.log(\"info\", \"{} plugin loaded!\");\n",
        ctx.name
    );

    if ctx.include_event {
        imports.push(
            r#"import { PlayerJoinEventData } from "pumpkin:plugin/event@0.1.0";"#.to_string(),
        );
        imports.push(r#"import { TextComponent } from "pumpkin:plugin/text@0.1.0";"#.to_string());

        load_body.push_str(
            r#"
    // Register player join event listener
    this.registerEvent(
      ctx,
      "player-join-event",
      (_srv, evt: PlayerJoinEventData) => {
        logging.log("info", `Player ${evt.player.getName()} joined the game!`);
        evt.player
          .getWorld()
          .broadcastSystemMessage(
            TextComponent.text(`Welcome ${evt.player.getName()} to the server!`),
            false,
          );
      },
    );
"#,
        );
    }

    let index_ts = format!(
        r#"// {name} - Pumpkin Server Plugin in TypeScript
{imports}

class {struct_name} extends Plugin {{
  metadata(): PluginMetadata {{
    return {{
      name: "{name}",
      version: "0.1.0",
      authors: ["{author}"],
      description: "{desc}",
      dependencies: [],
      permissions: []
    }};
  }}

  onLoad(ctx: Context): void {{
{load_body}  }}
}}

registerPlugin(new {struct_name}());

export * from "@pumpkinmc/pumpkin-api-ts";
"#,
        name = ctx.name,
        imports = imports.join("\n"),
        struct_name = struct_name,
        author = ctx.author,
        desc = ctx.description,
        load_body = load_body
    );

    fs::write(dir.join("src/index.ts"), index_ts).context("Failed to write src/index.ts")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A TypeScript plugin for Pumpkin Minecraft Server compiled to WebAssembly.

## Getting Started

1. Install dependencies:
   ```bash
   npm install
   ```

2. Build the `.wasm` plugin:
   ```bash
   npm run build
   ```

3. Deploy to Pumpkin:
   Copy `dist/{name}.wasm` to your server's `plugins/` folder.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme).context("Failed to write README.md")?;

    Ok(())
}
