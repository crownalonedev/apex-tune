//! Debloat: preinstalled apps, startup items, background tasks and services.
//!
//! Each item is its own journaled action, so one item failing (some tasks
//! are owned by TrustedInstaller) never undoes or blocks the others, and
//! each can be restored on its own.

pub mod apps;
pub mod background;
pub mod startup;
