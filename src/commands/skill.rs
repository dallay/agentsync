use crate::output::{self, HumanFormatter, LabelKind, OutputMode};
use agentsync::skills::catalog::EmbeddedSkillCatalog;
use agentsync::skills::provider::{Provider, SkillsShProvider, resolve_catalog_install_source};
use agentsync::skills::registry;
use agentsync::skills::suggest::{
    SuggestInstallJsonResponse, SuggestInstallMode, SuggestInstallPhase,
    SuggestInstallProgressEvent, SuggestInstallProgressReporter, SuggestInstallStatus,
    SuggestResponse, SuggestionService,
};
use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use dialoguer::{MultiSelect, theme::ColorfulTheme};
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::path::{Component, Path};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestInstallOutputMode {
    Json,
    HumanLine { use_color: bool },
    HumanLive { use_color: bool },
}

fn detect_suggest_install_output_mode(
    json: bool,
    stdout_is_tty: bool,
    no_color: Option<&str>,
    clicolor: Option<&str>,
    term: Option<&str>,
) -> SuggestInstallOutputMode {
    let base = output::detect_output_mode(json, stdout_is_tty, no_color, clicolor, term);
    match base {
        OutputMode::Json => SuggestInstallOutputMode::Json,
        OutputMode::Human { use_color } => {
            let term_is_dumb = term.is_some_and(|v| v.eq_ignore_ascii_case("dumb"));
            if stdout_is_tty && !term_is_dumb {
                SuggestInstallOutputMode::HumanLive { use_color }
            } else {
                SuggestInstallOutputMode::HumanLine { use_color }
            }
        }
    }
}

fn suggest_install_output_mode(json: bool) -> SuggestInstallOutputMode {
    detect_suggest_install_output_mode(
        json,
        std::io::stdout().is_terminal(),
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("CLICOLOR").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

struct SuggestInstallLineReporter {
    formatter: HumanFormatter,
}

impl SuggestInstallLineReporter {
    fn new(use_color: bool) -> Self {
        Self {
            formatter: HumanFormatter::new(use_color),
        }
    }

    fn print_status(&self, symbol: &str, label: &str, kind: LabelKind, body: &str) {
        println!(
            "{} {body}",
            self.formatter.format_label(symbol, label, kind)
        );
    }
}

impl SuggestInstallProgressReporter for SuggestInstallLineReporter {
    fn on_event(&mut self, event: SuggestInstallProgressEvent) {
        match event {
            SuggestInstallProgressEvent::Resolving { skill_id } => {
                self.print_status("…", "resolving", LabelKind::Info, &skill_id);
            }
            SuggestInstallProgressEvent::Installing { skill_id } => {
                self.print_status("↓", "installing", LabelKind::Info, &skill_id);
            }
            SuggestInstallProgressEvent::SkippedAlreadyInstalled { skill_id } => {
                self.print_status("○", "already installed", LabelKind::Warning, &skill_id);
            }
            SuggestInstallProgressEvent::Installed { skill_id } => {
                self.print_status("✔", "installed", LabelKind::Success, &skill_id);
            }
            SuggestInstallProgressEvent::Failed {
                skill_id,
                phase,
                message,
            } => {
                let phase_label = match phase {
                    SuggestInstallPhase::Resolve => "resolve",
                    SuggestInstallPhase::Install => "install",
                };
                self.print_status(
                    "✗",
                    "failed",
                    LabelKind::Failure,
                    &format!("{skill_id} during {phase_label}: {message}"),
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SuggestInstallActivityKind {
    Resolving,
    Installing,
}

impl SuggestInstallActivityKind {
    fn label(self) -> &'static str {
        match self {
            Self::Resolving => "resolving",
            Self::Installing => "installing",
        }
    }
}

trait SuggestInstallLiveWriter: Send {
    fn write_str(&mut self, text: &str);
    fn flush(&mut self);
}

struct StdoutSuggestInstallLiveWriter;

impl SuggestInstallLiveWriter for StdoutSuggestInstallLiveWriter {
    fn write_str(&mut self, text: &str) {
        let _ = std::io::stdout().write_all(text.as_bytes());
    }

    fn flush(&mut self) {
        let _ = std::io::stdout().flush();
    }
}

#[derive(Debug)]
enum SuggestInstallLiveCommand {
    Activity {
        kind: SuggestInstallActivityKind,
        skill_id: String,
    },
    TerminalLine(String),
    Stop,
}

struct SuggestInstallLiveReporter {
    formatter: HumanFormatter,
    tx: Sender<SuggestInstallLiveCommand>,
    worker: Option<JoinHandle<()>>,
}

impl SuggestInstallLiveReporter {
    const DEFAULT_TICK_INTERVAL: Duration = Duration::from_millis(80);

    fn new(use_color: bool) -> Self {
        Self::with_writer(
            use_color,
            Box::new(StdoutSuggestInstallLiveWriter),
            Self::DEFAULT_TICK_INTERVAL,
        )
    }

    fn with_writer(
        use_color: bool,
        writer: Box<dyn SuggestInstallLiveWriter>,
        tick_interval: Duration,
    ) -> Self {
        let formatter = HumanFormatter::new(use_color);
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_suggest_install_live_worker(rx, writer, formatter, tick_interval);
        });

        Self {
            formatter,
            tx,
            worker: Some(worker),
        }
    }

    fn finalize(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.tx.send(SuggestInstallLiveCommand::Stop);
            let _ = worker.join();
        }
    }
}

impl Drop for SuggestInstallLiveReporter {
    fn drop(&mut self) {
        self.finalize();
    }
}

impl SuggestInstallProgressReporter for SuggestInstallLiveReporter {
    fn on_event(&mut self, event: SuggestInstallProgressEvent) {
        let command = match event {
            SuggestInstallProgressEvent::Resolving { skill_id } => {
                SuggestInstallLiveCommand::Activity {
                    kind: SuggestInstallActivityKind::Resolving,
                    skill_id,
                }
            }
            SuggestInstallProgressEvent::Installing { skill_id } => {
                SuggestInstallLiveCommand::Activity {
                    kind: SuggestInstallActivityKind::Installing,
                    skill_id,
                }
            }
            SuggestInstallProgressEvent::SkippedAlreadyInstalled { skill_id } => {
                SuggestInstallLiveCommand::TerminalLine(render_suggest_install_status_line(
                    self.formatter,
                    "○",
                    "already installed",
                    LabelKind::Warning,
                    &skill_id,
                ))
            }
            SuggestInstallProgressEvent::Installed { skill_id } => {
                SuggestInstallLiveCommand::TerminalLine(render_suggest_install_status_line(
                    self.formatter,
                    "✔",
                    "installed",
                    LabelKind::Success,
                    &skill_id,
                ))
            }
            SuggestInstallProgressEvent::Failed {
                skill_id,
                phase,
                message,
            } => {
                let phase_label = match phase {
                    SuggestInstallPhase::Resolve => "resolve",
                    SuggestInstallPhase::Install => "install",
                };
                SuggestInstallLiveCommand::TerminalLine(render_suggest_install_status_line(
                    self.formatter,
                    "✗",
                    "failed",
                    LabelKind::Failure,
                    &format!("{skill_id} during {phase_label}: {message}"),
                ))
            }
        };

        let _ = self.tx.send(command);
    }
}

fn render_suggest_install_status_line(
    formatter: HumanFormatter,
    symbol: &str,
    label: &str,
    kind: LabelKind,
    body: &str,
) -> String {
    format!("{} {body}", formatter.format_label(symbol, label, kind))
}

fn render_skill_action_success(action: &str, skill_id: &str, use_color: bool) -> String {
    let formatter = HumanFormatter::new(use_color);
    render_suggest_install_status_line(formatter, "✔", action, LabelKind::Success, skill_id)
}

fn render_skill_hint(remediation: &str) -> String {
    format!("  Hint: {remediation}")
}

fn render_skill_command_error(
    error_message: &str,
    remediation: &str,
    use_color: bool,
) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    vec![
        render_suggest_install_status_line(
            formatter,
            "✗",
            "error",
            LabelKind::Failure,
            error_message,
        ),
        render_skill_hint(remediation),
    ]
}

fn render_skill_action_failure(
    skill_id: &str,
    error_message: &str,
    remediation: &str,
    use_color: bool,
) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    vec![
        render_suggest_install_status_line(
            formatter,
            "✗",
            "failed",
            LabelKind::Failure,
            &format!("{skill_id}: {error_message}"),
        ),
        render_skill_hint(remediation),
    ]
}

fn render_suggest_install_activity_line(
    formatter: HumanFormatter,
    frame: &str,
    kind: SuggestInstallActivityKind,
    skill_id: &str,
) -> String {
    render_suggest_install_status_line(formatter, frame, kind.label(), LabelKind::Info, skill_id)
}

fn render_live_line(writer: &mut dyn SuggestInstallLiveWriter, line: &str, last_width: &mut usize) {
    let width = line.chars().count();
    let padding = " ".repeat(last_width.saturating_sub(width));
    writer.write_str("\r");
    writer.write_str(line);
    if !padding.is_empty() {
        writer.write_str(&padding);
    }
    writer.flush();
    *last_width = width;
}

fn clear_live_line(writer: &mut dyn SuggestInstallLiveWriter, last_width: &mut usize) {
    if *last_width == 0 {
        return;
    }

    writer.write_str("\r");
    writer.write_str(&" ".repeat(*last_width));
    writer.write_str("\r");
    writer.flush();
    *last_width = 0;
}

fn run_suggest_install_live_worker(
    rx: mpsc::Receiver<SuggestInstallLiveCommand>,
    mut writer: Box<dyn SuggestInstallLiveWriter>,
    formatter: HumanFormatter,
    tick_interval: Duration,
) {
    const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    let mut active: Option<(SuggestInstallActivityKind, String)> = None;
    let mut frame_index = 0usize;
    let mut last_width = 0usize;

    loop {
        match rx.recv_timeout(tick_interval) {
            Ok(SuggestInstallLiveCommand::Activity { kind, skill_id }) => {
                active = Some((kind, skill_id));
                frame_index = 0;
                if let Some((active_kind, active_skill_id)) = active.as_ref() {
                    let line = render_suggest_install_activity_line(
                        formatter,
                        SPINNER_FRAMES[frame_index],
                        *active_kind,
                        active_skill_id,
                    );
                    render_live_line(&mut *writer, &line, &mut last_width);
                    frame_index = (frame_index + 1) % SPINNER_FRAMES.len();
                }
            }
            Ok(SuggestInstallLiveCommand::TerminalLine(line)) => {
                clear_live_line(&mut *writer, &mut last_width);
                writer.write_str(&line);
                writer.write_str("\n");
                writer.flush();
                active = None;
            }
            Ok(SuggestInstallLiveCommand::Stop) => {
                if active.is_some() {
                    clear_live_line(&mut *writer, &mut last_width);
                    writer.write_str("\n");
                    writer.flush();
                }
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some((kind, skill_id)) = active.as_ref() {
                    let line = render_suggest_install_activity_line(
                        formatter,
                        SPINNER_FRAMES[frame_index],
                        *kind,
                        skill_id,
                    );
                    render_live_line(&mut *writer, &line, &mut last_width);
                    frame_index = (frame_index + 1) % SPINNER_FRAMES.len();
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn print_suggest_install_batch_start(
    mode: SuggestInstallMode,
    selected_count: usize,
    use_color: bool,
) {
    for line in render_suggest_install_batch_start(mode, selected_count, use_color) {
        println!("{line}");
    }
}

fn render_suggest_install_batch_start(
    mode: SuggestInstallMode,
    selected_count: usize,
    use_color: bool,
) -> Vec<String> {
    let formatter = HumanFormatter::new(use_color);
    let noun = if selected_count == 1 {
        "skill"
    } else {
        "skills"
    };
    let detail = match mode {
        SuggestInstallMode::Interactive => {
            format!("Installing {selected_count} selected recommended {noun}...")
        }
        SuggestInstallMode::InstallAll => {
            format!("Installing {selected_count} recommended {noun}...")
        }
    };
    vec![
        formatter.format_heading("➤ Install recommendations"),
        format!("  {detail}"),
    ]
}

fn render_suggest_install_completion_summary(
    response: &SuggestInstallJsonResponse,
    use_color: bool,
) -> String {
    let formatter = HumanFormatter::new(use_color);
    let installed = response
        .results
        .iter()
        .filter(|result| result.status == SuggestInstallStatus::Installed)
        .count();
    let skipped = response
        .results
        .iter()
        .filter(|result| result.status == SuggestInstallStatus::AlreadyInstalled)
        .count();
    let failed = response
        .results
        .iter()
        .filter(|result| result.status == SuggestInstallStatus::Failed)
        .count();

    let mut lines = vec![formatter.format_heading("Recommendation install summary")];
    lines.push(format!(
        "  {}: {installed}",
        formatter.format_label("✔", "Installed", LabelKind::Success)
    ));
    lines.push(format!(
        "  {}: {skipped}",
        formatter.format_label("○", "Already installed", LabelKind::Warning)
    ));
    lines.push(format!(
        "  {}: {failed}",
        formatter.format_label("✗", "Failed", LabelKind::Failure)
    ));

    if response.mode == SuggestInstallMode::Interactive && response.selected_skill_ids.is_empty() {
        lines.push("  Note: nothing selected.".to_string());
    } else if installed == 0 && failed == 0 {
        lines.push("  Note: nothing installable to do.".to_string());
    }

    let failures = response
        .results
        .iter()
        .filter(|result| result.status == SuggestInstallStatus::Failed)
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        lines.push("  Failure details:".to_string());
        for failure in failures {
            let message = failure.error_message.as_deref().unwrap_or("unknown error");
            lines.push(format!("    - {}: {}", failure.skill_id, message));
        }
    }

    lines.join("\n")
}

fn render_skill_suggest_human(response: &SuggestResponse, use_color: bool) -> String {
    let formatter = HumanFormatter::new(use_color);
    let mut lines = Vec::<String>::new();

    lines.push(formatter.format_heading("➤ Detected technologies"));
    if response.detections.is_empty() {
        lines.push("  none".to_string());
    } else {
        for detection in &response.detections {
            let evidence = detection
                .evidence
                .iter()
                .map(|evidence| evidence.path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "  {} ({}): {}",
                detection.technology,
                detection.confidence.as_human_label(),
                evidence
            ));
        }
    }

    lines.push(String::new());
    lines.push(formatter.format_heading("➤ Recommended skills"));
    if response.recommendations.is_empty() {
        lines.push("  none".to_string());
    } else {
        for recommendation in &response.recommendations {
            let installed = if recommendation.installed {
                match recommendation.installed_version.as_deref() {
                    Some(version) => format!("installed ({version})"),
                    None => "installed".to_string(),
                }
            } else {
                "not installed".to_string()
            };

            lines.push(format!(
                "  {} — {} [{}]",
                recommendation.skill_id, recommendation.title, installed
            ));
            lines.push(format!("    {}", recommendation.summary));
            for reason in &recommendation.reasons {
                lines.push(format!("    reason: {reason}"));
            }
        }
    }

    lines.push(String::new());
    lines.push(formatter.format_heading("➤ Summary"));
    lines.push(format!("  Detected: {}", response.summary.detected_count));
    lines.push(format!(
        "  Recommended: {}",
        response.summary.recommended_count
    ));
    lines.push(format!(
        "  Installable: {}",
        response.summary.installable_count
    ));

    lines.join("\n")
}

fn render_skill_success_json(
    skill_id: &str,
    skill: Option<&registry::SkillEntry>,
    status: &str,
) -> serde_json::Value {
    serde_json::json!({
        "id": skill_id,
        "name": skill.and_then(|skill| skill.name.clone()),
        "description": skill.and_then(|skill| skill.description.clone()),
        "version": skill.and_then(|skill| skill.version.clone()),
        "files": skill.and_then(|skill| skill.files.clone()),
        "manifest_hash": skill.and_then(|skill| skill.manifest_hash.clone()),
        "installed_at": skill.and_then(|skill| skill.installed_at.clone()),
        "status": status
    })
}

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Install a skill from skills.sh or a custom provider
    Install(SkillInstallArgs),
    /// Update a skill to latest version
    Update(SkillUpdateArgs),
    /// Uninstall a skill
    Uninstall(SkillUninstallArgs),
    /// Suggest repo-aware skills without installing them
    Suggest(SkillSuggestArgs),
    /// List installed skills
    List,
}

/// Arguments for installing a skill
#[derive(Args, Debug)]
pub struct SkillInstallArgs {
    /// Skill id to install
    pub skill_id: String,
    /// Optional source (dir, archive, or URL)
    #[arg(long)]
    pub source: Option<String>,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for updating a skill
#[derive(Args, Debug)]
pub struct SkillUpdateArgs {
    /// Skill id to update
    pub skill_id: String,
    /// Optional source (dir, archive, or URL)
    #[arg(long)]
    pub source: Option<String>,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for uninstalling a skill
#[derive(Args, Debug)]
pub struct SkillUninstallArgs {
    /// Skill id to uninstall
    pub skill_id: String,
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
}

/// Arguments for suggesting skills
#[derive(Args, Debug)]
pub struct SkillSuggestArgs {
    /// Output JSON instead of human-friendly
    #[arg(long)]
    pub json: bool,
    /// Install recommended skills using a guided flow
    #[arg(long)]
    pub install: bool,
    /// Select all recommended skills instead of using the guided prompt
    #[arg(long, requires = "install")]
    pub all: bool,
}

pub fn run_update(args: SkillUpdateArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");
    std::fs::create_dir_all(&target_root)?;

    let skill_id = &args.skill_id;
    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let source = resolve_source(skill_id, args.source.clone())?;
    let update_source_path = std::path::Path::new(&source);
    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.block_on(agentsync::skills::update::update_skill_async(
            skill_id,
            &target_root,
            update_source_path,
        ))
    } else {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(agentsync::skills::update::update_skill_async(
            skill_id,
            &target_root,
            update_source_path,
        ))
    };
    let mode = output::output_mode(args.json);
    match result {
        Ok(()) => {
            match mode {
                OutputMode::Json => {
                    let registry_path = project_root
                        .join(".agents")
                        .join("skills")
                        .join("registry.json");
                    let reg_res = registry::read_registry(&registry_path);
                    let entry = reg_res
                        .as_ref()
                        .ok()
                        .and_then(|reg| reg.skills.as_ref())
                        .and_then(|s| s.get(skill_id));
                    if let Err(ref e) = reg_res {
                        tracing::warn!(
                            ?e,
                            "Failed to read registry after update, falling back to schema-stable response"
                        );
                    }
                    let output = render_skill_success_json(skill_id, entry, "updated");
                    println!("{}", serde_json::to_string(&output)?);
                }
                OutputMode::Human { use_color } => {
                    println!(
                        "{}",
                        render_skill_action_success("updated", skill_id, use_color)
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            let err_string = e.to_string();
            let code = "update_error";
            let remediation = remediation_for_error(&err_string);

            match mode {
                OutputMode::Json => {
                    let output = serde_json::json!({
                        "error": err_string,
                        "code": code,
                        "remediation": remediation
                    });
                    println!("{}", serde_json::to_string(&output)?);
                    Err(e.into())
                }
                OutputMode::Human { use_color } => {
                    error!(%code, %err_string, "Update failed");
                    for line in
                        render_skill_action_failure(skill_id, &err_string, remediation, use_color)
                    {
                        println!("{line}");
                    }
                    Err(e.into())
                }
            }
        }
    }
}

pub fn run_skill(cmd: SkillCommand, project_root: PathBuf) -> Result<()> {
    match cmd {
        SkillCommand::Install(args) => run_install(args, project_root),
        SkillCommand::Update(args) => run_update(args, project_root),
        SkillCommand::Uninstall(args) => run_uninstall(args, project_root),
        SkillCommand::Suggest(args) => run_suggest(args, project_root),
        SkillCommand::List => {
            // Signal failure until List is implemented so CLI exits non-zero
            bail!("list command not implemented")
        }
    }
}

pub fn run_suggest(args: SkillSuggestArgs, project_root: PathBuf) -> Result<()> {
    let service = SuggestionService;
    let result = (|| -> Result<()> {
        let response = service.suggest(&project_root)?;
        let output_mode = suggest_install_output_mode(args.json);

        if !args.install {
            if args.json {
                println!("{}", serde_json::to_string(&response.to_json_response())?);
            } else {
                let use_color = match output::output_mode(false) {
                    OutputMode::Human { use_color } => use_color,
                    OutputMode::Json => false,
                };
                println!("{}", render_skill_suggest_human(&response, use_color));
            }
            return Ok(());
        }

        let provider = SuggestInstallProvider::default();
        let install_response = match output_mode {
            SuggestInstallOutputMode::Json => {
                if args.all {
                    service.install_all_with(&project_root, &response, &provider)
                } else {
                    ensure_interactive_install_supported()?;
                    let selected_skill_ids = prompt_for_recommended_skills(&response)?;
                    service.install_selected_with(
                        &project_root,
                        &response,
                        &provider,
                        SuggestInstallMode::Interactive,
                        &selected_skill_ids,
                        |skill_id, source, target_root| {
                            agentsync::skills::install::blocking_fetch_and_install_skill(
                                skill_id,
                                source,
                                target_root,
                            )
                            .map_err(|error| anyhow::anyhow!(error))
                        },
                    )
                }
            }
            SuggestInstallOutputMode::HumanLine { use_color }
            | SuggestInstallOutputMode::HumanLive { use_color } => {
                let (mode, selected_skill_ids) = if args.all {
                    (
                        SuggestInstallMode::InstallAll,
                        response
                            .recommendations
                            .iter()
                            .map(|recommendation| recommendation.skill_id.clone())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    ensure_interactive_install_supported()?;
                    (
                        SuggestInstallMode::Interactive,
                        prompt_for_recommended_skills(&response)?,
                    )
                };

                print_suggest_install_batch_start(mode, selected_skill_ids.len(), use_color);
                match output_mode {
                    SuggestInstallOutputMode::HumanLine { .. } => {
                        let mut reporter = SuggestInstallLineReporter::new(use_color);
                        service.install_selected_with_reporter(
                            &project_root,
                            &response,
                            &provider,
                            mode,
                            &selected_skill_ids,
                            &mut reporter,
                            |skill_id, source, target_root| {
                                agentsync::skills::install::blocking_fetch_and_install_skill(
                                    skill_id,
                                    source,
                                    target_root,
                                )
                                .map_err(|error| anyhow::anyhow!(error))
                            },
                        )
                    }
                    SuggestInstallOutputMode::HumanLive { .. } => {
                        let mut reporter = SuggestInstallLiveReporter::new(use_color);
                        let result = service.install_selected_with_reporter(
                            &project_root,
                            &response,
                            &provider,
                            mode,
                            &selected_skill_ids,
                            &mut reporter,
                            |skill_id, source, target_root| {
                                agentsync::skills::install::blocking_fetch_and_install_skill(
                                    skill_id,
                                    source,
                                    target_root,
                                )
                                .map_err(|error| anyhow::anyhow!(error))
                            },
                        );
                        reporter.finalize();
                        result
                    }
                    SuggestInstallOutputMode::Json => unreachable!(),
                }
            }
        }?;

        match output_mode {
            SuggestInstallOutputMode::Json => {
                // In JSON mode, include failure information in the output but don't fail the command
                // since the response contains detailed results that consumers can inspect
                println!("{}", serde_json::to_string(&install_response)?);
            }
            SuggestInstallOutputMode::HumanLine { use_color }
            | SuggestInstallOutputMode::HumanLive { use_color } => {
                println!(
                    "{}",
                    render_suggest_install_completion_summary(&install_response, use_color)
                );
            }
        }

        Ok(())
    })();

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            let error_message = error.to_string();
            let (code, remediation) = if error_message
                .contains("not part of the current recommendation set")
            {
                (
                    "invalid_suggestion_selection",
                    "Run 'agentsync skill suggest --json' to inspect available recommended skill ids.",
                )
            } else if error_message.contains("interactive terminal") {
                (
                    "interactive_tty_required",
                    "Run 'agentsync skill suggest --install --all' for a non-interactive install path.",
                )
            } else if args.install {
                ("install_error", remediation_for_error(&error_message))
            } else {
                (
                    "suggest_error",
                    "Verify the project root is readable and try again. Use --project-root to point to the repository you want to inspect.",
                )
            };

            if args.json {
                let output = serde_json::json!({
                    "error": error_message,
                    "code": code,
                    "remediation": remediation,
                });
                println!("{}", serde_json::to_string(&output)?);
            } else {
                error!(%code, error = %error_message, "Suggest failed");
                let use_color = match output::output_mode(false) {
                    OutputMode::Human { use_color } => use_color,
                    OutputMode::Json => false,
                };
                for line in render_skill_command_error(&error_message, remediation, use_color) {
                    println!("{line}");
                }
            }

            Err(error)
        }
    }
}

fn ensure_interactive_install_supported() -> Result<()> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "guided recommendation install requires an interactive terminal"
    ))
}

fn prompt_for_recommended_skills(
    response: &agentsync::skills::suggest::SuggestResponse,
) -> Result<Vec<String>> {
    let installable = response.installable_recommendations();
    if installable.is_empty() {
        return Ok(Vec::new());
    }

    let items = installable
        .iter()
        .map(|recommendation| format!("{} — {}", recommendation.skill_id, recommendation.summary))
        .collect::<Vec<_>>();
    let defaults = vec![true; items.len()];
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select recommended skills to install")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    Ok(selections
        .into_iter()
        .map(|index| installable[index].skill_id.clone())
        .collect())
}

pub fn run_install(args: SkillInstallArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");
    std::fs::create_dir_all(&target_root)?;

    // For this example we'll accept local ZIP/path as "skill_id" (for test and demo), real impl will use resolver
    // Accept: skill.zip / skill.tar.gz (local file path or URL); fallback: skill_id as path
    let skill_id = &args.skill_id;
    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let source = resolve_source(skill_id, args.source.clone())?;

    // Unified logic: install from archive, URL, or local directory
    tracing::debug!(
        skill_id = %skill_id,
        source = %source,
        target_root = %target_root.display(),
        "install"
    );
    let result = agentsync::skills::install::blocking_fetch_and_install_skill(
        skill_id,
        &source,
        &target_root,
    );
    let mode = output::output_mode(args.json);
    match result {
        Ok(()) => {
            match mode {
                OutputMode::Json => {
                    let registry_path = project_root
                        .join(".agents")
                        .join("skills")
                        .join("registry.json");
                    let reg_res = registry::read_registry(&registry_path);
                    let entry = reg_res
                        .as_ref()
                        .ok()
                        .and_then(|reg| reg.skills.as_ref())
                        .and_then(|s| s.get(&args.skill_id));
                    if let Err(ref e) = reg_res {
                        tracing::warn!(
                            ?e,
                            "Failed to read registry after install, falling back to schema-stable response"
                        );
                    }
                    let output = render_skill_success_json(&args.skill_id, entry, "installed");
                    println!("{}", serde_json::to_string(&output)?);
                }
                OutputMode::Human { use_color } => {
                    println!(
                        "{}",
                        render_skill_action_success("installed", skill_id, use_color)
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            let e: anyhow::Error = e.into();
            // Try to downcast to SkillInstallError to extract code/remediation
            let (err_string, code, remediation);
            err_string = e.to_string();
            code = "install_error";
            remediation = remediation_for_error(&err_string);

            match mode {
                OutputMode::Json => {
                    let output = serde_json::json!({
                        "error": err_string,
                        "code": code,
                        "remediation": remediation
                    });
                    println!("{}", serde_json::to_string(&output)?);
                    Err(e)
                }
                OutputMode::Human { use_color } => {
                    error!(%code, %err_string, "Install failed");
                    for line in
                        render_skill_action_failure(skill_id, &err_string, remediation, use_color)
                    {
                        println!("{line}");
                    }
                    Err(e)
                }
            }
        }
    }
}

struct SuggestInstallProvider {
    fallback: SkillsShProvider,
}

impl Default for SuggestInstallProvider {
    fn default() -> Self {
        Self {
            fallback: SkillsShProvider,
        }
    }
}

impl Provider for SuggestInstallProvider {
    fn manifest(&self) -> Result<String> {
        self.fallback.manifest()
    }

    fn resolve(&self, id: &str) -> Result<agentsync::skills::provider::SkillInstallInfo> {
        let catalog = EmbeddedSkillCatalog::default();
        if let Some(definition) = catalog.get_skill_definition(id) {
            let download_url = resolve_catalog_install_source(
                &catalog,
                &self.fallback,
                &definition.provider_skill_id,
                &definition.local_skill_id,
                None,
            )?;

            return Ok(agentsync::skills::provider::SkillInstallInfo {
                download_url: download_url.clone(),
                // Informational only today: install pipeline infers behavior from the source string.
                format: infer_install_source_format(&download_url),
            });
        }

        if let Ok(source_root) = std::env::var("AGENTSYNC_TEST_SKILL_SOURCE_DIR") {
            // or a simple local name. Use the full ID to find the local source directory.
            let source_path = PathBuf::from(source_root).join(id);
            if source_path.exists() {
                return Ok(agentsync::skills::provider::SkillInstallInfo {
                    download_url: source_path.display().to_string(),
                    format: "dir".to_string(),
                });
            }
        }

        self.fallback.resolve(id)
    }

    fn recommendation_catalog(
        &self,
    ) -> Result<Option<agentsync::skills::provider::ProviderCatalogMetadata>> {
        self.fallback.recommendation_catalog()
    }
}

pub fn run_uninstall(args: SkillUninstallArgs, project_root: PathBuf) -> Result<()> {
    let target_root = project_root.join(".agents").join("skills");

    let skill_id = &args.skill_id;

    // Validate skill_id to prevent path traversal or invalid path segments
    validate_skill_id(skill_id)?;

    let result = agentsync::skills::uninstall::uninstall_skill(skill_id, &target_root);

    let mode = output::output_mode(args.json);
    match result {
        Ok(()) => {
            match mode {
                OutputMode::Json => {
                    let output = serde_json::json!({
                        "id": skill_id,
                        "status": "uninstalled"
                    });
                    println!("{}", serde_json::to_string(&output)?);
                }
                OutputMode::Human { use_color } => {
                    println!(
                        "{}",
                        render_skill_action_success("uninstalled", skill_id, use_color)
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            let err_string = e.to_string();
            let (code, remediation) = match &e {
                agentsync::skills::uninstall::SkillUninstallError::NotFound(_) => (
                    "skill_not_found",
                    "Skill is not installed. Check .agents/skills/ directory for installed skills.",
                ),
                agentsync::skills::uninstall::SkillUninstallError::Validation(_) => (
                    "validation_error",
                    "Ensure the skill ID is valid (no special characters, not '.' or '..')",
                ),
                _ => (
                    "uninstall_error",
                    "Ensure you have proper permissions to remove files",
                ),
            };

            match mode {
                OutputMode::Json => {
                    let output = serde_json::json!({
                        "error": err_string,
                        "code": code,
                        "remediation": remediation
                    });
                    println!("{}", serde_json::to_string(&output)?);
                    Err(anyhow::anyhow!(e))
                }
                OutputMode::Human { use_color } => {
                    error!(%code, %err_string, "Uninstall failed");
                    for line in
                        render_skill_action_failure(skill_id, &err_string, remediation, use_color)
                    {
                        println!("{line}");
                    }
                    Err(anyhow::anyhow!(e))
                }
            }
        }
    }
}

fn resolve_source(skill_id: &str, source_arg: Option<String>) -> Result<String> {
    if let Some(s) = source_arg {
        // Check if it's a GitHub URL that needs conversion to ZIP format
        if let Some(github_url) = try_convert_github_url(&s) {
            tracing::info!(original = %s, converted = %github_url, "Converted GitHub URL to ZIP format");
            return Ok(github_url);
        }
        return Ok(s);
    }

    // If it doesn't look like a URL or a path, try to resolve via skills.sh
    if !skill_id.contains("://") && !skill_id.starts_with('/') && !skill_id.starts_with('.') {
        let catalog = EmbeddedSkillCatalog::default();
        if let Some(definition) = catalog.get_skill_definition_by_local_id(skill_id) {
            let provider = SkillsShProvider;
            return resolve_catalog_install_source(
                &catalog,
                &provider,
                &definition.provider_skill_id,
                &definition.local_skill_id,
                None,
            )
            .map_err(|e| {
                tracing::warn!(skill_id = %skill_id, provider_skill_id = %definition.provider_skill_id, ?e, "Failed to resolve catalog skill via skills provider");
                anyhow::anyhow!(
                    "failed to resolve skill '{}' via provider '{}': {}",
                    skill_id,
                    definition.provider_skill_id,
                    e
                )
            });
        }

        let provider = SkillsShProvider;
        match provider.resolve(skill_id) {
            Ok(info) => Ok(info.download_url),
            Err(e) => {
                tracing::warn!(%skill_id, ?e, "Failed to resolve skill via skills.sh");
                Err(anyhow::anyhow!(
                    "failed to resolve skill '{}' via skills.sh: {}",
                    skill_id,
                    e
                ))
            }
        }
    } else {
        Ok(skill_id.to_string())
    }
}

fn infer_install_source_format(source: &str) -> String {
    if source.starts_with("http://") || source.starts_with("https://") {
        if source.ends_with(".tar.gz") || source.ends_with(".tgz") {
            return "tar.gz".to_string();
        }

        if source.ends_with(".zip") {
            return "zip".to_string();
        }

        return "url".to_string();
    }

    "dir".to_string()
}

/// Attempts to convert a GitHub URL to a downloadable ZIP URL.
///
/// Supports the following GitHub URL formats:
/// - `https://github.com/owner/repo` → `https://github.com/owner/repo/archive/HEAD.zip`
/// - `https://github.com/owner/repo/tree/branch/path` → `https://github.com/owner/repo/archive/refs/heads/branch.zip#path`
/// - `https://github.com/owner/repo/blob/branch/path/file` → `https://github.com/owner/repo/archive/refs/heads/branch.zip#path`
///
/// **Limitation:** Branch names containing slashes (e.g., `feature/auth`) cannot be reliably
/// distinguished from subpaths without accessing the GitHub API. In such cases, the function
/// assumes the first segment after `tree/` or `blob/` is the branch name. For branches with
/// slashes, the resulting URL may be incorrect. If API access becomes available in the future,
/// this function could use the GitHub refs API to resolve the correct branch name via
/// longest-prefix matching.
///
/// Returns `None` if the URL is not a GitHub URL or already points to an archive.
fn try_convert_github_url(url: &str) -> Option<String> {
    // Parse the URL to properly handle query strings and fragments
    let parsed = url::Url::parse(url).ok()?;

    // Check if it's already an archive URL by examining the path component
    let path = parsed.path();
    if path.ends_with(".zip") || path.ends_with(".tar.gz") || path.ends_with(".tgz") {
        return None;
    }

    // Only process github.com URLs
    if parsed.host_str() != Some("github.com") {
        return None;
    }

    // Get the path segments
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.collect())
        .unwrap_or_default();

    // Minimum: owner/repo (at least 2 segments)
    if segments.len() < 2 {
        return None;
    }

    let owner = segments[0];
    let repo = segments[1];

    // Check if it's a tree or blob URL with subpath
    if segments.len() >= 4 && (segments[2] == "tree" || segments[2] == "blob") {
        let branch = segments[3];
        // The rest is the path within the repo
        let subpath = segments[4..].join("/");

        // If it's a blob URL pointing to a file, get the parent directory
        let final_subpath = if segments[2] == "blob" {
            if subpath.contains('/') {
                // Remove the filename to get the directory
                let path_parts: Vec<&str> = subpath.split('/').collect();
                if path_parts.len() > 1 {
                    path_parts[..path_parts.len() - 1].join("/")
                } else {
                    subpath
                }
            } else {
                // Blob pointing to a file at repo root (e.g., README.md)
                // Return empty string so no fragment is added
                String::new()
            }
        } else {
            subpath
        };

        let mut zip_url = format!(
            "https://github.com/{}/{}/archive/refs/heads/{}.zip",
            owner, repo, branch
        );

        if !final_subpath.is_empty() {
            zip_url.push('#');
            zip_url.push_str(&final_subpath);
        }

        return Some(zip_url);
    }

    // Simple repo URL: github.com/owner/repo
    if segments.len() == 2 {
        return Some(format!(
            "https://github.com/{}/{}/archive/HEAD.zip",
            owner, repo
        ));
    }

    // Repo URL with trailing segments but not tree/blob
    // e.g., github.com/owner/repo/ (with trailing slash)
    if segments.len() > 2 && segments[2].is_empty() {
        return Some(format!(
            "https://github.com/{}/{}/archive/HEAD.zip",
            owner, repo
        ));
    }

    None
}

fn remediation_for_error(msg: &str) -> &str {
    if msg.contains("manifest") {
        "Check the SKILL.md syntax, frontmatter, and ensure the 'name' field matches requirements. See agentsync docs/spec for manifest schema."
    } else if msg.contains("network") || msg.contains("download") || msg.contains("HTTP") {
        "Check your network connection and ensure the skill source URL is correct."
    } else if msg.contains("archive") {
        "Verify the skill archive is a valid zip or tar.gz. Try re-downloading or using a fresh archive."
    } else if msg.contains("permission") {
        "Check your file permissions or try running as administrator/root."
    } else if msg.contains("registry") {
        "There was a problem updating the registry. Ensure you have write access and the registry file isn't corrupted."
    } else {
        "See above error message. If unsure, run with increased verbosity or check the documentation."
    }
}

fn validate_skill_id(skill_id: &str) -> Result<()> {
    if skill_id.is_empty() {
        return Err(anyhow::anyhow!("skill id must not be empty"));
    }

    // Quick reject any obvious separators
    if skill_id.contains('/') || skill_id.contains('\\') {
        return Err(anyhow::anyhow!(
            "invalid skill id: must be a single path segment without '/' or '\\'"
        ));
    }

    let p = Path::new(skill_id);

    // Reject absolute paths or paths that start with a prefix/root (drive letter on Windows)
    if p.is_absolute() {
        return Err(anyhow::anyhow!(
            "invalid skill id: must not be an absolute path"
        ));
    }

    // Ensure components() yields exactly one Component::Normal
    let mut count_normal = 0usize;
    for comp in p.components() {
        match comp {
            Component::Normal(_) => count_normal += 1,
            // Any other component is invalid (RootDir, Prefix, CurDir, ParentDir)
            other => {
                return Err(anyhow::anyhow!(
                    "invalid skill id: contains invalid path component: {:?}",
                    other
                ));
            }
        }
    }

    if count_normal != 1 {
        return Err(anyhow::anyhow!(
            "invalid skill id: must be a single path segment"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentsync::skills::catalog::{CatalogSkillMetadata, EmbeddedSkillCatalog};
    use agentsync::skills::suggest::{
        DetectionConfidence, DetectionEvidence, SkillSuggestion, SuggestResponse, SuggestSummary,
        TechnologyDetection, TechnologyId,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn validate_skill_id_accepts_simple_names() {
        assert!(validate_skill_id("weather-skill").is_ok());
        assert!(validate_skill_id("hello").is_ok());
        assert!(validate_skill_id("a").is_ok());
        assert!(validate_skill_id("skill_123").is_ok());
    }

    #[test]
    fn validate_skill_id_rejects_invalid_inputs() {
        // empty
        assert!(validate_skill_id("").is_err());

        // separators
        assert!(validate_skill_id("foo/bar").is_err());
        assert!(validate_skill_id("foo\\bar").is_err());

        // relative/cur/parent
        assert!(validate_skill_id(".").is_err());
        assert!(validate_skill_id("..").is_err());

        // absolute path (unix)
        assert!(validate_skill_id("/abs/path").is_err());

        // absolute path (windows)
        assert!(validate_skill_id("C:\\path").is_err());
        assert!(validate_skill_id("C:/path").is_err());
    }

    #[test]
    fn run_skill_list_returns_error() {
        let project_root = std::env::temp_dir();
        let res = run_skill(SkillCommand::List, project_root);
        assert!(res.is_err(), "list should return error until implemented");
    }

    #[test]
    fn github_url_converter_simple_repo() {
        let result = try_convert_github_url("https://github.com/obra/superpowers");
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/HEAD.zip".to_string())
        );
    }

    #[test]
    fn github_url_converter_tree_with_subpath() {
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/tree/main/skills/systematic-debugging",
        );
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/refs/heads/main.zip#skills/systematic-debugging".to_string())
        );
    }

    #[test]
    fn github_url_converter_blob_extracts_directory() {
        // Blob URLs should extract the parent directory
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/blob/main/skills/systematic-debugging/SKILL.md",
        );
        assert_eq!(
            result,
            Some("https://github.com/obra/superpowers/archive/refs/heads/main.zip#skills/systematic-debugging".to_string())
        );
    }

    #[test]
    fn github_url_converter_already_zip_returns_none() {
        // Already a ZIP URL should return None
        let result = try_convert_github_url(
            "https://github.com/obra/superpowers/archive/refs/heads/main.zip",
        );
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_tar_gz_returns_none() {
        // Already a tar.gz URL should return None
        let result = try_convert_github_url("https://example.com/archive.tar.gz");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_non_github_returns_none() {
        // Non-GitHub URLs should return None
        let result = try_convert_github_url("https://gitlab.com/user/repo");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_tree_deeply_nested() {
        // Deeply nested paths
        let result = try_convert_github_url(
            "https://github.com/owner/repo/tree/develop/skills/category/my-skill",
        );
        assert_eq!(
            result,
            Some("https://github.com/owner/repo/archive/refs/heads/develop.zip#skills/category/my-skill".to_string())
        );
    }

    #[test]
    fn github_url_converter_blob_root_file() {
        // Blob URL pointing to a file at repository root should return ZIP URL without fragment
        let result = try_convert_github_url("https://github.com/owner/repo/blob/main/README.md");
        assert_eq!(
            result,
            Some("https://github.com/owner/repo/archive/refs/heads/main.zip".to_string())
        );
    }

    #[test]
    fn github_url_converter_zip_with_query_string() {
        // ZIP URL with query string should return None (already an archive)
        let result =
            try_convert_github_url("https://github.com/owner/repo/archive/HEAD.zip?token=abc123");
        assert_eq!(result, None);
    }

    #[test]
    fn github_url_converter_zip_with_fragment() {
        // ZIP URL with fragment should return None (already an archive)
        let result =
            try_convert_github_url("https://github.com/owner/repo/archive/HEAD.zip#subpath");
        assert_eq!(result, None);
    }

    #[test]
    fn skill_action_success_uses_shared_label_shape() {
        assert_eq!(
            render_skill_action_success("installed", "rust-async-patterns", false),
            "✔ installed rust-async-patterns"
        );
    }

    #[test]
    fn skill_action_failure_indents_hint() {
        assert_eq!(
            render_skill_action_failure(
                "rust-async-patterns",
                "simulated failure",
                "Check permissions and try again.",
                false,
            ),
            vec![
                "✗ failed rust-async-patterns: simulated failure".to_string(),
                "  Hint: Check permissions and try again.".to_string(),
            ]
        );
    }

    #[test]
    fn skill_suggest_human_uses_headings_and_summary_counts() {
        let response = SuggestResponse {
            detections: vec![TechnologyDetection {
                technology: TechnologyId::new("rust"),
                confidence: DetectionConfidence::High,
                root_relative_paths: vec![PathBuf::from("Cargo.toml")],
                evidence: vec![DetectionEvidence {
                    marker: "Cargo.toml".to_string(),
                    path: PathBuf::from("Cargo.toml"),
                    notes: None,
                }],
            }],
            recommendations: vec![{
                let catalog = EmbeddedSkillCatalog::default();
                let metadata = CatalogSkillMetadata {
                    skill_id: "rust-async-patterns".to_string(),
                    provider_skill_id: "dallay/rust-async-patterns".to_string(),
                    title: "Rust async programming patterns".to_string(),
                    summary: "Master Rust async programming with Tokio.".to_string(),
                };
                let mut suggestion = SkillSuggestion::new(&metadata, &catalog);
                suggestion.reasons = vec!["detected rust".to_string()];
                suggestion.matched_technologies = vec![TechnologyId::new("rust")];
                suggestion
            }],
            summary: SuggestSummary {
                detected_count: 1,
                recommended_count: 1,
                installable_count: 1,
            },
        };

        let output = render_skill_suggest_human(&response, false);
        assert!(output.contains("➤ Detected technologies"), "{output}");
        assert!(output.contains("  rust (high): Cargo.toml"), "{output}");
        assert!(output.contains("➤ Recommended skills"), "{output}");
        assert!(
            output.contains(
                "  rust-async-patterns — Rust async programming patterns [not installed]"
            ),
            "{output}"
        );
        assert!(
            output.contains("    Master Rust async programming with Tokio."),
            "{output}"
        );
        assert!(output.contains("    reason: detected rust"), "{output}");
        assert!(output.contains("➤ Summary"), "{output}");
        assert!(output.contains("  Detected: 1"), "{output}");
        assert!(output.contains("  Recommended: 1"), "{output}");
        assert!(output.contains("  Installable: 1"), "{output}");
    }

    #[test]
    fn skill_suggest_human_reports_none_when_empty() {
        let response = SuggestResponse {
            detections: Vec::new(),
            recommendations: Vec::new(),
            summary: SuggestSummary {
                detected_count: 0,
                recommended_count: 0,
                installable_count: 0,
            },
        };

        assert_eq!(
            render_skill_suggest_human(&response, false),
            "➤ Detected technologies\n  none\n\n➤ Recommended skills\n  none\n\n➤ Summary\n  Detected: 0\n  Recommended: 0\n  Installable: 0"
        );
    }

    #[test]
    fn skill_command_error_renders_failure_and_hint() {
        assert_eq!(
            render_skill_command_error("project root is unreadable", "Check permissions", false),
            vec![
                "✗ error project root is unreadable".to_string(),
                "  Hint: Check permissions".to_string(),
            ]
        );
    }

    #[test]
    fn suggest_install_batch_start_uses_heading_shape() {
        assert_eq!(
            render_suggest_install_batch_start(SuggestInstallMode::InstallAll, 2, false),
            vec![
                "➤ Install recommendations".to_string(),
                "  Installing 2 recommended skills...".to_string(),
            ]
        );
        assert_eq!(
            render_suggest_install_batch_start(SuggestInstallMode::Interactive, 1, false),
            vec![
                "➤ Install recommendations".to_string(),
                "  Installing 1 selected recommended skill...".to_string(),
            ]
        );
    }

    #[test]
    fn suggest_install_output_mode_prefers_json() {
        assert_eq!(
            detect_suggest_install_output_mode(true, true, None, None, Some("xterm-256color")),
            SuggestInstallOutputMode::Json
        );
    }

    #[test]
    fn suggest_install_output_mode_falls_back_to_human_line_without_tty() {
        assert_eq!(
            detect_suggest_install_output_mode(false, false, None, None, Some("xterm-256color")),
            SuggestInstallOutputMode::HumanLine { use_color: false }
        );
    }

    #[test]
    fn suggest_install_output_mode_selects_human_live_for_tty_by_default() {
        assert_eq!(
            detect_suggest_install_output_mode(false, true, None, None, Some("xterm-256color")),
            SuggestInstallOutputMode::HumanLive { use_color: true }
        );
    }

    #[test]
    fn suggest_install_output_mode_honors_no_color() {
        assert_eq!(
            detect_suggest_install_output_mode(
                false,
                true,
                Some("1"),
                None,
                Some("xterm-256color")
            ),
            SuggestInstallOutputMode::HumanLive { use_color: false }
        );
    }

    #[test]
    fn suggest_install_output_mode_honors_clicolor_zero() {
        assert_eq!(
            detect_suggest_install_output_mode(
                false,
                true,
                None,
                Some("0"),
                Some("xterm-256color")
            ),
            SuggestInstallOutputMode::HumanLive { use_color: false }
        );
    }

    #[test]
    fn suggest_install_output_mode_falls_back_to_human_line_for_dumb_term() {
        assert_eq!(
            detect_suggest_install_output_mode(false, true, None, None, Some("dumb")),
            SuggestInstallOutputMode::HumanLine { use_color: false }
        );
    }

    #[test]
    fn suggest_install_completion_summary_reports_explicit_counts_for_mixed_results() {
        let response = SuggestInstallJsonResponse {
            suggest: agentsync::skills::suggest::SuggestJsonResponse {
                detections: Vec::new(),
                recommendations: Vec::new(),
                summary: agentsync::skills::suggest::SuggestSummary {
                    detected_count: 0,
                    recommended_count: 3,
                    installable_count: 3,
                },
            },
            mode: SuggestInstallMode::InstallAll,
            selected_skill_ids: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            results: vec![
                agentsync::skills::suggest::SuggestInstallResult {
                    skill_id: "a".to_string(),
                    provider_skill_id: "dallay/test-a".to_string(),
                    status: SuggestInstallStatus::Installed,
                    error_message: None,
                },
                agentsync::skills::suggest::SuggestInstallResult {
                    skill_id: "b".to_string(),
                    provider_skill_id: "dallay/test-b".to_string(),
                    status: SuggestInstallStatus::AlreadyInstalled,
                    error_message: None,
                },
                agentsync::skills::suggest::SuggestInstallResult {
                    skill_id: "c".to_string(),
                    provider_skill_id: "dallay/test-c".to_string(),
                    status: SuggestInstallStatus::Failed,
                    error_message: Some("simulated install failure".to_string()),
                },
            ],
        };

        let summary = render_suggest_install_completion_summary(&response, false);
        assert!(
            summary.contains("Recommendation install summary"),
            "{summary}"
        );
        assert!(summary.contains("Installed: 1"), "{summary}");
        assert!(summary.contains("Already installed: 1"), "{summary}");
        assert!(summary.contains("Failed: 1"), "{summary}");
        assert!(summary.contains("Failure details:"), "{summary}");
        assert!(
            summary.contains("- c: simulated install failure"),
            "{summary}"
        );
    }

    #[test]
    fn suggest_install_completion_summary_reports_nothing_installable_with_explicit_counts() {
        let response = SuggestInstallJsonResponse {
            suggest: agentsync::skills::suggest::SuggestJsonResponse {
                detections: Vec::new(),
                recommendations: Vec::new(),
                summary: agentsync::skills::suggest::SuggestSummary {
                    detected_count: 0,
                    recommended_count: 2,
                    installable_count: 0,
                },
            },
            mode: SuggestInstallMode::InstallAll,
            selected_skill_ids: vec!["a".to_string(), "b".to_string()],
            results: vec![
                agentsync::skills::suggest::SuggestInstallResult {
                    skill_id: "a".to_string(),
                    provider_skill_id: "dallay/test-a".to_string(),
                    status: SuggestInstallStatus::AlreadyInstalled,
                    error_message: None,
                },
                agentsync::skills::suggest::SuggestInstallResult {
                    skill_id: "b".to_string(),
                    provider_skill_id: "dallay/test-b".to_string(),
                    status: SuggestInstallStatus::AlreadyInstalled,
                    error_message: None,
                },
            ],
        };

        let summary = render_suggest_install_completion_summary(&response, false);
        assert!(
            summary.contains("Recommendation install summary"),
            "{summary}"
        );
        assert!(summary.contains("Installed: 0"), "{summary}");
        assert!(summary.contains("Already installed: 2"), "{summary}");
        assert!(summary.contains("Failed: 0"), "{summary}");
        assert!(
            summary.contains("Note: nothing installable to do."),
            "{summary}"
        );
    }

    #[test]
    fn suggest_install_completion_summary_reports_nothing_installable_for_install_all_with_no_selection()
     {
        let response = SuggestInstallJsonResponse {
            suggest: agentsync::skills::suggest::SuggestJsonResponse {
                detections: Vec::new(),
                recommendations: Vec::new(),
                summary: agentsync::skills::suggest::SuggestSummary {
                    detected_count: 0,
                    recommended_count: 0,
                    installable_count: 0,
                },
            },
            mode: SuggestInstallMode::InstallAll,
            selected_skill_ids: Vec::new(),
            results: Vec::new(),
        };

        let summary = render_suggest_install_completion_summary(&response, false);
        assert!(
            summary.contains("Note: nothing installable to do."),
            "{summary}"
        );
        assert!(!summary.contains("Note: nothing selected."), "{summary}");
    }

    #[derive(Clone, Default)]
    struct RecordingLiveWriter {
        output: Arc<Mutex<String>>,
    }

    impl RecordingLiveWriter {
        fn snapshot(&self) -> String {
            self.output.lock().unwrap().clone()
        }
    }

    impl SuggestInstallLiveWriter for RecordingLiveWriter {
        fn write_str(&mut self, text: &str) {
            self.output.lock().unwrap().push_str(text);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn suggest_install_live_reporter_finalize_cleans_up_partial_spinner_frame() {
        let writer = RecordingLiveWriter::default();
        let mut reporter = SuggestInstallLiveReporter::with_writer(
            false,
            Box::new(writer.clone()),
            Duration::from_millis(10),
        );

        reporter.on_event(SuggestInstallProgressEvent::Resolving {
            skill_id: "rust-async-patterns".to_string(),
        });
        std::thread::sleep(Duration::from_millis(25));
        reporter.finalize();

        let output = writer.snapshot();
        assert!(
            output.contains("resolving rust-async-patterns"),
            "{output:?}"
        );
        assert!(output.ends_with('\n'), "{output:?}");
    }

    // Tests for output mode detection
    #[test]
    fn detect_suggest_install_output_mode_json_when_json_flag_true() {
        // Even with TTY, JSON should be preferred when json=true
        let mode = detect_suggest_install_output_mode(true, true, None, None, None);
        assert!(matches!(mode, SuggestInstallOutputMode::Json));
    }

    #[test]
    fn detect_suggest_install_output_mode_human_live_when_tty() {
        // With TTY and no json, should be HumanLive
        let mode = detect_suggest_install_output_mode(false, true, None, None, None);
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLive { use_color: true }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_human_line_when_no_tty() {
        // Without TTY, should be HumanLine
        let mode = detect_suggest_install_output_mode(false, false, None, None, None);
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLine { use_color: false }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_human_line_when_dumb_term() {
        // With "dumb" term, should be HumanLine (not live)
        let mode = detect_suggest_install_output_mode(false, true, None, None, Some("dumb"));
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLine { use_color: false }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_no_color_with_no_color_env() {
        // NO_COLOR=1 should disable color even with TTY
        let mode = detect_suggest_install_output_mode(false, true, Some("1"), None, None);
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLive { use_color: false }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_no_color_with_clicolor_zero() {
        // CLICOLOR=0 should disable color
        let mode = detect_suggest_install_output_mode(false, true, None, Some("0"), None);
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLive { use_color: false }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_color_with_clicolor_nonzero() {
        // CLICOLOR non-zero enables color
        let mode = detect_suggest_install_output_mode(false, true, None, Some("1"), None);
        assert!(matches!(
            mode,
            SuggestInstallOutputMode::HumanLive { use_color: true }
        ));
    }

    #[test]
    fn detect_suggest_install_output_mode_json_stays_json_without_tty() {
        let mode =
            detect_suggest_install_output_mode(true, false, Some("1"), Some("0"), Some("dumb"));
        assert!(matches!(mode, SuggestInstallOutputMode::Json));
    }

    // Tests for skill_id validation
    #[test]
    fn validate_skill_id_rejects_dotdot() {
        let result = validate_skill_id("..");
        assert!(result.is_err());
    }

    #[test]
    fn validate_skill_id_rejects_dot() {
        let result = validate_skill_id(".");
        assert!(result.is_err());
    }

    // Tests for remediation_for_error
    #[test]
    fn remediation_for_error_manifest() {
        let msg = "failed to parse manifest";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("manifest"));
    }

    #[test]
    fn remediation_for_error_network() {
        let msg = "network error: connection refused";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("network"));
    }

    #[test]
    fn remediation_for_error_download() {
        let msg = "download failed";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("network"));
    }

    #[test]
    fn remediation_for_error_archive() {
        let msg = "invalid archive format";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("archive"));
    }

    #[test]
    fn remediation_for_error_permission() {
        let msg = "permission denied";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("permission"));
    }

    #[test]
    fn remediation_for_error_fallback() {
        let msg = "some unknown error";
        let remediation = remediation_for_error(msg);
        assert!(remediation.contains("above error"));
    }
}
