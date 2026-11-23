// Copyright (C) 2026 Pumpkin-MC Contributors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Lightweight internationalization for `ppm`.
//!
//! Catalogs are discovered from `locales/*.json` at build time (see `build.rs`),
//! which validates them and embeds them as static tables. The active locale is
//! resolved from, in order: the `--locale` flag, `PPM_LOCALE` / `PPM_LANG`,
//! the standard POSIX locale variables, and finally the native OS locale.
//! Missing translations fall back to the default catalog (`en_US`).

use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};

mod generated {
    include!(concat!(env!("OUT_DIR"), "/locales_generated.rs"));
}

/// `(key, message)` pairs of a single catalog, sorted by key.
type Table = &'static [(&'static str, &'static str)];

/// Sentinel stored in [`CURRENT_LOCALE_INDEX`] before the locale is initialized.
const UNINITIALIZED: usize = usize::MAX;

/// Index into `generated::LOCALES` for the active locale.
///
/// Storing the table index in an `AtomicUsize` instead of a `LazyLock<RwLock<_>>`
/// avoids a duplicate `sys_locale::get_locales()` probe when `init()` runs at
/// startup, and turns every `tr!()` lookup into a single atomic load plus a
/// binary search with no lock or table scan.
static CURRENT_LOCALE_INDEX: AtomicUsize = AtomicUsize::new(UNINITIALIZED);

/// `build.rs` records the fallback catalog's index, preferring `en_US.json`.
fn default_index() -> usize {
    generated::DEFAULT_LOCALE_INDEX
}

/// Returns the default locale code from the single authoritative locale table.
fn default_code() -> &'static str {
    tables()[default_index()].0
}

fn tables() -> &'static [(&'static str, Table)] {
    generated::LOCALES
}

/// Finds a catalog's index by code, tolerating POSIX spellings and
/// separator/case differences (`zh-CN`, `zh_CN.UTF-8`, `pt_BR` vs `pt-BR`).
/// Exact matches are checked first so the hot path performs no normalization.
fn index_for(locale: &str) -> Option<usize> {
    let tables = tables();

    if let Some(index) = tables.iter().position(|(code, _)| *code == locale) {
        return Some(index);
    }

    let (requested, _) = strip_encoding_and_modifier(locale);

    tables.iter().position(|(code, _)| {
        let (known, _) = strip_encoding_and_modifier(code);
        known.eq_ignore_ascii_case(requested)
            || (known.len() == requested.len()
                && known.bytes().zip(requested.bytes()).all(|(a, b)| {
                    a.eq_ignore_ascii_case(&b) || matches!((a, b), (b'_', b'-') | (b'-', b'_'))
                }))
    })
}

fn lookup(table: Table, key: &str) -> Option<&'static str> {
    table
        .binary_search_by(|(existing, _)| (*existing).cmp(key))
        .ok()
        .map(|index| table[index].1)
}

/// Looks a message up by catalog index, falling back to the default catalog.
fn get_template_by_index(index: usize, key: &str) -> Option<&'static str> {
    let tables = tables();
    if let Some((_, table)) = tables.get(index)
        && let Some(message) = lookup(table, key)
    {
        return Some(message);
    }

    let fallback = default_index();
    if index != fallback
        && let Some((_, table)) = tables.get(fallback)
        && let Some(message) = lookup(table, key)
    {
        return Some(message);
    }

    None
}

fn current_index() -> usize {
    let index = CURRENT_LOCALE_INDEX.load(Ordering::Acquire);
    if index != UNINITIALIZED {
        return index;
    }

    let detected = index_for(detect_system_locale()).unwrap_or_else(default_index);
    match CURRENT_LOCALE_INDEX.compare_exchange(
        UNINITIALIZED,
        detected,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => detected,
        Err(existing) => existing,
    }
}

/// Switches the active locale, resolving it against the available catalogs.
///
/// Resolution order: `preferred`, `PPM_LOCALE` / `PPM_LANG`, POSIX locale
/// variables, native OS locale, then the default catalog. Returns the resolved
/// locale code.
pub fn init(preferred: Option<&str>) -> &'static str {
    let resolved = resolve_locale(&collect_system_candidates(preferred));
    let index = index_for(resolved).unwrap_or_else(default_index);
    CURRENT_LOCALE_INDEX.store(index, Ordering::Release);
    tables()[index].0
}

/// Returns every locale code discovered from `locales/*.json`, sorted.
pub fn available_locales() -> &'static [&'static str] {
    generated::LOCALE_CODES
}

/// Detects the system locale and resolves it to the best available catalog.
pub fn detect_system_locale() -> &'static str {
    resolve_locale(&collect_system_candidates(None))
}

/// Resolves an ordered list of preferred locale tags to an available catalog.
pub fn resolve_locale(candidates: &[String]) -> &'static str {
    resolve_locale_from_available(candidates, available_locales(), default_code())
}

/// Renders a template with no arguments, unescaping `{{`/`}}` when present so
/// plain lookups match parameterized lookups.
fn render_plain(template: &str) -> String {
    if template.contains(['{', '}']) {
        format_template(template, |_| None)
    } else {
        template.to_string()
    }
}

/// Translates `key` with the active locale, falling back to the default catalog.
pub fn t(key: &str) -> String {
    let template = get_template_by_index(current_index(), key).unwrap_or(key);
    render_plain(template)
}

/// Translates `key` with an explicit locale, falling back to the default catalog.
#[cfg(test)]
pub fn t_for(locale: &str, key: &str) -> String {
    let index = index_for(locale).unwrap_or_else(default_index);
    let template = get_template_by_index(index, key).unwrap_or(key);
    render_plain(template)
}

/// Looks `key` up for `index` and expands `{name}` placeholders in one pass.
///
/// Generic over the argument value type so that both `&str` slices and the
/// owned `String` slices produced by [`crate::tr!`] are served without an
/// intermediate allocation.
fn fill_template_by_index<'args, Value: AsRef<str>>(
    index: usize,
    key: &str,
    args: &'args [(&str, Value)],
) -> String {
    let template = get_template_by_index(index, key).unwrap_or(key);
    format_template(template, |name| {
        args.iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, value)| -> &'args str { value.as_ref() })
    })
}

/// Translates `key` for the active locale and substitutes `{name}` placeholders.
pub fn tf_owned(key: &str, args: &[(&str, String)]) -> String {
    fill_template_by_index(current_index(), key, args)
}

/// Translates a message key, optionally with named `{placeholder}` arguments.
///
/// # Examples
/// ```ignore
/// tr!("ui.no_plugins_found")
/// tr!("install.failed", plugin = name, error = err)
/// ```
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
    ($key:expr, $($name:ident = $val:expr),+ $(,)?) => {{
        let _args: &[(&str, String)] = &[
            $((stringify!($name), ($val).to_string()),)+
        ];
        $crate::i18n::tf_owned($key, _args)
    }};
}

fn collect_system_candidates(explicit: Option<&str>) -> Vec<String> {
    let os_locales: Vec<String> = sys_locale::get_locales().collect();
    collect_candidates_from_sources(explicit, |name| env::var(name).ok(), os_locales)
}

/// Builds the ordered locale candidates from explicit overrides, the
/// environment, and the native OS locale.
///
/// Follows GNU gettext: `LANGUAGE` outranks the `LC_*` variables, but only when
/// the effective message locale is not `C`/`POSIX`; a `C`/`POSIX` locale
/// disables translation entirely, including the OS fallback.
fn collect_candidates_from_sources(
    explicit: Option<&str>,
    env_get: impl Fn(&str) -> Option<String>,
    os_locales: Vec<String>,
) -> Vec<String> {
    let env_value = |name: &str| {
        env_get(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };

    let lc_all = env_value("LC_ALL");
    let lc_messages = env_value("LC_MESSAGES");
    let lang = env_value("LANG");
    let language_list = env_value("LANGUAGE");

    // POSIX precedence for the effective message locale.
    let effective = lc_all
        .as_deref()
        .or(lc_messages.as_deref())
        .or(lang.as_deref());
    let translate = !effective.is_some_and(is_posix_locale);

    let mut candidates: Vec<String> = Vec::new();
    let mut push = |candidate: &str| {
        let candidate = candidate.trim();
        if candidate.is_empty()
            || candidates
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(candidate))
        {
            return;
        }
        candidates.push(candidate.to_string());
    };

    // Tool-level overrides always win.
    if let Some(preferred) = explicit {
        push(preferred);
    }
    if let Some(value) = env_value("PPM_LOCALE") {
        push(&value);
    }
    if let Some(value) = env_value("PPM_LANG") {
        push(&value);
    }

    if translate {
        if let Some(list) = language_list.as_deref() {
            for part in list.split(':') {
                push(part);
            }
        }
        for value in [lc_all.as_deref(), lc_messages.as_deref(), lang.as_deref()]
            .into_iter()
            .flatten()
        {
            push(value);
        }
        for locale in &os_locales {
            push(locale);
        }
    } else if let Some(value) = effective {
        push(value);
    }

    candidates
}

/// A locale tag split into its `language[-script][-region]` components.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedLocale {
    language: String,
    script: Option<String>,
    region: Option<String>,
}

impl ParsedLocale {
    /// The script, either explicit or implied by a well-known language/region pair.
    fn inferred_script(&self) -> Option<&str> {
        if let Some(ref script) = self.script {
            return Some(script.as_str());
        }

        if self.language == "zh" {
            return match self.region.as_deref() {
                Some("tw" | "hk" | "mo") => Some("hant"),
                Some("cn" | "sg") => Some("hans"),
                _ => None,
            };
        }

        None
    }
}

/// The regional variant preferred when only the language is known.
fn canonical_region(language: &str) -> Option<&'static str> {
    match language {
        "zh" => Some("cn"),
        "en" => Some("us"),
        _ => None,
    }
}

/// The regional variant preferred when a script is known and multiple regional
/// catalogs share that script (e.g. `zh_HK` and `zh_TW` both use `hant`).
fn canonical_script_region(language: &str, script: &str) -> Option<&'static str> {
    match (language, script) {
        ("zh", "hans") => Some("cn"),
        ("zh", "hant") => Some("tw"),
        _ => None,
    }
}

/// Strips the POSIX `.encoding` and `@modifier` parts of a locale name.
fn strip_encoding_and_modifier(raw: &str) -> (&str, Option<&str>) {
    let trimmed = raw.trim();
    let (without_modifier, modifier) = match trimmed.split_once('@') {
        Some((before, modifier)) => (before, Some(modifier.trim())),
        None => (trimmed, None),
    };
    let base = match without_modifier.split_once('.') {
        Some((before, _encoding)) => before,
        None => without_modifier,
    };
    (base.trim(), modifier)
}

fn is_posix_locale(raw: &str) -> bool {
    let (base, _) = strip_encoding_and_modifier(raw);
    base.eq_ignore_ascii_case("c") || base.eq_ignore_ascii_case("posix")
}

fn parse_locale_tag(raw: &str) -> Option<ParsedLocale> {
    let (base, modifier) = strip_encoding_and_modifier(raw);

    if base.is_empty() || is_posix_locale(base) {
        return None;
    }

    let parts: Vec<String> = base
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect();

    let language = parts.first()?;

    // BCP 47 primary language subtags are 2-8 ASCII letters; anything else is noise.
    if !(2..=8).contains(&language.len()) || !language.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    let mut script: Option<String> = None;
    let mut region: Option<String> = None;

    for part in &parts[1..] {
        if script.is_none() && part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            script = Some(part.clone());
        } else if region.is_none()
            && ((part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
                || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit())))
        {
            region = Some(part.clone());
        }
    }

    if script.is_none()
        && let Some(modifier) = modifier
    {
        script = match modifier.to_ascii_lowercase().as_str() {
            "latin" | "latn" => Some("latn".to_string()),
            "cyrillic" | "cyrl" => Some("cyrl".to_string()),
            "hans" | "simplified" => Some("hans".to_string()),
            "hant" | "traditional" => Some("hant".to_string()),
            _ => None,
        };
    }

    Some(ParsedLocale {
        language: language.clone(),
        script,
        region,
    })
}

/// Matches candidates against available catalogs, most specific first:
/// exact tag, then language+script, then language+region, then bare language,
/// then any variant of that language.
fn resolve_locale_from_available<'a>(
    candidates: &[String],
    available: &[&'a str],
    default: &'a str,
) -> &'a str {
    let parsed_available: Vec<(&'a str, ParsedLocale)> = available
        .iter()
        .filter_map(|&code| parse_locale_tag(code).map(|parsed| (code, parsed)))
        .collect();

    for candidate in candidates {
        // A `C`/`POSIX` candidate means "no translations".
        if is_posix_locale(candidate) {
            return default;
        }

        let Some(parsed) = parse_locale_tag(candidate) else {
            continue;
        };

        // 1. Exact tag (language, script and region all equal).
        if let Some((code, _)) = parsed_available.iter().find(|(_, known)| *known == parsed) {
            return code;
        }

        // 2. Same language and script:
        //    First prefer an exact region match within the script (`zh-Hant-HK` -> `zh_HK`,
        //    `zh-Hant-TW` -> `zh_TW`), then the canonical region for that script (`zh-Hant`
        //    and `zh-Hant-CN` -> `zh_TW`), then the first matching catalog in sorted order.
        //    Script always outranks region, so `zh-Hant-CN` still yields traditional text.
        if let Some(script) = parsed.inferred_script() {
            if let Some(region) = parsed.region.as_deref()
                && let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
                    known.language == parsed.language
                        && known.inferred_script() == Some(script)
                        && known.region.as_deref() == Some(region)
                })
            {
                return code;
            }

            if let Some(canonical) = canonical_script_region(&parsed.language, script)
                && let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
                    known.language == parsed.language
                        && known.inferred_script() == Some(script)
                        && known.region.as_deref() == Some(canonical)
                })
            {
                return code;
            }

            if let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
                known.language == parsed.language && known.inferred_script() == Some(script)
            }) {
                return code;
            }
        }

        // 3. Same language and region (for languages without script variants).
        if let Some(region) = parsed.region.as_deref()
            && let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
                known.language == parsed.language && known.region.as_deref() == Some(region)
            })
        {
            return code;
        }

        // 4. A catalog that only specifies the language (`ja.json` for `ja_JP`).
        if let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
            known.language == parsed.language && known.region.is_none() && known.script.is_none()
        }) {
            return code;
        }

        // 5. Any variant of that language: the default, then the canonical region,
        //    then the first one in catalog order.
        if let Some(code) = parsed_available
            .iter()
            .find(|(code, known)| known.language == parsed.language && *code == default)
            .map(|(code, _)| *code)
        {
            return code;
        }

        if let Some(canonical) = canonical_region(&parsed.language)
            && let Some((code, _)) = parsed_available.iter().find(|(_, known)| {
                known.language == parsed.language && known.region.as_deref() == Some(canonical)
            })
        {
            return code;
        }

        if let Some((code, _)) = parsed_available
            .iter()
            .find(|(_, known)| known.language == parsed.language)
        {
            return code;
        }
    }

    default
}

/// Expands `{name}` placeholders in one pass; substituted values are never
/// rescanned, so an argument containing braces stays literal.
///
/// `{{` and `}}` escape literal braces, and unknown or unterminated
/// placeholders are preserved as written.
fn format_template<'a>(template: &str, lookup: impl Fn(&str) -> Option<&'a str>) -> String {
    if !template.contains(['{', '}']) {
        return template.to_string();
    }

    let mut out = String::with_capacity(template.len());
    let mut chars = template.char_indices().peekable();

    while let Some((start, character)) = chars.next() {
        match character {
            '{' => {
                if matches!(chars.peek(), Some(&(_, '{'))) {
                    chars.next();
                    out.push('{');
                    continue;
                }

                let mut end = None;
                for (index, inner) in chars.by_ref() {
                    if inner == '}' {
                        end = Some(index);
                        break;
                    }
                }

                match end {
                    Some(end) => match lookup(&template[start + 1..end]) {
                        Some(value) => out.push_str(value),
                        None => out.push_str(&template[start..=end]),
                    },
                    // Unterminated placeholder: keep the remainder verbatim.
                    None => {
                        out.push_str(&template[start..]);
                        break;
                    }
                }
            }
            '}' => {
                if matches!(chars.peek(), Some(&(_, '}'))) {
                    chars.next();
                }
                out.push('}');
            }
            _ => out.push(character),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashMap};
    use std::path::Path;

    #[test]
    fn test_parse_locale_tag() {
        assert_eq!(
            parse_locale_tag("zh_CN.UTF-8"),
            Some(ParsedLocale {
                language: "zh".into(),
                script: None,
                region: Some("cn".into()),
            })
        );
        assert_eq!(
            parse_locale_tag("zh-Hans-CN"),
            Some(ParsedLocale {
                language: "zh".into(),
                script: Some("hans".into()),
                region: Some("cn".into()),
            })
        );
        assert_eq!(
            parse_locale_tag("zh-Hant-TW"),
            Some(ParsedLocale {
                language: "zh".into(),
                script: Some("hant".into()),
                region: Some("tw".into()),
            })
        );
        assert_eq!(
            parse_locale_tag("ES_419"),
            Some(ParsedLocale {
                language: "es".into(),
                script: None,
                region: Some("419".into()),
            })
        );
        assert_eq!(
            parse_locale_tag("fr_FR.UTF-8@latin"),
            Some(ParsedLocale {
                language: "fr".into(),
                script: Some("latn".into()),
                region: Some("fr".into()),
            })
        );
        assert_eq!(parse_locale_tag("C.UTF-8"), None);
        assert_eq!(parse_locale_tag("POSIX"), None);
        assert_eq!(parse_locale_tag(""), None);
        // Numeric "language" tags are not locale names.
        assert_eq!(parse_locale_tag("419"), None);
    }

    #[test]
    fn test_resolve_locale_matching_and_fallback() {
        let available = ["en_US", "zh_CN", "zh_TW"];

        assert_eq!(
            resolve_locale_from_available(&["zh_CN.UTF-8".into()], &available, "en_US"),
            "zh_CN"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hans-CN".into()], &available, "en_US"),
            "zh_CN"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hans".into()], &available, "en_US"),
            "zh_CN"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh_SG".into()], &available, "en_US"),
            "zh_CN"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant-TW".into()], &available, "en_US"),
            "zh_TW"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh_HK.UTF-8".into()], &available, "en_US"),
            "zh_TW"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant".into()], &available, "en_US"),
            "zh_TW"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh".into()], &available, "en_US"),
            "zh_CN"
        );
        assert_eq!(
            resolve_locale_from_available(&["en_GB.UTF-8".into()], &available, "en_US"),
            "en_US"
        );
        assert_eq!(
            resolve_locale_from_available(&["fr_FR.UTF-8".into()], &available, "en_US"),
            "en_US"
        );
        assert_eq!(
            resolve_locale_from_available(&["C.UTF-8".into(), "zh_CN".into()], &available, "en_US"),
            "en_US"
        );
        // The first matching candidate wins.
        assert_eq!(
            resolve_locale_from_available(&["fr_FR".into(), "zh_TW".into()], &available, "en_US"),
            "zh_TW"
        );
    }

    #[test]
    fn test_script_outranks_region() {
        let available = ["en_US", "zh_CN", "zh_HK", "zh_TW"];

        // A traditional-script request must not be answered with simplified text
        // just because the region component matches.
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant-CN".into()], &available, "en_US"),
            "zh_TW"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hans-TW".into()], &available, "en_US"),
            "zh_CN"
        );
        // When both `zh_HK` and `zh_TW` are present, explicit script+region tags
        // must keep their exact region instead of falling to alphabetical order.
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant-TW".into()], &available, "en_US"),
            "zh_TW"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant-HK".into()], &available, "en_US"),
            "zh_HK"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh_HK.UTF-8".into()], &available, "en_US"),
            "zh_HK"
        );
        assert_eq!(
            resolve_locale_from_available(&["zh-Hant".into()], &available, "en_US"),
            "zh_TW"
        );
    }

    #[test]
    fn test_dynamic_locale_addition_without_hardcoding() {
        // Any newly added translation file becomes selectable without code changes.
        let available = ["en_US", "ja_JP", "zh_CN", "zh_TW"];
        assert_eq!(
            resolve_locale_from_available(&["ja-JP".into()], &available, "en_US"),
            "ja_JP"
        );
        assert_eq!(
            resolve_locale_from_available(&["ja".into()], &available, "en_US"),
            "ja_JP"
        );
    }

    #[test]
    fn test_candidate_priority_order() {
        let env_map = HashMap::from([
            ("PPM_LOCALE", "zh_TW"),
            ("LC_ALL", "en_US.UTF-8"),
            ("LANG", "zh_CN.UTF-8"),
        ]);
        let candidates = collect_candidates_from_sources(
            Some("zh_CN"),
            |key| env_map.get(key).map(|value| (*value).to_string()),
            vec!["en-US".into()],
        );
        assert_eq!(candidates[0], "zh_CN");
        assert_eq!(candidates[1], "zh_TW");
        // The POSIX precedence `LC_ALL` -> `LC_MESSAGES` -> `LANG` is kept, and a
        // lower-priority variable only acts as a fallback (`en-US` is the OS locale).
        assert_eq!(
            candidates,
            vec!["zh_CN", "zh_TW", "en_US.UTF-8", "zh_CN.UTF-8", "en-US"]
        );

        // A C/POSIX locale disables translation entirely.
        let lc_all_env = HashMap::from([
            ("LC_ALL", "C"),
            ("LANGUAGE", "zh_CN:en_US"),
            ("LANG", "zh_CN.UTF-8"),
        ]);
        let candidates = collect_candidates_from_sources(
            None,
            |key| lc_all_env.get(key).map(|value| (*value).to_string()),
            vec!["zh-CN".into()],
        );
        assert_eq!(candidates, vec!["C"]);

        // `LANGUAGE` outranks `LC_ALL` for a real (non-C) locale, per gettext.
        let language_env = HashMap::from([("LANGUAGE", "zh_TW:en"), ("LC_ALL", "de_DE.UTF-8")]);
        let candidates = collect_candidates_from_sources(
            None,
            |key| language_env.get(key).map(|value| (*value).to_string()),
            Vec::new(),
        );
        assert_eq!(candidates, vec!["zh_TW", "en", "de_DE.UTF-8"]);

        // LANGUAGE colon list, then LANG, then the OS locale.
        let lang_list_env =
            HashMap::from([("LANGUAGE", "fr_FR:zh_TW:en_US"), ("LANG", "zh_CN.UTF-8")]);
        let candidates = collect_candidates_from_sources(
            None,
            |key| lang_list_env.get(key).map(|value| (*value).to_string()),
            vec!["en-US".into()],
        );
        assert_eq!(
            candidates,
            vec!["fr_FR", "zh_TW", "en_US", "zh_CN.UTF-8", "en-US"]
        );

        // Duplicate spellings are collapsed case-insensitively.
        let duplicate_env = HashMap::from([("LANGUAGE", "zh_TW:zh_tw:ZH_TW:en_US")]);
        let candidates = collect_candidates_from_sources(
            None,
            |key| duplicate_env.get(key).map(|value| (*value).to_string()),
            Vec::new(),
        );
        assert_eq!(candidates, vec!["zh_TW", "en_US"]);

        // Windows/macOS fall back to the OS locales when no env vars are set.
        let candidates = collect_candidates_from_sources(
            None,
            |_| None,
            vec!["zh-Hans-CN".into(), "en-US".into()],
        );
        assert_eq!(candidates, vec!["zh-Hans-CN", "en-US"]);
    }

    #[test]
    fn test_format_template_single_pass() {
        // Ensures values containing placeholder syntax are NOT re-substituted.
        let formatted = format_template("Installing {name}: {error}", |name| match name {
            "name" => Some("{error}"),
            "error" => Some("boom"),
            _ => None,
        });
        assert_eq!(formatted, "Installing {error}: boom");

        // Escapes are honoured, whether or not arguments are supplied.
        assert_eq!(format_template("{{literal}}", |_| None), "{literal}");
        assert_eq!(format_template("a }} b", |_| None), "a } b");

        // Unknown placeholder preserved verbatim.
        assert_eq!(
            format_template("Hello {unknown}", |_| None),
            "Hello {unknown}"
        );

        // Unterminated placeholder keeps the remainder.
        assert_eq!(format_template("Hello {name", |_| Some("x")), "Hello {name");

        // Text without braces is returned untouched.
        assert_eq!(format_template("plain text", |_| None), "plain text");
    }

    #[test]
    fn test_translation_lookup_and_fallback() {
        let locales = available_locales();
        assert!(locales.contains(&default_code()));
        assert!(locales.contains(&"zh_CN"));
        assert!(locales.contains(&"zh_HK"));
        assert!(locales.contains(&"zh_TW"));
        assert!(locales.windows(2).all(|pair| pair[0] < pair[1]));

        assert_eq!(
            t_for("en_US", "ui.no_plugins_found"),
            "No plugins found matching criteria."
        );
        assert_eq!(
            t_for("zh_CN", "ui.no_plugins_found"),
            "未找到符合条件的插件。"
        );
        assert_eq!(
            t_for("zh_HK", "ui.no_plugins_found"),
            "找不到符合條件的插件。"
        );
        assert_eq!(
            t_for("zh_TW", "ui.no_plugins_found"),
            "找不到符合條件的外掛。"
        );

        // Separator and case variants resolve to the same catalog.
        assert_eq!(
            t_for("zh-CN", "ui.no_plugins_found"),
            "未找到符合条件的插件。"
        );
        assert_eq!(
            t_for("zh_CN.UTF-8", "ui.no_plugins_found"),
            "未找到符合条件的插件。"
        );

        // Unsupported locale falls back to the default catalog.
        assert_eq!(
            t_for("fr_FR", "ui.no_plugins_found"),
            "No plugins found matching criteria."
        );

        // Unknown key returns the key itself.
        assert_eq!(t_for("zh_CN", "nonexistent.key"), "nonexistent.key");

        // Plain lookups unescape `{{`/`}}` the same way parameterized lookups do.
        assert_eq!(render_plain("literal {{brace}}"), "literal {brace}");

        let fill = |locale: &str, key: &str, args: &[(&str, &str)]| {
            let idx = index_for(locale).unwrap_or_else(default_index);
            fill_template_by_index(idx, key, args)
        };

        // Parameterized translation, including values that contain braces.
        assert_eq!(
            fill("zh_CN", "install.failed", &[("plugin", "AppleSkin")]),
            "安装 'AppleSkin' 失败"
        );
        assert_eq!(
            fill("en_US", "install.failed", &[("plugin", "{braces}")]),
            "Failed to install '{braces}'"
        );
        // A missing argument is kept verbatim rather than producing an empty string.
        assert_eq!(
            fill("en_US", "install.failed", &[]),
            "Failed to install '{plugin}'"
        );
    }

    #[test]
    fn test_all_catalogs_have_matching_keys_and_placeholders() {
        let default_table = tables()[default_index()].1;
        let default_keys: BTreeSet<&str> = default_table.iter().map(|(key, _)| *key).collect();

        assert_eq!(tables()[default_index()].0, default_code());
        assert!(
            !default_keys.is_empty(),
            "default catalog must not be empty"
        );
        assert!(
            default_table.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "default catalog keys must be sorted for binary search"
        );

        for (code, table) in tables() {
            let keys: BTreeSet<&str> = table.iter().map(|(key, _)| *key).collect();
            assert_eq!(
                keys, default_keys,
                "locale '{code}' has a different key set"
            );
            assert!(
                table.windows(2).all(|pair| pair[0].0 < pair[1].0),
                "locale '{code}' keys must be sorted for binary search"
            );

            for (key, message) in table.iter() {
                assert!(
                    !message.trim().is_empty(),
                    "locale '{code}' has an empty message for '{key}'"
                );
            }
        }
    }

    /// Guards against a `tr!` call whose key is absent from the catalogs:
    /// such a call would print the raw key to users.
    #[test]
    fn test_every_tr_key_exists_in_catalogs() {
        let default_table = tables()[default_index()].1;
        let default_keys: BTreeSet<&str> = default_table.iter().map(|(key, _)| *key).collect();

        let mut missing = BTreeSet::new();
        for source in rust_sources() {
            for key in tr_keys(&source) {
                if !default_keys.contains(key.as_str()) {
                    missing.insert(key);
                }
            }
        }

        assert!(
            missing.is_empty(),
            "these keys are used by tr! but missing from locales/{}.json: {missing:?}",
            default_code()
        );
    }

    /// Guards against orphaned keys left behind in `locales/*.json` after refactors.
    #[test]
    fn test_every_catalog_key_is_used() {
        let default_table = tables()[default_index()].1;
        let sources = rust_sources();

        let mut unused = BTreeSet::new();
        for (key, _) in default_table.iter() {
            let quoted = format!("\"{key}\"");
            if !sources.iter().any(|src| src.contains(&quoted)) {
                unused.insert(*key);
            }
        }

        assert!(
            unused.is_empty(),
            "these keys exist in locales/{}.json but are never referenced in src/: {unused:?}",
            default_code()
        );
    }

    fn rust_sources() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sources = Vec::new();
        let mut pending = vec![root];

        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs")
                    && let Ok(content) = std::fs::read_to_string(&path)
                {
                    sources.push(content);
                }
            }
        }

        assert!(!sources.is_empty(), "no Rust sources found to scan");
        sources
    }

    /// Extracts the key literal of every `tr!` call that takes one.
    fn tr_keys(source: &str) -> Vec<String> {
        // Written via `concat!` so the needle literal itself does not appear in
        // this file's source and cannot be mistaken for a `tr!` call.
        let needle = concat!("tr", "!(");
        let mut keys = Vec::new();
        let mut rest = source;

        while let Some(index) = rest.find(needle) {
            let inside_string_literal = rest[..index].ends_with('"');
            let after = &rest[index + needle.len()..];
            rest = after;

            if inside_string_literal {
                continue;
            }

            let trimmed = after.trim_start();
            let Some(literal) = trimmed.strip_prefix('"') else {
                continue;
            };
            let Some(end) = literal.find('"') else {
                continue;
            };

            keys.push(literal[..end].to_string());
        }

        keys
    }
}
