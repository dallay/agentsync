use colored::Colorize;
use std::io::IsTerminal;

/// Semantic kind for a status label. Determines color when color is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    Info,
    Warning,
    Success,
    Failure,
}

/// Output mode for single-operation skill commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Human { use_color: bool },
}

/// Formatter for human-readable skill command output.
/// When `use_color` is true, applies ANSI color+bold via the `colored` crate.
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
        if !self.use_color {
            return text;
        }

        match kind {
            LabelKind::Info => text.cyan().bold().to_string(),
            LabelKind::Warning => text.yellow().bold().to_string(),
            LabelKind::Success => text.green().bold().to_string(),
            LabelKind::Failure => text.red().bold().to_string(),
        }
    }

    /// Format a heading: bold when color is enabled, plain otherwise.
    pub fn format_heading(&self, heading: &str) -> String {
        if self.use_color {
            heading.bold().to_string()
        } else {
            heading.to_string()
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

    // --- HumanFormatter::format_label tests ---

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
    fn format_label_each_kind_uses_distinct_color() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let info = fmt.format_label("i", "info", LabelKind::Info);
        let warn = fmt.format_label("!", "warn", LabelKind::Warning);
        let ok = fmt.format_label("✔", "ok", LabelKind::Success);
        let fail = fmt.format_label("✗", "fail", LabelKind::Failure);
        // All contain ANSI escapes
        for c in [&info, &warn, &ok, &fail] {
            assert!(c.contains("\x1b["), "{c}");
        }
        // All four are distinct strings (different color codes)
        let unique: HashSet<&String> = [&info, &warn, &ok, &fail].into_iter().collect();
        assert_eq!(unique.len(), 4);
    }

    // --- HumanFormatter::format_heading tests ---

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

    // --- detect_output_mode tests ---

    #[test]
    fn detect_output_mode_json_takes_priority() {
        assert_eq!(
            detect_output_mode(true, true, None, None, Some("xterm")),
            OutputMode::Json
        );
    }

    #[test]
    fn detect_output_mode_json_ignores_env_overrides() {
        // Even with NO_COLOR, CLICOLOR=0, TERM=dumb — JSON wins
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
    fn detect_output_mode_no_color_any_nonempty_value() {
        assert_eq!(
            detect_output_mode(false, true, Some("yes"), None, Some("xterm")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_no_color_empty_value_allows_color() {
        // Empty NO_COLOR should NOT suppress color (spec: non-empty)
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
    fn detect_output_mode_clicolor_nonzero_allows_color() {
        assert_eq!(
            detect_output_mode(false, true, None, Some("1"), Some("xterm")),
            OutputMode::Human { use_color: true }
        );
    }

    #[test]
    fn detect_output_mode_dumb_term() {
        assert_eq!(
            detect_output_mode(false, true, None, None, Some("dumb")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_dumb_term_case_insensitive() {
        assert_eq!(
            detect_output_mode(false, true, None, None, Some("DUMB")),
            OutputMode::Human { use_color: false }
        );
    }

    #[test]
    fn detect_output_mode_no_term_env_allows_color_on_tty() {
        assert_eq!(
            detect_output_mode(false, true, None, None, None),
            OutputMode::Human { use_color: true }
        );
    }

    // --- Human-output pattern tests for install/update/uninstall ---

    #[test]
    fn install_success_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "my-skill";
        let line = format!(
            "{} {skill_id}",
            fmt.format_label("✔", "installed", LabelKind::Success)
        );
        assert_eq!(line, "✔ installed my-skill");
    }

    #[test]
    fn install_success_human_output_colored() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let skill_id = "my-skill";
        let line = format!(
            "{} {skill_id}",
            fmt.format_label("✔", "installed", LabelKind::Success)
        );
        assert!(line.contains("✔ installed"), "{line}");
        assert!(line.contains("my-skill"), "{line}");
        assert!(line.contains("\x1b["), "expected ANSI: {line}");
    }

    #[test]
    fn install_error_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "bad-skill";
        let err_msg = "source not found";
        let line1 = format!(
            "{} {skill_id}: {err_msg}",
            fmt.format_label("✗", "failed", LabelKind::Failure)
        );
        let line2 = format!("Hint: {}", "Check the SKILL.md syntax");
        assert_eq!(line1, "✗ failed bad-skill: source not found");
        assert!(line2.starts_with("Hint: "));
        assert!(!line2.contains("\x1b["), "Hint must not be colored");
    }

    #[test]
    fn update_success_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "my-skill";
        let line = format!(
            "{} {skill_id}",
            fmt.format_label("✔", "updated", LabelKind::Success)
        );
        assert_eq!(line, "✔ updated my-skill");
    }

    #[test]
    fn update_error_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "bad-skill";
        let err_msg = "update failed";
        let line1 = format!(
            "{} {skill_id}: {err_msg}",
            fmt.format_label("✗", "failed", LabelKind::Failure)
        );
        assert_eq!(line1, "✗ failed bad-skill: update failed");
    }

    #[test]
    fn uninstall_success_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "my-skill";
        let line = format!(
            "{} {skill_id}",
            fmt.format_label("✔", "uninstalled", LabelKind::Success)
        );
        assert_eq!(line, "✔ uninstalled my-skill");
    }

    #[test]
    fn uninstall_error_human_output_pattern() {
        let fmt = HumanFormatter::new(false);
        let skill_id = "missing-skill";
        let err_msg = "skill not found";
        let line1 = format!(
            "{} {skill_id}: {err_msg}",
            fmt.format_label("✗", "failed", LabelKind::Failure)
        );
        let line2 = "Hint: Try 'list' to verify installed skills";
        assert_eq!(line1, "✗ failed missing-skill: skill not found");
        assert!(line2.contains("list"), "hint should mention list");
        assert!(!line2.contains("\x1b["), "Hint must not be colored");
    }

    #[test]
    fn uninstall_error_colored_has_ansi_on_failure_line_only() {
        force_color();
        let fmt = HumanFormatter::new(true);
        let skill_id = "missing-skill";
        let err_msg = "skill not found";
        let failure_line = format!(
            "{} {skill_id}: {err_msg}",
            fmt.format_label("✗", "failed", LabelKind::Failure)
        );
        let hint_line = "Hint: Try 'list' to verify installed skills";
        assert!(
            failure_line.contains("\x1b["),
            "failure line should have ANSI: {failure_line}"
        );
        assert!(
            !hint_line.contains("\x1b["),
            "hint line must NOT have ANSI: {hint_line}"
        );
    }
}
