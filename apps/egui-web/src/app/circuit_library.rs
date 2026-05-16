//! In-memory named circuit library for the toolbar circuit picker.
//!
//! The stored circuit body is the canonical `{"cols":[...]}` JSON shared with
//! URL hashes and undo checkpoints. Submodules keep domain operations, app
//! integration, persistence, startup reconciliation, and wasm test hooks apart.

mod app_adapter;
mod model;
mod startup;
mod storage;
mod test_hooks;

pub(crate) use model::{CircuitEntry, CircuitId, CircuitLibrary};
pub(crate) use storage::persist_library;
pub(crate) use test_hooks::wire_test_hooks;
