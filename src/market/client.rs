// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use std::io::IsTerminal;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::{Client, StatusCode, header};

use super::models::{
    CheckUpdateResponse, ExternalPluginMetadata, ListPluginsParams, PluginTarget,
    parse_plugin_input,
};

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
    /// Accepts a plugin database ID, public ID (e.g. `S1zisQxa`), name (case-insensitive),
    /// author-scoped identifier (`author/name`), or full marketplace URL.
    pub async fn get_plugin(&self, id_or_name: &str) -> Result<ExternalPluginMetadata> {
        let parsed = parse_plugin_input(id_or_name);
        let target = match parsed {
            PluginTarget::Marketplace(id) => id,
            PluginTarget::DirectUrl(url) => {
                bail!(
                    "'{url}' is an external direct download URL, not a marketplace plugin page.\n\
                     To install it directly, run: ppm install {url}"
                );
            }
        };

        let trimmed = target.trim();

        // 1. Try direct fetch via ID / public_id
        let direct_url = format!("{}/api/v1/rest/plugins/{}", self.base_url, trimmed);
        let resp = self.http.get(&direct_url).send().await;

        if let Ok(res) = resp
            && res.status() == StatusCode::OK
            && let Ok(plugin) = res.json::<ExternalPluginMetadata>().await
        {
            return Ok(plugin);
        }

        // 2. Check author-scoped format: "author/plugin"
        if let Some((author, name)) = trimmed.split_once('/') {
            let author = author.trim();
            let name = name.trim();
            let params = ListPluginsParams {
                q: Some(name.to_string()),
                limit: Some(20),
                ..Default::default()
            };
            let search_results = self.list_plugins(&params).await?;
            let author_matches: Vec<ExternalPluginMetadata> = search_results
                .into_iter()
                .filter(|p| {
                    p.name.eq_ignore_ascii_case(name) && p.dev_name.eq_ignore_ascii_case(author)
                })
                .collect();

            if author_matches.len() == 1 {
                return Ok(author_matches.into_iter().next().unwrap());
            } else if author_matches.len() > 1 {
                return resolve_candidates(trimmed, &author_matches);
            } else {
                bail!("Plugin '{name}' by author '{author}' not found on the Pumpkin Marketplace.");
            }
        }

        // 3. Search by query to find matching name
        let params = ListPluginsParams {
            q: Some(trimmed.to_string()),
            limit: Some(20),
            ..Default::default()
        };

        let search_results = self.list_plugins(&params).await?;

        // Look for exact matches on name, public_id, or database ID
        let exact_matches: Vec<ExternalPluginMetadata> = search_results
            .iter()
            .filter(|p| {
                p.name.eq_ignore_ascii_case(trimmed)
                    || p.public_id.eq_ignore_ascii_case(trimmed)
                    || p.id.to_string() == trimmed
            })
            .cloned()
            .collect();

        if exact_matches.len() == 1 {
            return Ok(exact_matches.into_iter().next().unwrap());
        }

        // Multiple exact matches (same name published by different authors)
        if exact_matches.len() > 1 {
            return resolve_candidates(trimmed, &exact_matches);
        }

        // No exact match found
        if search_results.is_empty() {
            bail!("Plugin '{id_or_name}' not found on the Pumpkin Marketplace.");
        }

        if search_results.len() == 1 {
            return Ok(search_results.into_iter().next().unwrap());
        }

        // Multiple partial matches found matching the query
        resolve_candidates(trimmed, &search_results)
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

    /// Start downloading a plugin WASM binary from the marketplace.
    ///
    /// Returns the streaming response along with the filename, display name, and optional metadata.
    pub async fn download_plugin(
        &self,
        id_or_name: &str,
        token: Option<&str>,
    ) -> Result<(
        reqwest::Response,
        String,
        String,
        Option<ExternalPluginMetadata>,
    )> {
        // Resolve metadata first to have clean plugin name & verify existence
        let metadata = self.get_plugin(id_or_name).await;

        let download_target = match &metadata {
            Ok(meta) => meta.public_id.clone(),
            Err(_) => id_or_name.to_string(),
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

                let display_name = match &metadata {
                    Ok(meta) => meta.name.clone(),
                    Err(_) => filename.trim_end_matches(".wasm").to_string(),
                };

                let meta_opt = metadata.ok();
                Ok((resp, filename, display_name, meta_opt))
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

    /// Download a plugin directly from an external HTTP/HTTPS URL.
    pub async fn download_direct_url(
        &self,
        url: &str,
        token: Option<&str>,
    ) -> Result<(
        reqwest::Response,
        String,
        String,
        Option<ExternalPluginMetadata>,
    )> {
        let mut req = self.http.get(url);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }

        let resp = req
            .send()
            .await
            .with_context(|| format!("Failed to initiate download from '{url}'"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("Failed to download plugin from '{url}' ({status}): {text}");
        }

        let filename = extract_filename(&resp).unwrap_or_else(|| {
            if let Ok(parsed) = reqwest::Url::parse(url)
                && let Some(mut segs) = parsed.path_segments()
                && let Some(last) = segs.next_back().filter(|s| !s.is_empty())
            {
                let name = last.split('?').next().unwrap_or(last);
                if name.ends_with(".wasm") {
                    return name.to_string();
                } else {
                    return format!("{name}.wasm");
                }
            }
            "plugin.wasm".to_string()
        });

        let display_name = filename.trim_end_matches(".wasm").to_string();
        Ok((resp, filename, display_name, None))
    }
}

#[derive(Clone)]
struct CandidateChoice(ExternalPluginMetadata);

impl std::fmt::Display for CandidateChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = &self.0;
        let ver = p.version.as_deref().unwrap_or("latest");
        let price_tag = match p.type_.as_str() {
            "paid" => format!(" (${:.2})", p.price),
            "adwall" => " (ad-supported)".to_string(),
            _ => String::new(),
        };
        write!(
            f,
            "{} by {} [v{}, {} dl, ID: {}{}]",
            p.name, p.dev_name, ver, p.downloads, p.public_id, price_tag
        )
    }
}

fn resolve_candidates(
    query: &str,
    candidates: &[ExternalPluginMetadata],
) -> Result<ExternalPluginMetadata> {
    if std::io::stdin().is_terminal() {
        prompt_candidate_selection(query, candidates)
    } else {
        let mut msg = format!("Multiple plugins matched '{query}':\n");
        for p in candidates {
            let ver = p.version.as_deref().unwrap_or("latest");
            let price = if p.type_ == "paid" {
                format!(" (${:.2})", p.price)
            } else {
                String::new()
            };
            msg.push_str(&format!(
                "  • {} by {} [v{}, {} dl, ID: {}{}]\n",
                p.name, p.dev_name, ver, p.downloads, p.public_id, price
            ));
        }
        msg.push_str("Please specify the exact Public ID, Database ID, or full URL to proceed.");
        bail!(msg);
    }
}

fn prompt_candidate_selection(
    query: &str,
    candidates: &[ExternalPluginMetadata],
) -> Result<ExternalPluginMetadata> {
    let options: Vec<CandidateChoice> = candidates.iter().cloned().map(CandidateChoice).collect();

    let prompt_msg = format!(
        "Multiple plugins found matching '{}'. Please select one:",
        query
    );

    let selected = inquire::Select::new(&prompt_msg, options)
        .with_page_size(10)
        .prompt();

    match selected {
        Ok(choice) => Ok(choice.0),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            bail!("Plugin selection cancelled by user.");
        }
        Err(e) => bail!("Failed to select plugin: {e}"),
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
