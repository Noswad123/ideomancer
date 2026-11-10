//! Reusable static discovery and saved-catalog class diagrams.
mod adapters;
mod analysis;
mod analyze;
mod catalog;
mod cli;
mod discovery;
mod extensions;
mod focus;
mod javascript;
mod javascript_native;
mod js_focus;
mod js_modules;
mod manifest;
mod openapi;
mod packet;
mod plan;
mod presentation;
mod render;
mod router_context;
mod validate;
mod workflow;

pub mod engine;

/// Executable plumbing, not part of the reusable engine contract.
#[doc(hidden)]
pub fn cli_main() {
    cli::main();
}
