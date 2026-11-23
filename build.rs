use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Preferred default locale; used for catalogs and as the fallback language.
const PREFERRED_DEFAULT: &str = "en_US";

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let locales_dir = Path::new(&manifest_dir).join("locales");

    println!("cargo:rerun-if-changed=locales");

    let files = collect_locale_files(&locales_dir);
    let catalogs = load_catalogs(&files);
    let default_code = pick_default_locale(&catalogs);

    validate_catalogs(&catalogs, &default_code);

    let generated = render_tables(&catalogs, &default_code);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let destination = Path::new(&out_dir).join("locales_generated.rs");
    fs::write(&destination, generated)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", destination.display()));
}

/// Lists `locales/*.json` as `(code, path)` pairs sorted by locale code.
fn collect_locale_files(locales_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();

    if locales_dir.is_dir() {
        let entries = fs::read_dir(locales_dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", locales_dir.display()));

        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("failed to read a locales/ entry: {error}"))
                .path();

            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }

            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("locale file name must be valid UTF-8")
                .to_string();

            let code = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("locale file stem must be valid UTF-8")
                .to_string();

            assert!(
                !code.is_empty()
                    && code
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "invalid translation file '{file_name}': the file stem must contain only ASCII \
                 alphanumeric characters, '_' or '-'"
            );

            println!("cargo:rerun-if-changed=locales/{file_name}");
            files.push((code, path));
        }
    }

    assert!(
        !files.is_empty(),
        "no translation files found in '{}'; at least 'en_US.json' is required",
        locales_dir.display()
    );

    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn load_catalogs(files: &[(String, PathBuf)]) -> Vec<(String, BTreeMap<String, String>)> {
    files
        .iter()
        .map(|(code, path)| {
            let content = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("failed to read locales/{code}.json: {error}"));

            let catalog: BTreeMap<String, String> =
                serde_json::from_str(&content).unwrap_or_else(|error| {
                    panic!("locales/{code}.json must be a JSON object of string values: {error}")
                });

            (code.clone(), catalog)
        })
        .collect()
}

/// Chooses the fallback locale: `en_US`, otherwise the first English catalog,
/// otherwise the first catalog in alphabetical order.
fn pick_default_locale(catalogs: &[(String, BTreeMap<String, String>)]) -> String {
    if catalogs.iter().any(|(code, _)| code == PREFERRED_DEFAULT) {
        return PREFERRED_DEFAULT.to_string();
    }

    if let Some((code, _)) = catalogs.iter().find(|(code, _)| language_of(code) == "en") {
        return code.clone();
    }

    catalogs
        .first()
        .map(|(code, _)| code.clone())
        .unwrap_or_else(|| PREFERRED_DEFAULT.to_string())
}

fn validate_catalogs(catalogs: &[(String, BTreeMap<String, String>)], default_code: &str) {
    let default_catalog = catalogs
        .iter()
        .find(|(code, _)| code == default_code)
        .map(|(_, catalog)| catalog)
        .unwrap_or_else(|| panic!("default locale '{default_code}' has no translation file"));

    for (key, value) in default_catalog {
        assert!(
            !key.is_empty(),
            "locales/{default_code}.json has an empty key"
        );
        assert!(
            !value.trim().is_empty(),
            "locales/{default_code}.json has an empty message for '{key}'"
        );
    }

    for (code, catalog) in catalogs {
        for key in default_catalog.keys() {
            assert!(
                catalog.contains_key(key),
                "locales/{code}.json is missing the key '{key}'"
            );
        }

        for (key, value) in catalog {
            assert!(
                !value.trim().is_empty(),
                "locales/{code}.json has an empty message for '{key}'"
            );

            let Some(default_value) = default_catalog.get(key) else {
                panic!(
                    "locales/{code}.json has a key '{key}' that is missing from '{default_code}'"
                );
            };

            let expected = placeholders(default_value);
            let actual = placeholders(value);
            assert_eq!(
                actual, expected,
                "locales/{code}.json uses different placeholders than '{default_code}' for '{key}'"
            );
        }
    }
}

/// Renders the translation tables as Rust source with zero runtime parsing cost.
fn render_tables(catalogs: &[(String, BTreeMap<String, String>)], default_code: &str) -> String {
    let default_index = catalogs
        .iter()
        .position(|(code, _)| code == default_code)
        .unwrap_or_else(|| panic!("default locale '{default_code}' not found in catalogs"));

    let mut generated = String::new();
    generated.push_str("// @generated by build.rs from locales/*.json; do not edit.\n\n");
    generated.push_str(&format!(
        "pub const DEFAULT_LOCALE_INDEX: usize = {default_index};\n\n"
    ));
    generated.push_str("pub const LOCALE_CODES: &[&str] = &[\n");
    for (code, _) in catalogs {
        generated.push_str(&format!("    {code:?},\n"));
    }
    generated.push_str("];\n\n");
    generated.push_str("pub const LOCALES: &[(&str, &[(&str, &str)])] = &[\n");

    for (code, catalog) in catalogs {
        generated.push_str(&format!("    ({code:?}, &[\n"));
        for (key, value) in catalog {
            generated.push_str(&format!("        ({key:?}, {value:?}),\n"));
        }
        generated.push_str("    ]),\n");
    }

    generated.push_str("];\n");
    generated
}

fn language_of(code: &str) -> String {
    code.split(['_', '-'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Collects `{placeholder}` names; `{{` and `}}` are escapes, matching the
/// runtime formatter in `src/i18n`.
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut chars = text.char_indices().peekable();

    while let Some((start, character)) = chars.next() {
        if character != '{' {
            continue;
        }

        if matches!(chars.peek(), Some(&(_, '{'))) {
            chars.next();
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
            Some(end) => {
                found.insert(text[start + 1..end].to_string());
            }
            None => break,
        }
    }

    found
}
