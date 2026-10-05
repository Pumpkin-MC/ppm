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
    TypeScript,
    Python,
    Go,
    CSharp,
    C,
}

impl PluginLanguage {
    pub const ALL: &'static [PluginLanguage] = &[
        PluginLanguage::Rust,
        PluginLanguage::TypeScript,
        PluginLanguage::Python,
        PluginLanguage::Go,
        PluginLanguage::CSharp,
        PluginLanguage::C,
    ];

    pub fn display_label(&self) -> &'static str {
        match self {
            Self::Rust => "Rust (Recommended - official pumpkin-plugin-api)",
            Self::TypeScript => "TypeScript / JavaScript (@pumpkinmc/pumpkin-api-ts)",
            Self::Python => "Python (pumpkin-api)",
            Self::Go => "Go (github.com/Pumpkin-MC/pumpkin-api-go)",
            Self::CSharp => "C# (.NET 10 with PumpkinMC.PumpkinApi)",
            Self::C => "C (pumpkin-api-c)",
        }
    }

    #[allow(dead_code)]
    pub fn identifier(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Python => "python",
            Self::Go => "go",
            Self::CSharp => "csharp",
            Self::C => "c",
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
            "typescript" | "ts" | "javascript" | "js" => Ok(Self::TypeScript),
            "python" | "py" => Ok(Self::Python),
            "go" | "golang" => Ok(Self::Go),
            "csharp" | "cs" | "c#" | "dotnet" => Ok(Self::CSharp),
            "c" => Ok(Self::C),
            other => bail!(
                "Unknown plugin language: '{other}'. Supported: rust, typescript (ts), python (py), go, csharp (cs), c"
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
        assert_eq!(PluginLanguage::from_str("ts").unwrap(), PluginLanguage::TypeScript);
        assert_eq!(PluginLanguage::from_str("typescript").unwrap(), PluginLanguage::TypeScript);
        assert_eq!(PluginLanguage::from_str("py").unwrap(), PluginLanguage::Python);
        assert_eq!(PluginLanguage::from_str("python").unwrap(), PluginLanguage::Python);
        assert_eq!(PluginLanguage::from_str("go").unwrap(), PluginLanguage::Go);
        assert_eq!(PluginLanguage::from_str("cs").unwrap(), PluginLanguage::CSharp);
        assert_eq!(PluginLanguage::from_str("c").unwrap(), PluginLanguage::C);
        assert!(PluginLanguage::from_str("ruby").is_err());
    }
}
