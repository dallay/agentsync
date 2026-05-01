use colored::Colorize;
use std::io::IsTerminal;

/// Semantic kind for a status label. Determines color when color is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Info,
    Warning,
    Success,
    Failure,
    #[allow(dead_code)]
    Muted,
}

/// Output mode for commands that support machine-readable JSON and human output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Human { use_color: bool },
}

/// Formatter for human-readable command output.
///
/// When `use_color` is true, applies ANSI styling via the `colored` crate.
/// When false, returns plain text preserving Unicode symbols.
#[derive(Debug, Clone, Copy)]
pub struct HumanFormatter {
    use_color: bool,
}

impl HumanFormatter {
    pub fn new(use_color: bool) -> Self {
        Self { use_color }
    }

    /// Format a status label: "{symbol} {label}" with color per kind.
    pub fn format_label(&self, symbol: &str, label: &str, kind: LabelKind) -> String {
        let text = format!("{symbol} {label}");
        self.style_by_kind(text, kind, true)
    }

    /// Format a heading: bold when color is enabled, plain otherwise.
    pub fn format_heading(&self, heading: &str) -> String {
        if self.use_color {
            heading.bold().to_string()
        } else {
            heading.to_string()
        }
    }

    /// Format subdued text for details that should not dominate the output.
    #[allow(dead_code)]
    pub fn format_muted(&self, text: &str) -> String {
        if self.use_color {
            text.dimmed().to_string()
        } else {
            text.to_string()
        }
    }

    /// Format a key/value line as "label: value", with a muted label when color is enabled.
    #[allow(dead_code)]
    pub fn format_key_value(&self, label: &str, value: &str) -> String {
        let formatted_label = if self.use_color {
            label.dimmed().to_string()
        } else {
            label.to_string()
        };
        format!("{formatted_label}: {value}")
    }

    /// Format a summary line as "label: value", styling the value by semantic kind.
    #[allow(dead_code)]
    pub fn format_summary_line(&self, label: &str, value: &str, kind: LabelKind) -> String {
        let formatted_value = self.style_by_kind(value.to_string(), kind, false);
        self.format_key_value(label, &formatted_value)
    }

    /// Format a hint line using the existing arrow convention.
    #[allow(dead_code)]
    pub fn format_hint(&self, text: &str) -> String {
        let arrow = if self.use_color {
            "↳".blue().to_string()
        } else {
            "↳".to_string()
        };
        format!("{arrow} {text}")
    }

    fn style_by_kind(&self, text: String, kind: LabelKind, bold: bool) -> String {
        if !self.use_color {
            return text;
        }

        match (kind, bold) {
            (LabelKind::Info, true) => text.cyan().bold().to_string(),
            (LabelKind::Warning, true) => text.yellow().bold().to_string(),
            (LabelKind::Success, true) => text.green().bold().to_string(),
            (LabelKind::Failure, true) => text.red().bold().to_string(),
            (LabelKind::Muted, true) => text.dimmed().bold().to_string(),
            (LabelKind::Info, false) => text.cyan().to_string(),
            (LabelKind::Warning, false) => text.yellow().to_string(),
            (LabelKind::Success, false) => text.green().to_string(),
            (LabelKind::Failure, false) => text.red().to_string(),
            (LabelKind::Muted, false) => text.dimmed().to_string(),
        }
    }
}

/// Detect output mode from CLI flags and environment.
///
/// Pure function — accepts pre-read values for testability.
/// The `json` flag takes absolute priority.
/// Color is enabled only when stdout is a TTY AND none of the
/// NO_COLOR / CLICOLOR=0 / TERM=dumb overrides are active.
pub fn detect_output_mode(
    json: bool,
    stdout_is_tty: bool,
    no_color: Option<&str>,
    clicolor: Option<&str>,
    term: Option<&str>,
) -> OutputMode {
    if json {
        return OutputMode::Json;
    }
    let no_color_set = no_color.is_some_and(|v| !v.is_empty());
    let clicolor_zero = clicolor.is_some_and(|v| v == "0");
    let dumb_term = term.is_some_and(|v| v.eq_ignore_ascii_case("dumb"));
    let use_color = stdout_is_tty && !dumb_term && !no_color_set && !clicolor_zero;
    OutputMode::Human { use_color }
}

/// Convenience wrapper that reads real environment state.
pub fn output_mode(json: bool) -> OutputMode {
    detect_output_mode(
        json,
        std::io::stdout().is_terminal(),
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("CLICOLOR").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Force the `colored` crate to emit ANSI codes even when stdout is not a TTY
    /// (e.g., inside `cargo test` which pipes stdout).
    ///
    /// Does NOT unset on drop — the `HumanFormatter(use_color: false)` tests
    /// short-circuit before calling the `colored` crate, so the global override
    /// doesn't affect them. Avoiding unset prevents race conditions between
    /// parallel tests toggling the global flag.
    fn force_color() {
        colored::control::set_override(true);
    }

    #[test]
    fn format_label_success_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_label("✔", "installed", LabelKind::Success);
        assert!(result.contains("✔ installed"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_label_success_plain() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_label("✔", "installed", LabelKind::Success);
        assert_eq!(result, "✔ installed");
        assert!(
            !result.contains("\x1b["),
            "unexpected ANSI escape: {result}"
        );
    }

    #[test]
    fn format_label_failure_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_label("✗", "failed", LabelKind::Failure);
        assert!(result.contains("✗ failed"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_label_failure_plain() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_label("✗", "failed", LabelKind::Failure);
        assert_eq!(result, "✗ failed");
        assert!(
            !result.contains("\x1b["),
            "unexpected ANSI escape: {result}"
        );
    }

    #[test]
    fn format_label_info_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_label("i", "resolving", LabelKind::Info);
        assert!(result.contains("i resolving"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_label_info_plain() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_label("i", "resolving", LabelKind::Info);
        assert_eq!(result, "i resolving");
    }

    #[test]
    fn format_label_warning_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_label("!", "warn", LabelKind::Warning);
        assert!(result.contains("! warn"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_label_warning_plain() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_label("!", "warn", LabelKind::Warning);
        assert_eq!(result, "! warn");
    }

    #[test]
    fn format_label_muted_plain() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_label("○", "skipped", LabelKind::Muted);
        assert_eq!(result, "○ skipped");
    }

    #[test]
    fn format_label_each_kind_uses_distinct_color() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let info = fmt.format_label("i", "info", LabelKind::Info);
        let warn = fmt.format_label("!", "warn", LabelKind::Warning);
        let ok = fmt.format_label("✔", "ok", LabelKind::Success);
        let fail = fmt.format_label("✗", "fail", LabelKind::Failure);
        let muted = fmt.format_label("○", "muted", LabelKind::Muted);

        for c in [&info, &warn, &ok, &fail, &muted] {
            assert!(c.contains("\x1b["), "{c}");
        }

        let unique: HashSet<&String> = [&info, &warn, &ok, &fail, &muted].into_iter().collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn format_heading_bold_when_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_heading("Summary");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
        assert!(result.contains("Summary"), "{result}");
    }

    #[test]
    fn format_heading_plain_when_no_color() {
        let fmt = HumanFormatter::new(false);
        let result = fmt.format_heading("Summary");
        assert_eq!(result, "Summary");
    }

    #[test]
    fn format_muted_plain_when_no_color() {
        let fmt = HumanFormatter::new(false);
        assert_eq!(fmt.format_muted("details"), "details");
    }

    #[test]
    fn format_muted_dimmed_when_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_muted("details");
        assert!(result.contains("details"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_key_value_plain() {
        let fmt = HumanFormatter::new(false);
        assert_eq!(fmt.format_key_value("Created", "2"), "Created: 2");
    }

    #[test]
    fn format_key_value_dims_label_when_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_key_value("Created", "2");
        assert!(result.contains("Created"), "{result}");
        assert!(result.contains(": 2"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_summary_line_plain() {
        let fmt = HumanFormatter::new(false);
        assert_eq!(
            fmt.format_summary_line("Errors", "0", LabelKind::Success),
            "Errors: 0"
        );
    }

    #[test]
    fn format_summary_line_colors_value_when_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_summary_line("Errors", "0", LabelKind::Success);
        assert!(result.contains("Errors"), "{result}");
        assert!(result.contains("0"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn format_hint_plain() {
        let fmt = HumanFormatter::new(false);
        assert_eq!(
            fmt.format_hint("Run agentsync apply"),
            "↳ Run agentsync apply"
        );
    }

    #[test]
    fn format_hint_colors_arrow_when_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let result = fmt.format_hint("Run agentsync apply");
        assert!(result.contains("↳"), "{result}");
        assert!(result.contains("Run agentsync apply"), "{result}");
        assert!(result.contains("\x1b["), "expected ANSI escape: {result}");
    }

    #[test]
    fn detect_output_mode_json_takes_priority() {
        assert_eq!(
            detect_output_mode(true, true, None, None, Some("xterm")),
            OutputMode::Json
        );
    }

    #[test]
    fn detect_output_mode_json_ignores_env_overrides() {
        assert_eq!(
            detect_output_mode(true, false, Some("1"), Some("0"), Some("dumb")),
            OutputMode::Json
        );
    }

    #[test]
    fn detect_output_mode_tty_with_color() {
        assert_eq!(
            detect_output_mode(false, true, None, None, Some("xterm-256color")),
            OutputMode::Human { use_color: true }
        );
    }

    #[test]
    fn detect_output_mode_no_tty_no_color() {
        assert_eq!(
            detect_output_mode(false, false, None, None, Some("xterm-256color")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_no_color_env() {
        assert_eq!(
            detect_output_mode(false, true, Some("1"), None, Some("xterm")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_empty_no_color_env_does_not_disable() {
        assert_eq!(
            detect_output_mode(false, true, Some(""), None, Some("xterm")),
            OutputMode::Human { use_color: true }
        );
    }

    #[test]
    fn detect_output_mode_clicolor_zero() {
        assert_eq!(
            detect_output_mode(false, true, None, Some("0"), Some("xterm")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_clicolor_nonzero_keeps_color() {
        assert_eq!(
            detect_output_mode(false, true, None, Some("1"), Some("xterm")),
            OutputMode::Human { use_color: true }
        );
    }

    #[test]
    fn detect_output_mode_term_dumb() {
        assert_eq!(
            detect_output_mode(false, true, None, None, Some("dumb")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_term_dumb_case_insensitive() {
        assert_eq!(
            detect_output_mode(false, true, None, None, Some("DUMB")),
            OutputMode::Human { use_color: false }
        );
    }
}
