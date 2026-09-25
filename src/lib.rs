//! Public facade for the rules engine, AI policies, and terminal interface.
pub use cchess_ai::GameAi;
pub use cchess_core::*;
pub use cchess_tui as tui;

pub mod ai {
    pub use cchess_ai::*;
}
