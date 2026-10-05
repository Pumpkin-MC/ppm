# Pumpkin Package Manager (`ppm`)

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org)
[![Pumpkin](https://img.shields.io/badge/Pumpkin-Server-ff7518.svg)](https://github.com/Pumpkin-MC/Pumpkin)

**`ppm`** is the official package and plugin manager for the [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) Minecraft server ecosystem.

It allows server administrators and developers to effortlessly browse, search, and download WebAssembly (`.wasm`) plugins from the [Pumpkin Marketplace](https://market.pumpkinmc.org), manage installed plugins on their servers, and scaffold new plugin projects in multiple programming languages with an interactive TUI wizard.

---

## 🌟 Key Features

- 🔍 **Marketplace Search & Discovery**: Search, browse, and inspect plugin details directly from `market.pumpkinmc.org`.
- 📥 **One-Command Installation**: Download signed `.wasm` plugin binaries directly into your server's `plugins/` folder with live progress indicators.
- 📦 **Local Plugin Management**: List active/deactivated plugins, check file sizes, and safely remove plugins.
- 🔄 **Plugin Updates**: Check for updates and automatically upgrade installed plugins to the latest release.
- 🚀 **Interactive Plugin Template Generator**: Rapidly bootstrap new Pumpkin plugins with a friendly TUI wizard supporting:
  - 🦀 **Rust** (official `pumpkin-plugin-api`)
  - 🟨 **TypeScript / JavaScript** (`@pumpkinmc/pumpkin-api-ts`)
  - 🐍 **Python** (`pumpkin-api`)
  - 🐹 **Go** (`github.com/Pumpkin-MC/pumpkin-api-go`)
  - 🟣 **C# (.NET 10+)** (`PumpkinMC.PumpkinApi`)
  - 🇨 **C** (`pumpkin-api-c`)

---

## 🚀 Installation

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

## 📖 Command Reference

### 1. Searching the Marketplace

Search plugins by keyword, name, or developer:

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

### 2. Browsing Recent & Popular Plugins

List plugins from the marketplace with pagination:

```bash
ppm list --limit 10
ppm list --category "Admin Tools"
```

### 3. Inspecting Plugin Details

View metadata, versions, author, and description:

```bash
ppm info AppleSkinPumpkin
# You can also query by public ID or database ID:
ppm info z0UPVzl8
```

### 4. Installing Plugins

Download and install a plugin directly into your Pumpkin server:

```bash
# Install to default directory (./plugins)
ppm install AppleSkinPumpkin

# Install to custom plugins directory
ppm install AppleSkinPumpkin --plugins-dir /path/to/server/plugins

# Install with custom filename
ppm install AppleSkinPumpkin -o custom_appleskin.wasm

# Force overwrite existing file
ppm install AppleSkinPumpkin --force

# Install paid/private plugin with access token
ppm install PaidPlugin --token <YOUR_TOKEN>
```

### 5. Managing Installed Plugins

List all WebAssembly plugins currently in your plugins directory:

```bash
ppm installed
# or using aliases:
ppm ls
```

Remove an installed plugin:

```bash
ppm uninstall AppleSkinPumpkin
# Skip confirmation prompt:
ppm uninstall AppleSkinPumpkin -y
```

Check for available updates:

```bash
# Check all installed plugins for updates
ppm update --check

# Upgrade a specific plugin
ppm update AppleSkinPumpkin

# Upgrade all installed plugins
ppm update
```

### 6. Creating New Plugins (`ppm new`)

Launch the interactive TUI wizard to scaffold a new plugin:

```bash
ppm new
```

The wizard prompts for:
1. **Plugin Name**
2. **Author Name** (auto-detected from `git config` when available)
3. **Description**
4. **Target Language** (Rust, TypeScript, Python, Go, C#, C)
5. **Sample Features** (Custom command handler, Player join event listener)

#### Non-Interactive / Scriptable Creation

You can also pass arguments directly to skip interactive prompts:

```bash
# Scaffold a Rust plugin
ppm new my-awesome-plugin --lang rust -y

# Scaffold a TypeScript plugin
ppm new ts-plugin --lang ts -y

# Scaffold a Python plugin
ppm new py-plugin --lang py -y
```

---

## ⚙️ Configuration & Environment Variables

| Variable | CLI Flag | Default | Description |
|---|---|---|---|
| `PUMPKIN_MARKET_URL` | `--market-url <URL>` | `https://market.pumpkinmc.org` | Pumpkin Marketplace API base URL |
| `PUMPKIN_PLUGINS_DIR` | `--plugins-dir <DIR>` | `./plugins` | Default directory where plugins are installed |
| `PPM_TOKEN` | `--token <TOKEN>` | `None` | Authentication token for private/paid plugins |

---

## 📄 License

This project is licensed under the **GNU General Public License v3.0** (GPL-3.0-or-later) - see the [LICENSE](LICENSE) file for details.
