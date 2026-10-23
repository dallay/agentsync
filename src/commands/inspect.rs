use agentsync::config::Config;
use agentsync::inspect::{DiagnosticSeverity, ResolvedConfiguration, resolve_configuration};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct InspectArgs {
    /// Assistant identifier from agentsync.toml (for example: opencode or codex).
    #[arg(long)]
    pub agent: String,
    /// Output machine-readable JSON.
    #[arg(long)]
    pub json: bool,
    /// Exit with code 1 when the report contains an error-severity diagnostic.
    #[arg(long)]
    pub strict: bool,
}

pub fn run_inspect(args: InspectArgs, project_root: PathBuf) -> Result<()> {
    let config_path = Config::find_config(&project_root)?;
    let config = Config::load(&config_path)?;
    let report = resolve_configuration(&config, &config_path, &args.agent)?;
    if args.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human(&report);
    }
    if args.strict
        && report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        anyhow::bail!("inspect found error-severity diagnostics");
    }
    Ok(())
}

fn print_human(report: &ResolvedConfiguration) {
    println!("AgentSync-managed configuration for {}", report.agent);
    println!("Scope: {}", report.scope);
    for item in &report.items {
        println!("\n{} [{}]", item.kind, item.state.state_name());
        println!("  source: {}", item.source);
        println!("  target: {} ({})", item.target, item.sync_mode);
        println!("  destination: {}", item.destination);
        println!("  owner: {} ({})", item.owner, item.origin);
        println!("  provider support: {}", item.provider_support);
        for child in &item.children {
            println!(
                "  child: {} -> {} [{}]",
                child.source,
                child.destination,
                child.state.state_name()
            );
        }
    }
    println!(
        "\nMCP: {} (enabled: {}, merge: {})",
        report.mcp.capability, report.mcp.enabled, report.mcp.merge_strategy
    );
    if let Some(destination) = &report.mcp.destination {
        println!("  destination: {destination}");
    }
    for server in &report.mcp.servers {
        println!(
            "  {} [{}; {}; {}]",
            server.name,
            server.origin,
            server.state.state_name(),
            if server.disabled {
                "disabled"
            } else if server.included {
                "included"
            } else {
                "not included"
            }
        );
    }
    for diagnostic in &report.diagnostics {
        println!(
            "\n{:?} [{:?}] target={} path={}: {}",
            diagnostic.code,
            diagnostic.severity,
            diagnostic.target.as_deref().unwrap_or("-"),
            diagnostic.path.as_deref().unwrap_or("-"),
            diagnostic.message
        );
    }
    println!("\n{}", report.scope_notice);
}

trait StateName {
    fn state_name(&self) -> &'static str;
}
impl StateName for agentsync::inspect::ItemState {
    fn state_name(&self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Projected => "projected",
            Self::Observed => "observed",
            Self::Unsupported => "unsupported",
            Self::Conflicted => "conflicted",
        }
    }
}
impl StateName for agentsync::inspect::McpServerReport {
    fn state_name(&self) -> &'static str {
        self.state.state_name()
    }
}
