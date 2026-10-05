// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::fmt;
use std::str::FromStr;

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginLanguage {
    Rust,
    Python,
    CSharp,
    C,
    Go,
    Kotlin,
    D,
    Zig,
    TypeScript,
}

impl PluginLanguage {
    pub const ALL: &'static [PluginLanguage] = &[
        PluginLanguage::Rust,
        PluginLanguage::Python,
        PluginLanguage::CSharp,
        PluginLanguage::C,
        PluginLanguage::Go,
        PluginLanguage::Kotlin,
        PluginLanguage::D,
        PluginLanguage::Zig,
        PluginLanguage::TypeScript,
    ];

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Rust => "Rust (Recommended - pumpkin-plugin-api)",
            Self::Python => "Python (pumpkin-api)",
            Self::CSharp => "C# (.NET 10 with PumpkinMC.PumpkinApi)",
            Self::C => "C (pumpkin-api-c)",
            Self::Go => "Go (github.com/Pumpkin-MC/pumpkin-api-go)",
            Self::Kotlin => "Kotlin (wasmWasi Multiplatform)",
            Self::D => "D (dlang with LDC wasm32-wasi)",
            Self::Zig => "Zig (native wasm32-wasi target)",
            Self::TypeScript => "TypeScript / JavaScript (@pumpkinmc/pumpkin-api-ts)",
        }
    }

    #[allow(dead_code)]
    pub fn identifier(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::CSharp => "csharp",
            Self::C => "c",
            Self::Go => "go",
            Self::Kotlin => "kotlin",
            Self::D => "d",
            Self::Zig => "zig",
            Self::TypeScript => "typescript",
        }
    }
}

impl fmt::Display for PluginLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

impl FromStr for PluginLanguage {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().trim() {
            "rust" | "rs" => Ok(Self::Rust),
            "python" | "py" => Ok(Self::Python),
            "csharp" | "cs" | "c#" | "dotnet" => Ok(Self::CSharp),
            "c" => Ok(Self::C),
            "go" | "golang" => Ok(Self::Go),
            "kotlin" | "kt" => Ok(Self::Kotlin),
            "d" | "dlang" => Ok(Self::D),
            "zig" => Ok(Self::Zig),
            "typescript" | "ts" | "javascript" | "js" => Ok(Self::TypeScript),
            other => bail!(
                "Unknown plugin language: '{other}'. Supported: rust, python (py), csharp (cs), c, go, kotlin (kt), d, zig, typescript (ts)"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_parsing() {
        assert_eq!(PluginLanguage::from_str("rust").unwrap(), PluginLanguage::Rust);
        assert_eq!(PluginLanguage::from_str("rs").unwrap(), PluginLanguage::Rust);
        assert_eq!(PluginLanguage::from_str("python").unwrap(), PluginLanguage::Python);
        assert_eq!(PluginLanguage::from_str("py").unwrap(), PluginLanguage::Python);
        assert_eq!(PluginLanguage::from_str("csharp").unwrap(), PluginLanguage::CSharp);
        assert_eq!(PluginLanguage::from_str("cs").unwrap(), PluginLanguage::CSharp);
        assert_eq!(PluginLanguage::from_str("c").unwrap(), PluginLanguage::C);
        assert_eq!(PluginLanguage::from_str("go").unwrap(), PluginLanguage::Go);
        assert_eq!(PluginLanguage::from_str("golang").unwrap(), PluginLanguage::Go);
        assert_eq!(PluginLanguage::from_str("kotlin").unwrap(), PluginLanguage::Kotlin);
        assert_eq!(PluginLanguage::from_str("kt").unwrap(), PluginLanguage::Kotlin);
        assert_eq!(PluginLanguage::from_str("d").unwrap(), PluginLanguage::D);
        assert_eq!(PluginLanguage::from_str("dlang").unwrap(), PluginLanguage::D);
        assert_eq!(PluginLanguage::from_str("zig").unwrap(), PluginLanguage::Zig);
        assert_eq!(PluginLanguage::from_str("typescript").unwrap(), PluginLanguage::TypeScript);
        assert_eq!(PluginLanguage::from_str("ts").unwrap(), PluginLanguage::TypeScript);
        assert!(PluginLanguage::from_str("unknown_lang").is_err());
    }
}
