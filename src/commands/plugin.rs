use agentsync::config::Config;
use agentsync::plugins::{PluginApplyResult, PluginLock, PluginManager, PluginSelection};
use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum PluginCommand {
    /// Resolve a configured marketplace/plugin and write its immutable lock entry.
    Add(PluginSelectionArgs),
    /// Re-resolve a configured marketplace/plugin and refresh its immutable lock entry.
    Update(PluginSelectionArgs),
    /// List locked repository-owned plugins.
    List(PluginOutputArgs),
    /// Remove a locked plugin and its AgentSync-owned skills.
    Remove(PluginSelectionArgs),
    /// Validate locked sources and report materialization drift without changing files.
    Status(PluginOutputArgs),
}

#[derive(Args, Debug)]
pub struct PluginSelectionArgs {
    /// Selection in the form marketplace/plugin.
    pub selection: String,
    /// Output machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct PluginOutputArgs {
    /// Output machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

pub fn run_plugin(command: PluginCommand, project_root: PathBuf) -> Result<()> {
    let config_path = Config::find_config(&project_root)?;
    let config = Config::load(&config_path)?;
    let manager = PluginManager::new(
        Config::project_root(&config_path),
        config_path,
        config.plugins,
    );

    match command {
        PluginCommand::Add(args) => run_lock_operation(&manager, &args, false),
        PluginCommand::Update(args) => run_lock_operation(&manager, &args, true),
        PluginCommand::List(args) => run_list(&manager, args.json),
        PluginCommand::Remove(args) => run_remove(&manager, &args),
        PluginCommand::Status(args) => run_status(&manager, args.json),
    }
}

fn run_lock_operation(
    manager: &PluginManager,
    args: &PluginSelectionArgs,
    update: bool,
) -> Result<()> {
    let selection = parse_selection(&args.selection)?;
    let result = if update {
        manager.update(&selection)?
    } else {
        manager.add(&selection)?
    };
    print_result(
        args.json,
        if update { "updated" } else { "added" },
        &selection,
        &result,
    )
}

fn run_remove(manager: &PluginManager, args: &PluginSelectionArgs) -> Result<()> {
    let selection = parse_selection(&args.selection)?;
    let result = manager.remove(&selection, false)?;
    print_result(args.json, "removed", &selection, &result)
}

fn run_list(manager: &PluginManager, json: bool) -> Result<()> {
    let lock = match manager.load_lock() {
        Ok(lock) => lock,
        Err(error) if error.to_string().contains("failed to read plugin lockfile") => {
            PluginLock::default()
        }
        Err(error) => return Err(error),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&lock)?);
    } else if lock.plugins.is_empty() {
        println!("No repository-owned plugins are locked.");
    } else {
        for plugin in lock.plugins.values() {
            println!(
                "{} — {} skill(s), {} MCP server(s), revision {}",
                plugin.key(),
                plugin.skills.len(),
                plugin.mcp_servers.len(),
                plugin.source.revision
            );
        }
    }
    Ok(())
}

fn run_status(manager: &PluginManager, json: bool) -> Result<()> {
    let result = manager.apply(true)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "skills": result.updated,
                "mcp_servers": result.mcp_servers.keys().collect::<Vec<_>>(),
            }))?
        );
    } else {
        println!(
            "Plugin sources are locked and available ({} skill(s), {} MCP server(s)).",
            result.updated,
            result.mcp_servers.len()
        );
    }
    Ok(())
}

fn print_result(
    json: bool,
    status: &str,
    selection: &PluginSelection,
    result: &PluginApplyResult,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "plugin": selection.key(),
                "status": status,
                "created": result.created,
                "updated": result.updated,
                "skipped": result.skipped,
                "removed": result.removed,
                "mcp_servers": result.mcp_servers.keys().collect::<Vec<_>>(),
            }))?
        );
    } else {
        println!(
            "{} {} (created {}, updated {}, skipped {}, removed {})",
            status,
            selection.key(),
            result.created,
            result.updated,
            result.skipped,
            result.removed
        );
    }
    Ok(())
}

fn parse_selection(value: &str) -> Result<PluginSelection> {
    let (marketplace, plugin) = value
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("plugin selection must use marketplace/plugin"))?;
    ensure_no_slash(marketplace, "marketplace")?;
    ensure_no_slash(plugin, "plugin")?;
    Ok(PluginSelection {
        marketplace: marketplace.to_string(),
        plugin: plugin.to_string(),
    })
}

fn ensure_no_slash(value: &str, kind: &str) -> Result<()> {
    if value.is_empty() || value.contains('/') || value.contains('\\') {
        bail!("invalid {kind} in plugin selection");
    }
    Ok(())
}
