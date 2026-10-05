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
    let struct_name = ctx.struct_name();

    // NuGet.Config
    let nuget_config = r#"<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <add key="dotnet-experimental" value="https://pkgs.dev.azure.com/dnceng/public/_packaging/dotnet-experimental/nuget/v3/index.json" />
  </packageSources>
</configuration>
"#;
    fs::write(dir.join("NuGet.Config"), nuget_config)
        .context("Failed to write NuGet.Config")?;

    // .csproj
    let csproj = format!(
        r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <RuntimeIdentifier>wasi-wasm</RuntimeIdentifier>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="PumpkinMC.PumpkinApi" Version="*" />
    <PackageReference Include="ByteCodeAlliance.Componentize.DotNet.Wasm.SDK" Version="*" />
  </ItemGroup>
</Project>
"#
    );
    let csproj_name = format!("{struct_name}.csproj");
    fs::write(dir.join(&csproj_name), csproj)
        .with_context(|| format!("Failed to write {csproj_name}"))?;

    // .gitignore
    let gitignore = r#"bin/
obj/
*.wasm
"#;
    fs::write(dir.join(".gitignore"), gitignore)
        .context("Failed to write .gitignore")?;

    // Plugin.cs
    let plugin_cs = format!(
        r#"using PluginWorld;
using PluginWorld.wit.Exports.pumpkin.plugin.v0_1_0;
using PluginWorld.wit.Imports.pumpkin.plugin.v0_1_0;

namespace {struct_name};

public class {struct_name}Plugin : IPluginWorldExports, IMetadataExports
{{
    public static void InitPlugin() {{ }}

    public static void OnLoad(IContextImports.Context context)
    {{
        ILoggingImports.Log(ILoggingImports.Level.Info, "{name} C# plugin loaded!");
    }}

    public static void OnUnload(IContextImports.Context context) {{ }}

    public static IEventImports.Event HandleEvent(uint eventId, IServerImports.Server server, IEventImports.Event @event)
    {{
        return @event;
    }}

    public static int HandleCommand(uint commandId, ICommandImports.CommandSender sender, IServerImports.Server server, ICommandImports.ConsumedArgs args)
    {{
        return 0;
    }}

    public static void HandleTask(uint handlerId, IServerImports.Server server) {{ }}

    public static IMetadataExports.PluginMetadata Metadata()
    {{
        return new IMetadataExports.PluginMetadata(
            "{name}",
            "0.1.0",
            "{desc}",
            ["{author}"],
            []
        );
    }}
}}
"#,
        struct_name = struct_name,
        name = ctx.name,
        desc = ctx.description,
        author = ctx.author
    );
    fs::write(dir.join("Plugin.cs"), plugin_cs)
        .context("Failed to write Plugin.cs")?;

    // README.md
    let readme = format!(
        r#"# {name}

{desc}

A C# (.NET 10+) plugin for the Pumpkin Minecraft Server compiled to WebAssembly.

## Building

```bash
dotnet build -c Release
```

Copy the compiled `.wasm` file from `bin/Release/net10.0/wasi-wasm/` into your Pumpkin server's `plugins/` directory.
"#,
        name = ctx.name,
        desc = ctx.description
    );
    fs::write(dir.join("README.md"), readme)
        .context("Failed to write README.md")?;

    Ok(())
}
