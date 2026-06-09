//! Command handlers — one module per top-level CLI command group.
//!
//! Each module receives parsed CLI arguments and orchestrates the work by
//! calling into the infrastructure layer (`auth`, `client`, `context`, etc.).
//! No business logic lives here beyond what is specific to a single command.

pub mod auth;
pub mod doctor;
pub mod init;
pub mod issue;
