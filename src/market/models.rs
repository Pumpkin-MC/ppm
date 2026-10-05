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
