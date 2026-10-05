// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::{Client, StatusCode, header};

use super::models::{CheckUpdateResponse, ExternalPluginMetadata, ListPluginsParams};

/// Client for communicating with the Pumpkin Marketplace REST API.
#[derive(Clone, Debug)]
pub struct MarketClient {
    base_url: String,
    http: Client,
}

impl MarketClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        let mut base = base_url.into();
        if base.ends_with('/') {
            base.pop();
        }

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("ppm/0.1.0 (Pumpkin Package Manager)"),
        );

        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to initialize HTTP client");

        Self {
            base_url: base,
            http,
        }
    }

    /// List or search plugins with query parameters.
    pub async fn list_plugins(
        &self,
        params: &ListPluginsParams,
    ) -> Result<Vec<ExternalPluginMetadata>> {
        let url = format!("{}/api/v1/rest/plugins", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(params)
            .send()
            .await
            .context("Failed to connect to Pumpkin Marketplace")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Marketplace API error ({status}): {text}");
        }

        let plugins: Vec<ExternalPluginMetadata> = resp
            .json()
            .await
            .context("Failed to parse plugin list response")?;
        Ok(plugins)
    }

    /// Retrieve detailed information about a single plugin.
    ///
    /// Accepts a plugin database ID, public ID (e.g. `S1zisQxa`), or name (case-insensitive).
    pub async fn get_plugin(&self, id_or_name: &str) -> Result<ExternalPluginMetadata> {
        let trimmed = id_or_name.trim();

        // 1. Try direct fetch via ID / public_id
        let direct_url = format!("{}/api/v1/rest/plugins/{}", self.base_url, trimmed);
        let resp = self.http.get(&direct_url).send().await;

        if let Ok(res) = resp
            && res.status() == StatusCode::OK
            && let Ok(plugin) = res.json::<ExternalPluginMetadata>().await
        {
            return Ok(plugin);
        }

        // 2. Search by query to find matching name
        let params = ListPluginsParams {
            q: Some(trimmed.to_string()),
            limit: Some(20),
            ..Default::default()
        };

        let search_results = self.list_plugins(&params).await?;

        // Try exact match on name or public_id (case-insensitive)
        if let Some(exact) = search_results.iter().find(|p| {
            p.name.eq_ignore_ascii_case(trimmed)
                || p.public_id.eq_ignore_ascii_case(trimmed)
                || p.id.to_string() == trimmed
        }) {
            return Ok(exact.clone());
        }

        // Try prefix or substring match if only one candidate found
        if search_results.len() == 1 {
            return Ok(search_results.into_iter().next().unwrap());
        }

        if search_results.is_empty() {
            bail!("Plugin '{id_or_name}' not found on the Pumpkin Marketplace.");
        }

        // Multiple results found but no exact match
        let suggestions: Vec<String> = search_results.iter().map(|p| p.name.clone()).collect();
        bail!(
            "Multiple plugins matched '{id_or_name}': {}. Please specify the full name or ID.",
            suggestions.join(", ")
        );
    }

    /// Check if an update is available for an installed plugin.
    pub async fn check_update(
        &self,
        plugin_name: &str,
        current_version: &str,
    ) -> Result<CheckUpdateResponse> {
        let url = format!("{}/api/v1/rest/check-update", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("plugin_name", plugin_name),
                ("current_version", current_version),
            ])
            .send()
            .await
            .context("Failed to check for updates")?;

        if !resp.status().is_success() {
            bail!("Check update failed with status: {}", resp.status());
        }

        let check: CheckUpdateResponse =
            resp.json().await.context("Failed to parse update info")?;
        Ok(check)
    }

    /// Start downloading a plugin WASM binary.
    ///
    /// Returns the streaming response along with the filename deduced from
    /// headers or the plugin name.
    pub async fn download_plugin(
        &self,
        id_or_name: &str,
        token: Option<&str>,
    ) -> Result<(reqwest::Response, String)> {
        // Resolve metadata first to have clean plugin name & verify existence
        let metadata = self.get_plugin(id_or_name).await;

        let download_target = match &metadata {
            Ok(meta) => meta.public_id.as_str(),
            Err(_) => id_or_name,
        };

        let url = format!(
            "{}/api/v1/rest/plugins/{}/download",
            self.base_url, download_target
        );

        let mut req = self.http.get(&url);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }

        let resp = req
            .send()
            .await
            .context("Failed to initiate plugin download")?;

        match resp.status() {
            StatusCode::OK => {
                let filename = extract_filename(&resp).unwrap_or_else(|| {
                    if let Ok(meta) = &metadata {
                        format!("{}.wasm", meta.name.replace(' ', "_"))
                    } else {
                        format!("{}.wasm", id_or_name.replace(' ', "_"))
                    }
                });

                Ok((resp, filename))
            }
            StatusCode::UNAUTHORIZED => {
                bail!(
                    "Unauthorized (401): This is a paid or private plugin.\n\
                     Please provide an access token with '--token <TOKEN>' or set PPM_TOKEN."
                );
            }
            StatusCode::FORBIDDEN => {
                bail!(
                    "Forbidden (403): You do not have permission or an active license\n\
                     to download this plugin from the Pumpkin Marketplace."
                );
            }
            StatusCode::NOT_FOUND => {
                bail!("Plugin '{id_or_name}' was not found on the Pumpkin Marketplace (404).");
            }
            status => {
                let text = resp.text().await.unwrap_or_default();
                bail!("Failed to download plugin ({status}): {text}");
            }
        }
    }
}

/// Helper to parse filename from Content-Disposition header.
fn extract_filename(resp: &reqwest::Response) -> Option<String> {
    let header_val = resp
        .headers()
        .get(header::CONTENT_DISPOSITION)?
        .to_str()
        .ok()?;

    // Example header: attachment; filename="EpicRTP.wasm"; filename*=UTF-8''EpicRTP.wasm
    for part in header_val.split(';') {
        let part = part.trim();
        if let Some(stripped) = part.strip_prefix("filename=") {
            let clean = stripped.trim_matches('"').trim();
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }

    None
}
