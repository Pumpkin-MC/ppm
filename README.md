# Pumpkin Package Manager (`ppm`)

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org)
[![Pumpkin](https://img.shields.io/badge/Pumpkin-Server-ff7518.svg)](https://github.com/Pumpkin-MC/Pumpkin)

**`ppm`** is the official package and plugin manager for the [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) Minecraft server ecosystem.

It allows server administrators and developers to effortlessly search and download WebAssembly (`.wasm`) plugins from the [Pumpkin Marketplace](https://market.pumpkinmc.org), manage installed plugins directly inside server roots or `plugins/` folders, and scaffold new plugin projects across all supported languages with an interactive TUI wizard.

---

## Installation

### From Source

Ensure you have Rust and Cargo installed (edition 2024 compatible, Rust 1.85+):

```bash
git clone https://github.com/Pumpkin-MC/ppm.git
cd ppm
cargo install --path .
```

Or build the release binary manually:

```bash
cargo build --release
# Binary available at target/release/ppm
```

---

## Command Reference

### 1. Searching the Marketplace

Search plugins by keyword, name, or author:

```bash
# General search
ppm search AppleSkin

# Filter by category (e.g. Utilities, Fun, Admin Tools, Economy, Chat)
ppm search --category Utilities

# Filter by type (free, paid, adwall)
ppm search --type free

# Output as JSON
ppm search AppleSkin --json
```

### 2. Inspecting Plugin Details

View metadata, versions, author, and description:

```bash
ppm info AppleSkinPumpkin
# You can also query by public ID or database ID:
ppm info z0UPVzl8
```

### 3. Installing Plugins

Download and install a plugin directly:

```bash
# Executed in Pumpkin server root -> installs to ./plugins/
ppm install AppleSkinPumpkin

# Paste entire URL from marketplace
ppm install https://market.pumpkinmc.org/plugin/S1zisQxa

# Install from a direct external download URL (.wasm)
ppm install https://github.com/Pumpkin-MC/example/releases/download/v1.0.0/example.wasm

# Install by author namespace or unique public ID
ppm install alex/economy
ppm install S1zisQxa

# Restore all marketplace plugins recorded in .ppm.lock
ppm install

# Interactive ambiguity resolution:
# If multiple plugins share the same name, ppm interactively prompts
# you to select which one you want (displaying author, version, and downloads).

# Executed directly inside the plugins folder -> installs to ./
ppm install AppleSkinPumpkin

# Install multiple plugins in batch
ppm install AppleSkin Essentials Economy

# Install to custom plugins directory
ppm install AppleSkinPumpkin --plugins-dir /path/to/server/plugins

# Install with custom filename (single plugin)
ppm install AppleSkinPumpkin -o custom_appleskin.wasm

# Force overwrite existing file
ppm install AppleSkinPumpkin --force

# Install paid/private plugin with access token
ppm install PaidPlugin --token <YOUR_TOKEN>
```

#### Deterministic Lockfile (`.ppm.lock`)

When installing plugins from the Pumpkin Marketplace, `ppm` automatically tracks them in a `.ppm.lock` file inside your plugins directory.
- **Unambiguous Tracking:** Records the plugin's exact marketplace `public_id`, author, version, and local file name.
- **Marketplace Only:** Direct URL downloads (`http://` / `https://`) are installed directly and not tracked in the lockfile.
- **One-Command Restore:** Run `ppm install` with no arguments on a new server deployment or CI/CD pipeline to automatically install all plugins from `.ppm.lock`.
- **Automatic Sync:** Running `ppm uninstall <plugin>` cleanly removes the entry from `.ppm.lock`.

### 4. Managing Installed Plugins

List all WebAssembly plugins currently in your plugins directory:

```bash
ppm installed
# or using alias:
ppm ls
```

Remove installed plugin(s):

```bash
ppm uninstall AppleSkinPumpkin
# Remove multiple plugins at once:
ppm uninstall AppleSkin Essentials Economy -y
```

Check for available updates:

```bash
# Check all installed plugins for updates (utilizes .ppm.lock for exact IDs)
ppm update --check

# Upgrade a specific plugin
ppm update AppleSkinPumpkin

# Upgrade all installed plugins
ppm update
```

### 5. Creating New Plugins (`ppm new`)

Launch the interactive TUI wizard to scaffold a new plugin:

```bash
ppm new
```

The wizard prompts for:
1. **Plugin Name**
2. **Author Name** (auto-detected from `git config` when available)
3. **Description**
4. **Target Language** (Rust, Python, C#, C, Go, Kotlin, D, Zig, TypeScript)
5. **Sample Features** (Custom command handler, Player join event listener)

#### Non-Interactive / Scriptable Creation

You can also pass arguments directly to skip interactive prompts:

```bash
# Rust
ppm new my-plugin --lang rust -y

# Python
ppm new my-plugin --lang py -y

# C#
ppm new my-plugin --lang cs -y

# C
ppm new my-plugin --lang c -y

# Go
ppm new my-plugin --lang go -y

# Kotlin
ppm new my-plugin --lang kt -y

# D
ppm new my-plugin --lang d -y

# Zig
ppm new my-plugin --lang zig -y

# TypeScript
ppm new my-plugin --lang ts -y
```

### 6. Automatic Shell Completions

Shell completions for **Bash**, **Zsh**, and **Fish** are **automatically installed and configured by default** whenever `ppm` runs or when you run `ppm self-install`. No manual commands or completion flags are required!

Completions are automatically placed into:
- **Bash:** `~/.local/share/bash-completion/completions/ppm` (and sourced via `~/.bashrc`)
- **Zsh:** `~/.zfunc/_ppm` (and loaded via `~/.zshrc`)
- **Fish:** `~/.config/fish/completions/ppm.fish` (automatically loaded by Fish)

---

## Configuration & Environment Variables

| Variable | CLI Flag | Default | Description |
|---|---|---|---|
| `PUMPKIN_MARKET_URL` | `--market-url <URL>` | `https://market.pumpkinmc.org` | Pumpkin Marketplace API base URL |
| `PUMPKIN_PLUGINS_DIR` | `--plugins-dir <DIR>` | Auto-detected | Directory where plugins are installed (auto-resolves server root vs plugins dir) |
| `PPM_TOKEN` | `--token <TOKEN>` | `None` | Authentication token for private/paid plugins |

---

## License

This project is licensed under the **GNU General Public License v3.0** (GPL-3.0-or-later) - see the [LICENSE](LICENSE) file for details.
