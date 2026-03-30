//! Skills module
//!
//! Core types and re-exports for skill management.

pub mod catalog;
pub mod detect;
pub mod install;
pub mod manifest;
pub mod provider;
pub mod registry;
pub mod suggest;
pub mod transaction;
pub mod uninstall;
pub mod update;

pub use catalog::*;
pub use detect::*;
pub use install::*;
pub use manifest::*;
pub use provider::*;
pub use registry::*;
pub use suggest::*;
pub use transaction::*;
pub use uninstall::*;
pub use update::*;
