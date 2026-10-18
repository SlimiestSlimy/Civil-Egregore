//! Civil Egregore's client: the window showing a world ([`renderer`])
//! and the menus laid over it ([`gui`]); and what game content's code
//! reaches in it ([`client_api`]).
//!
//! The designs: `docs/renderer.md`, `docs/gui.md`; function by
//! function: `docs/reference.md`.

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

pub mod client_api;
mod gui;
mod renderer;
mod server_end;

pub use renderer::run_window;
