use std::error::Error as StdError;
use std::fmt::{Display, Write as _};

use clap::Command;
use clap::builder::{StyledStr, Styles};
use clap::error::{ContextKind, ContextValue, ErrorKind};

use crate::tr;

/// Labels `clap` prints around localized content.
struct Labels {
    usage: String,
    error: String,
    tip: String,
}

impl Labels {
    fn load() -> Self {
        Self {
            usage: tr!("clap.usage"),
            error: tr!("clap.error_prefix"),
            tip: tr!("clap.tip"),
        }
    }
}

/// The localized replacement table for clap's built-in chrome.
struct Translations {
    labels: Labels,
    /// Section headings (`Commands:`, `Arguments:`, `Options:`).
    headings: Vec<(&'static str, String)>,
    /// Description of clap's built-in `help` subcommand.
    builtin: Vec<(&'static str, String)>,
    /// Bracketed annotations appended by `clap` (`[env: `, `[default: `, …).
    annotations: Vec<(&'static str, String)>,
}

impl Translations {
    fn new() -> Self {
        // Help/version arguments are supplied by `localized_command`; only the
        // descriptions of clap's generated `help` subcommand remain.
        let mut builtin: Vec<(&'static str, String)> = vec![
            (
                "Print this message or the help of the given subcommand(s)",
                tr!("cli.cmd.help"),
            ),
            ("Print help for the subcommand(s)", tr!("cli.help.command")),
        ];
        builtin.retain(|(source, target)| *source != target.as_str());

        let mut headings: Vec<(&'static str, String)> = vec![
            ("Commands:", tr!("clap.commands")),
            ("Arguments:", tr!("clap.arguments")),
            ("Options:", tr!("clap.options")),
        ];
        headings.retain(|(source, target)| *source != target.as_str());

        let mut annotations: Vec<(&'static str, String)> = vec![
            ("[aliases: ", tr!("clap.aliases")),
            ("[alias: ", tr!("clap.alias")),
            ("[env: ", tr!("clap.env")),
            ("[default: ", tr!("clap.default")),
            ("[possible values: ", tr!("clap.possible_values")),
            ("[subcommands: ", tr!("clap.subcommands")),
        ];
        annotations.retain(|(source, target)| *source != target.as_str());

        Self {
            labels: Labels::load(),
            headings,
            builtin,
            annotations,
        }
    }

    fn is_identity(&self) -> bool {
        self.labels.usage == "Usage:"
            && self.headings.is_empty()
            && self.builtin.is_empty()
            && self.annotations.is_empty()
    }

    /// Rewrites the chrome of an already-rendered help or usage block.
    ///
    /// Headings and `Usage:` are matched only at line start (ignoring any
    /// leading ANSI CSI sequences), so mid-line descriptions cannot be altered.
    fn apply(&self, text: &str) -> String {
        if self.is_identity() {
            return text.to_string();
        }

        let mut out = String::with_capacity(text.len() + 64);

        for (index, raw_line) in text.split('\n').enumerate() {
            if index > 0 {
                out.push('\n');
            }

            let mut line = self.rewrite_line_heading(raw_line);

            for (source, target) in &self.builtin {
                if line.contains(*source) {
                    line = line.replace(*source, target);
                }
            }

            for (source, target) in &self.annotations {
                if line.contains(*source) {
                    line = line.replace(*source, target);
                }
            }

            out.push_str(&line);
        }

        out
    }

    /// Rewrites a standalone section heading or leading `Usage:` label on `line`.
    fn rewrite_line_heading(&self, line: &str) -> String {
        let start = skip_leading_ansi(line);
        let rest = &line[start..];

        // Leading `Usage:` line (with or without ANSI reset sequences between
        // the label and the command synopsis).
        if self.labels.usage != "Usage:"
            && let Some(after_usage) = rest.strip_prefix("Usage:")
        {
            let reset_len = skip_leading_ansi(after_usage);
            let mut tail = &after_usage[reset_len..];
            if ends_with_fullwidth_colon(&self.labels.usage) {
                tail = tail.strip_prefix(' ').unwrap_or(tail);
            }

            let mut rewritten = String::with_capacity(line.len() + self.labels.usage.len());
            rewritten.push_str(&line[..start]);
            rewritten.push_str(&self.labels.usage);
            rewritten.push_str(&after_usage[..reset_len]);
            rewritten.push_str(tail);
            return rewritten;
        }

        // Standalone section heading line such as `Commands:`, `Arguments:`, `Options:`.
        for (source, target) in &self.headings {
            if let Some(after_heading) = rest.strip_prefix(*source) {
                let trailing_ansi = skip_leading_ansi(after_heading);
                if after_heading[trailing_ansi..]
                    .trim_end_matches('\r')
                    .is_empty()
                {
                    let mut rewritten = String::with_capacity(line.len() + target.len());
                    rewritten.push_str(&line[..start]);
                    rewritten.push_str(target);
                    rewritten.push_str(after_heading);
                    return rewritten;
                }
            }
        }

        line.to_string()
    }
}

/// Renders a `clap::Error` for display, localizing help, version, and failure text.
///
/// The returned text keeps `clap`'s ANSI styling only when `styled` is true, so
/// piped output and `NO_COLOR` environments stay plain.
pub(super) fn render(error: &clap::Error, cmd: &Command, styled: bool) -> String {
    let translations = Translations::new();

    match error.kind() {
        // Help and version are rendered content that `clap` already produced
        // from the localized command tree; only the chrome needs translating.
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        | ErrorKind::DisplayVersion => translations.apply(&styled_text(&error.render(), styled)),

        _ => {
            let styles = cmd.get_styles().clone();
            let mut out = String::new();

            out.push_str(&paint(
                styles.get_error(),
                &translations.labels.error,
                styled,
            ));
            if !ends_with_fullwidth_colon(&translations.labels.error) {
                out.push(' ');
            }
            out.push_str(&describe(error, &styles, styled));

            append_suggestions(&mut out, error, &translations.labels.tip, &styles, styled);

            // Mirror clap's RichFormatter: emit Usage only when the error itself
            // carries one (e.g. missing/unknown args or subcommands, using the
            // exact failing subcommand's usage), never fabricating the root usage
            // on value-validation failures.
            if let Some(ContextValue::StyledStr(usage)) = error.get(ContextKind::Usage) {
                let usage = styled_text(usage, styled);
                if !usage.is_empty() {
                    out.push_str("\n\n");
                    out.push_str(&translations.apply(&usage));
                }
            }

            if let Some(flag) = help_flag(cmd) {
                out.push_str("\n\n");
                out.push_str(&tr!(
                    "clap.more_info",
                    flag = paint(styles.get_literal(), &flag, styled)
                ));
            }

            out.push('\n');
            out
        }
    }
}

/// Converts a `StyledStr` to text, preserving escape sequences only when styled.
fn styled_text(styled: &StyledStr, keep_style: bool) -> String {
    if keep_style {
        styled.ansi().to_string()
    } else {
        styled.to_string()
    }
}

/// Wraps `text` in an `anstyle::Style` when `styled` is true.
fn paint(style: impl Display, text: &str, styled: bool) -> String {
    if styled {
        format!("{style}{text}{style:#}")
    } else {
        text.to_string()
    }
}

/// Whether `label` already terminates with a full-width CJK colon that needs no
/// following ASCII space.
fn ends_with_fullwidth_colon(label: &str) -> bool {
    label.ends_with('：')
}

/// Returns the byte offset past any leading ANSI CSI sequences (`\x1b[...<final>`).
fn skip_leading_ansi(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut index = 0;

    while index + 1 < bytes.len() && bytes[index] == 0x1b && bytes[index + 1] == b'[' {
        let mut cursor = index + 2;
        while cursor < bytes.len() && !(0x40..=0x7e).contains(&bytes[cursor]) {
            cursor += 1;
        }
        if cursor < bytes.len() {
            index = cursor + 1;
        } else {
            break;
        }
    }

    index
}

fn string_ctx(error: &clap::Error, kind: ContextKind) -> Option<&str> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.as_str()),
        _ => None,
    }
}

fn number_ctx(error: &clap::Error, kind: ContextKind) -> Option<isize> {
    match error.get(kind) {
        Some(ContextValue::Number(value)) => Some(*value),
        _ => None,
    }
}

/// Appends a bracketed list such as `  [possible values: a, b]` when values exist.
fn append_values_list(
    out: &mut String,
    prefix: &str,
    values: Option<&ContextValue>,
    styles: &Styles,
    styled: bool,
) {
    let Some(ContextValue::Strings(items)) = values else {
        return;
    };
    if items.is_empty() {
        return;
    }

    let valid = styles.get_valid();
    out.push_str("\n  ");
    out.push_str(prefix);
    for (idx, item) in items.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&paint(valid, item, styled));
    }
    out.push(']');
}

/// Appends localized "tip: did you mean …" suggestions carried on `error`.
fn append_suggestions(
    out: &mut String,
    error: &clap::Error,
    tip_label: &str,
    styles: &Styles,
    styled: bool,
) {
    let valid = styles.get_valid();
    let tip_prefix = if ends_with_fullwidth_colon(tip_label) {
        paint(valid, tip_label, styled)
    } else {
        format!("{} ", paint(valid, tip_label, styled))
    };

    let mut had_tip = false;
    for kind in [
        ContextKind::SuggestedSubcommand,
        ContextKind::SuggestedArg,
        ContextKind::SuggestedValue,
    ] {
        let Some(suggestion) = error.get(kind) else {
            continue;
        };

        let formatted = match suggestion {
            ContextValue::String(value) => Some(paint(valid, value, styled)),
            ContextValue::Strings(values) if !values.is_empty() => Some(
                values
                    .iter()
                    .map(|value| paint(valid, value, styled))
                    .collect::<Vec<_>>()
                    .join("', '"),
            ),
            _ => None,
        };

        if let Some(value) = formatted {
            out.push_str(if had_tip { "\n" } else { "\n\n" });
            had_tip = true;
            let _ = write!(
                out,
                "  {tip_prefix}{}",
                tr!("clap.did_you_mean", value = value)
            );
        }
    }

    if let Some(ContextValue::StyledStrs(tips)) = error.get(ContextKind::Suggested) {
        for tip in tips {
            out.push_str(if had_tip { "\n" } else { "\n\n" });
            had_tip = true;
            let _ = write!(out, "  {tip_prefix}{}", styled_text(tip, styled));
        }
    }
}

/// Builds the localized explanation for a parse failure.
fn describe(error: &clap::Error, styles: &Styles, styled: bool) -> String {
    let invalid = styles.get_invalid();
    let valid = styles.get_valid();
    let literal = styles.get_literal();

    match error.kind() {
        ErrorKind::UnknownArgument => string_ctx(error, ContextKind::InvalidArg).map(|arg| {
            tr!(
                "clap.err.unexpected_argument",
                arg = paint(invalid, arg, styled)
            )
        }),
        ErrorKind::InvalidSubcommand => {
            string_ctx(error, ContextKind::InvalidSubcommand).map(|sub| {
                tr!(
                    "clap.err.unrecognized_subcommand",
                    sub = paint(invalid, sub, styled)
                )
            })
        }
        ErrorKind::MissingSubcommand => {
            string_ctx(error, ContextKind::InvalidSubcommand).map(|cmd| {
                let mut msg = tr!(
                    "clap.err.missing_subcommand",
                    cmd = paint(invalid, cmd, styled)
                );
                append_values_list(
                    &mut msg,
                    &tr!("clap.subcommands"),
                    error.get(ContextKind::ValidSubcommand),
                    styles,
                    styled,
                );
                msg
            })
        }
        ErrorKind::MissingRequiredArgument => match error.get(ContextKind::InvalidArg) {
            Some(ContextValue::Strings(items)) if !items.is_empty() => {
                let mut msg = tr!("clap.err.missing_required");
                for item in items {
                    let _ = write!(msg, "\n  {}", paint(valid, item, styled));
                }
                Some(msg)
            }
            Some(ContextValue::String(item)) => Some(format!(
                "{}\n  {}",
                tr!("clap.err.missing_required"),
                paint(valid, item, styled)
            )),
            _ => None,
        },
        ErrorKind::InvalidValue | ErrorKind::ValueValidation => match (
            string_ctx(error, ContextKind::InvalidArg),
            string_ctx(error, ContextKind::InvalidValue),
        ) {
            (Some(arg), Some(value)) => {
                let validation = error.kind() == ErrorKind::ValueValidation;
                let mut msg = if value.is_empty() && !validation {
                    tr!("clap.err.value_required", arg = paint(invalid, arg, styled))
                } else {
                    tr!(
                        "clap.err.invalid_value",
                        arg = paint(literal, arg, styled),
                        value = paint(invalid, value, styled)
                    )
                };
                if validation {
                    if let Some(source) = StdError::source(error) {
                        let _ = write!(msg, ": {source}");
                    }
                } else {
                    append_values_list(
                        &mut msg,
                        &tr!("clap.possible_values"),
                        error.get(ContextKind::ValidValue),
                        styles,
                        styled,
                    );
                }
                Some(msg)
            }
            _ => None,
        },
        ErrorKind::ArgumentConflict => string_ctx(error, ContextKind::InvalidArg).map(|raw_arg| {
            let arg = paint(invalid, raw_arg, styled);
            match error.get(ContextKind::PriorArg) {
                Some(ContextValue::String(prior)) if prior == raw_arg => {
                    tr!("clap.err.argument_duplicate", arg = arg)
                }
                Some(ContextValue::String(prior)) => format!(
                    "{} '{}'",
                    tr!("clap.err.argument_conflict", arg = arg),
                    paint(invalid, prior, styled)
                ),
                Some(ContextValue::Strings(priors)) => {
                    let mut msg = tr!("clap.err.argument_conflict", arg = arg);
                    for prior in priors {
                        let _ = write!(msg, "\n  {}", paint(invalid, prior, styled));
                    }
                    msg
                }
                _ => tr!("clap.err.argument_conflict", arg = arg),
            }
        }),
        ErrorKind::TooManyValues => match (
            string_ctx(error, ContextKind::InvalidArg),
            string_ctx(error, ContextKind::InvalidValue),
        ) {
            (Some(arg), Some(value)) => Some(tr!(
                "clap.err.too_many_values",
                arg = paint(literal, arg, styled),
                value = paint(invalid, value, styled)
            )),
            _ => None,
        },
        ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => {
            let bound_kind = if error.kind() == ErrorKind::TooFewValues {
                ContextKind::MinValues
            } else {
                ContextKind::ExpectedNumValues
            };
            match (
                string_ctx(error, ContextKind::InvalidArg),
                number_ctx(error, ContextKind::ActualNumValues),
                number_ctx(error, bound_kind),
            ) {
                (Some(arg), Some(actual), Some(bound)) => {
                    let arg = paint(literal, arg, styled);
                    let actual = paint(invalid, &actual.to_string(), styled);
                    let bound = paint(valid, &bound.to_string(), styled);
                    Some(if error.kind() == ErrorKind::TooFewValues {
                        tr!(
                            "clap.err.too_few_values",
                            arg = arg,
                            actual = actual,
                            min = bound
                        )
                    } else {
                        tr!(
                            "clap.err.wrong_number_values",
                            arg = arg,
                            actual = actual,
                            expected = bound
                        )
                    })
                }
                _ => None,
            }
        }
        ErrorKind::NoEquals => string_ctx(error, ContextKind::InvalidArg)
            .map(|arg| tr!("clap.err.no_equals", arg = paint(invalid, arg, styled))),
        _ => None,
    }
    .unwrap_or_else(|| tr!("clap.err.fallback"))
}

/// The help flag `clap` advertises in its "for more information" hint.
///
/// `clap` derives this from its built-in help argument, but that one is disabled
/// here in favour of a translated replacement, so the configured argument is
/// inspected instead.
fn help_flag(cmd: &Command) -> Option<String> {
    use clap::ArgAction;

    let help_arg = cmd.get_arguments().find(|arg| {
        matches!(
            arg.get_action(),
            ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong
        )
    });

    if let Some(arg) = help_arg {
        if let Some(long) = arg.get_long() {
            return Some(format!("--{long}"));
        }
        if let Some(short) = arg.get_short() {
            return Some(format!("-{short}"));
        }
    }

    if cmd.has_subcommands() && !cmd.is_disable_help_subcommand_set() {
        Some("help".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The active locale is process-global, so tests that switch it must not run
    /// concurrently with one another.
    static LOCALE_LOCK: Mutex<()> = Mutex::new(());

    /// Runs `body` for every catalog, then restores the default locale.
    fn for_each_locale(body: impl Fn(&str)) {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());

        for &locale in crate::i18n::available_locales() {
            assert_eq!(crate::i18n::init(Some(locale)), locale);
            body(locale);
        }

        crate::i18n::init(Some("en_US"));
    }

    fn first_error(args: &[&str]) -> (clap::Error, Command) {
        let mut cmd = crate::cli::localized_command();
        let error = cmd
            .try_get_matches_from_mut(args)
            .expect_err("expected a parse error");
        (error, cmd)
    }

    #[test]
    fn test_parse_errors_are_localized() {
        for_each_locale(|locale| {
            let prefix = crate::i18n::t("clap.error_prefix");
            let usage = crate::i18n::t("clap.usage");

            let (error, cmd) = first_error(&["ppm", "--bogus"]);
            let rendered = render(&error, &cmd, false);

            assert!(
                rendered.starts_with(prefix.trim_end()),
                "{locale}: wrong error prefix: {rendered}"
            );
            assert!(
                rendered.contains(usage.trim_end()),
                "{locale}: missing usage heading: {rendered}"
            );
            assert!(
                rendered.contains("'--bogus'"),
                "{locale}: argument name should be quoted: {rendered}"
            );

            if locale != "en_US" {
                assert!(
                    !rendered.contains("unexpected argument"),
                    "{locale}: untranslated message: {rendered}"
                );
                assert!(
                    !rendered.contains("For more information, try"),
                    "{locale}: untranslated hint: {rendered}"
                );
            }

            let (error, cmd) = first_error(&["ppm", "nope"]);
            let rendered = render(&error, &cmd, false);
            assert!(
                rendered.contains("nope"),
                "{locale}: subcommand name dropped: {rendered}"
            );
            if locale != "en_US" {
                assert!(
                    !rendered.contains("unrecognized subcommand"),
                    "{locale}: untranslated message: {rendered}"
                );
            }
        });
    }

    #[test]
    fn test_subcommand_errors_preserve_subcommand_usage_and_validation_cause() {
        for_each_locale(|locale| {
            // Missing required argument on a subcommand must show that subcommand's usage.
            let (error, cmd) = first_error(&["ppm", "uninstall"]);
            let rendered = render(&error, &cmd, false);
            assert!(
                rendered.contains("ppm uninstall"),
                "{locale}: expected subcommand usage, got:\n{rendered}"
            );
            assert!(
                rendered.contains("<PLUGINS>..."),
                "{locale}: expected missing argument list, got:\n{rendered}"
            );

            // Numeric parsing failure must retain its underlying validation cause
            // and must not fabricate the root command's usage line.
            let (error, cmd) = first_error(&["ppm", "search", "foo", "--limit", "abc"]);
            let rendered = render(&error, &cmd, false);
            assert!(
                rendered.contains("invalid digit"),
                "{locale}: expected validation error cause, got:\n{rendered}"
            );
            assert!(
                !rendered.contains("ppm [OPTIONS] <COMMAND>"),
                "{locale}: value validation should not emit root usage, got:\n{rendered}"
            );
        });
    }

    #[test]
    fn test_typo_suggestions_are_localized() {
        for_each_locale(|locale| {
            let tip = crate::i18n::t("clap.tip");

            let (error, cmd) = first_error(&["ppm", "serch"]);
            let rendered = render(&error, &cmd, false);
            assert!(
                rendered.contains(tip.trim_end()) && rendered.contains("'search'"),
                "{locale}: expected localized subcommand suggestion, got:\n{rendered}"
            );

            let (error, cmd) = first_error(&["ppm", "--locle", "zh_CN", "installed"]);
            let rendered = render(&error, &cmd, false);
            assert!(
                rendered.contains(tip.trim_end()) && rendered.contains("'--locale'"),
                "{locale}: expected localized argument suggestion, got:\n{rendered}"
            );
        });
    }

    #[test]
    fn test_styled_error_emits_ansi_only_when_requested() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::i18n::init(Some("zh_CN"));

        let (error, cmd) = first_error(&["ppm", "serch"]);
        let plain = render(&error, &cmd, false);
        let styled = render(&error, &cmd, true);

        assert!(
            !plain.contains('\x1b'),
            "plain output leaked ANSI: {plain:?}"
        );
        assert!(
            styled.contains('\x1b'),
            "styled output should retain ANSI escapes: {styled:?}"
        );
        assert!(
            styled.contains("用法：\x1b[0m\x1b[1mppm"),
            "styled full-width usage colon should not have an extra space: {styled:?}"
        );

        crate::i18n::init(Some("en_US"));
    }

    #[test]
    fn test_headings_are_only_rewritten_at_line_start() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        crate::i18n::init(Some("zh_CN"));

        let translations = Translations::new();
        let sample =
            "Description mentioning Options: mid-line\nOptions:\n\x1b[1m\x1b[4mCommands:\x1b[0m\n";
        let rewritten = translations.apply(sample);

        assert!(
            rewritten.contains("Description mentioning Options: mid-line"),
            "mid-line text was corrupted: {rewritten:?}"
        );
        assert!(
            rewritten.contains("\n选项：\n"),
            "standalone heading was not translated: {rewritten:?}"
        );
        assert!(
            rewritten.contains("\x1b[1m\x1b[4m命令：\x1b[0m\n"),
            "styled standalone heading was not translated: {rewritten:?}"
        );

        crate::i18n::init(Some("en_US"));
    }

    #[test]
    fn test_help_chrome_is_localized() {
        for_each_locale(|locale| {
            let (error, cmd) = first_error(&["ppm", "--help"]);
            let help = render(&error, &cmd, false);

            // Every chrome label must appear in the catalog's own wording.
            for key in [
                "clap.usage",
                "clap.commands",
                "clap.options",
                "clap.env",
                "clap.default",
                "clap.aliases",
                "cli.cmd.help",
                "cli.arg.version",
            ] {
                let expected = crate::i18n::t(key);
                assert!(
                    help.contains(expected.trim_end()),
                    "{locale}: help is missing {key} ({expected:?}):\n{help}"
                );
            }

            // A translated catalog must not fall back to the English chrome.
            if locale != "en_US" {
                for untranslated in [
                    "Usage:",
                    "Commands:",
                    "Options:",
                    "[env: ",
                    "[default: ",
                    "[alias: ",
                    "[aliases: ",
                    "Print this message or the help of the given subcommand(s)",
                ] {
                    assert!(
                        !help.contains(untranslated),
                        "{locale}: untranslated {untranslated:?}:\n{help}"
                    );
                }
            }
        });
    }

    #[test]
    fn test_version_output_is_untouched() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let (error, cmd) = first_error(&["ppm", "--version"]);
        let rendered = render(&error, &cmd, false);
        assert!(
            rendered.contains(concat!("ppm ", env!("CARGO_PKG_VERSION"))),
            "unexpected version output: {rendered}"
        );
    }

    #[test]
    fn test_parenthesised_help_labels_are_not_corrupted() {
        // Regression: replacing the bare `Print help` label first turned
        // `Print help (see …)` into a mix of English and translated text.
        for_each_locale(|locale| {
            let (error, cmd) = first_error(&["ppm", "-h"]);
            let help = render(&error, &cmd, false);

            if locale == "en_US" {
                return;
            }

            assert!(
                !help.contains("(see more with '--help')")
                    && !help.contains("(see a summary with '-h')"),
                "{locale}: parenthesised help label was corrupted:\n{help}"
            );
        });
    }

    #[test]
    fn test_subcommand_help_is_localized() {
        for_each_locale(|locale| {
            let (error, cmd) = first_error(&["ppm", "install", "--help"]);
            let help = render(&error, &cmd, false);

            assert!(
                help.contains(crate::i18n::t("clap.usage").trim_end()),
                "{locale}: missing usage heading:\n{help}"
            );
            assert!(
                help.contains(crate::i18n::t("clap.options").trim_end()),
                "{locale}: missing options heading:\n{help}"
            );
            assert!(
                help.contains(crate::i18n::t("cli.cmd.install").trim_end()),
                "{locale}: subcommand about not localized:\n{help}"
            );
        });
    }
}
