//! Moving a deployment from the version it runs to another one.
//!
//! The mechanism at the far end already exists: the operator reconciles an
//! `IdentityInstanceUpgrade` and has since it was written. What was missing is
//! everything before it, starting with deciding whether an upgrade may be
//! asked for at all.

pub mod commands;
pub mod policy;
pub mod ports;
pub mod run;
pub mod run_ports;
pub mod scheduler;
pub mod service;
