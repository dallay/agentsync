//! AgentSync - AI Agent Configuration Synchronization
//!
//! A tool for managing AI coding assistant configurations using symbolic links.
//! Unlike tools that copy files, AgentSync creates symlinks to maintain a single
//! source of truth for your AI agent instructions.

pub mod config;
pub mod gitignore;
pub mod init;
pub mod linker;
pub mod mcp;

pub use config::Config;
pub use linker::{Linker, SyncOptions, SyncResult};
pub use mcp::{McpAgent, McpGenerator, McpSyncResult};
