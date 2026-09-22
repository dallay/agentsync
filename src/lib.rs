//! AgentSync - AI Agent Configuration Synchronization
//!
//! A tool for managing AI coding assistant configurations using symbolic links.
//! Unlike tools that copy files, AgentSync creates symlinks to maintain a single
//! source of truth for your AI agent instructions.

pub(crate) mod agent_ids;
pub mod config;
pub mod gitignore;
pub mod init;
pub mod linker;
pub mod logging;
pub mod mcp;
pub mod plugins;
pub mod skills;
pub mod skills_layout;
pub mod update_check;

pub use config::Config;
pub use linker::{Linker, SyncOptions, SyncResult};
pub use mcp::{McpAgent, McpAgentDocumentation, McpGenerator, McpSyncResult};

/// Convert an AgentSync command filename to the Z-Code command filename.
/// Z-Code strips only `.md`, while AgentSync's canonical command convention is
/// `<name>.agent.md`; remove the compatibility suffix so the slash command is
/// exposed as `/name`.
pub fn zcode_command_destination(file_name: &str) -> String {
    file_name
        .strip_suffix(".agent.md")
        .map_or_else(|| file_name.to_string(), |name| format!("{name}.md"))
}
pub use plugins::{
    PluginApplyMode, PluginApplyResult, PluginManager, PluginMcpApproval, PluginMcpStatus,
    PluginSource, PluginStatusReport, PluginsConfig,
};
