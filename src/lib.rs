//! AgentSync - AI Agent Configuration Synchronization
//!
//! A tool for managing AI coding assistant configurations using symbolic links.
//! Unlike tools that copy files, AgentSync creates symlinks to maintain a single
//! source of truth for your AI agent instructions.

/// Handles the configuration loading and parsing.
pub mod config;
/// Manages the `.gitignore` file to ignore synced files.
pub mod gitignore;
/// Initializes a new `agentsync.toml` configuration file.
pub mod init;
/// The core logic for creating and managing symbolic links.
pub mod linker;
/// Handles the synchronization of MCP (Model Context Protocol) configurations.
pub mod mcp;

/// The main configuration for AgentSync, parsed from `agentsync.toml`.
pub use config::Config;
/// The core struct responsible for creating and managing symlinks.
pub use linker::{Linker, SyncOptions, SyncResult};
/// MCP-related structs and utilities for synchronizing MCP configurations.
pub use mcp::{McpAgent, McpGenerator, McpSyncResult};
