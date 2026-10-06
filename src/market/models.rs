// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use serde::{Deserialize, Serialize};

/// Metadata returned by the Pumpkin Marketplace REST API for a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalPluginMetadata {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub price: f64,
    pub downloads: i64,
    pub dev_name: String,
    #[serde(default)]
    pub preview_url: Option<String>,
    pub public_id: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Query parameters for listing plugins.
#[derive(Debug, Default, Clone, Serialize)]
pub struct ListPluginsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// Response returned from check-update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckUpdateResponse {
    pub update_available: bool,
    #[serde(default)]
    pub latest_version: Option<String>,
}

/// Target representation of user input when specifying a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginTarget {
    /// A marketplace plugin identifier (name, author/plugin, public ID, or database ID)
    Marketplace(String),
    /// A direct HTTP(S) URL to download a .wasm plugin from an external host
    DirectUrl(String),
}

/// Parses user input for a plugin, handling raw names, IDs, full marketplace URLs,
/// or direct HTTP(S) download links.
pub fn parse_plugin_input(raw_input: &str) -> PluginTarget {
    let mut trimmed = raw_input.trim();
    // Strip surrounding quotes
    trimmed = trimmed.trim_matches('\'').trim_matches('"').trim();

    // If copied from browser without scheme (e.g. market.pumpkinmc.org/plugin/...)
    let url_candidate = if trimmed.starts_with("market.pumpkinmc.org/") {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    };

    if let Ok(url) = reqwest::Url::parse(&url_candidate)
        && (url.scheme() == "http" || url.scheme() == "https")
    {
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|seg| !seg.is_empty()).collect())
            .unwrap_or_default();

        // 1. REST API endpoint: /api/v1/rest/plugins/<id>[/download]
        if segments.len() >= 4
            && segments[0].eq_ignore_ascii_case("api")
            && segments[1].eq_ignore_ascii_case("v1")
            && segments[2].eq_ignore_ascii_case("rest")
            && segments[3].eq_ignore_ascii_case("plugins")
            && let Some(id) = segments.get(4)
        {
            return PluginTarget::Marketplace(id.to_string());
        }

        // 2. Marketplace web UI routes: /plugin/<id>, /plugins/<id>, /p/<id>
        if segments.len() >= 2
            && (segments[0].eq_ignore_ascii_case("plugin")
                || segments[0].eq_ignore_ascii_case("plugins")
                || segments[0].eq_ignore_ascii_case("p"))
        {
            return PluginTarget::Marketplace(segments[1].to_string());
        }

        // 3. Short URL on marketplace domain: market.pumpkinmc.org/<id>
        if let Some(host) = url.host_str()
            && (host.eq_ignore_ascii_case("market.pumpkinmc.org") || host.contains("pumpkin"))
            && segments.len() == 1
            && !segments[0].ends_with(".wasm")
        {
            return PluginTarget::Marketplace(segments[0].to_string());
        }

        // 4. Otherwise, it is an external direct download URL
        return PluginTarget::DirectUrl(url_candidate);
    }

    PluginTarget::Marketplace(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_name() {
        assert_eq!(
            parse_plugin_input("AppleSkin"),
            PluginTarget::Marketplace("AppleSkin".into())
        );
        assert_eq!(
            parse_plugin_input("42"),
            PluginTarget::Marketplace("42".into())
        );
        assert_eq!(
            parse_plugin_input("S1zisQxa"),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input("alex/economy"),
            PluginTarget::Marketplace("alex/economy".into())
        );
    }

    #[test]
    fn test_parse_market_urls() {
        assert_eq!(
            parse_plugin_input("https://market.pumpkinmc.org/plugin/S1zisQxa"),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input("https://market.pumpkinmc.org/plugins/S1zisQxa/"),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input("https://market.pumpkinmc.org/p/S1zisQxa?ref=search"),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input("https://market.pumpkinmc.org/api/v1/rest/plugins/S1zisQxa"),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input(
                "https://market.pumpkinmc.org/api/v1/rest/plugins/S1zisQxa/download"
            ),
            PluginTarget::Marketplace("S1zisQxa".into())
        );
        assert_eq!(
            parse_plugin_input("market.pumpkinmc.org/plugin/AppleSkin"),
            PluginTarget::Marketplace("AppleSkin".into())
        );
    }

    #[test]
    fn test_parse_direct_urls() {
        assert_eq!(
            parse_plugin_input(
                "https://github.com/Pumpkin-MC/example/releases/download/v1.0.0/example.wasm"
            ),
            PluginTarget::DirectUrl(
                "https://github.com/Pumpkin-MC/example/releases/download/v1.0.0/example.wasm"
                    .into()
            )
        );
        assert_eq!(
            parse_plugin_input("http://custom-host.org/files/plugin.wasm"),
            PluginTarget::DirectUrl("http://custom-host.org/files/plugin.wasm".into())
        );
    }
}
