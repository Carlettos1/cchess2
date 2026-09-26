//! Deterministic square-lattice rules, independent of the legacy sandbox and UI.
pub mod abilities;
pub mod cards;
pub mod clock;
pub mod commands;
pub mod content;
pub mod effects;
pub mod events;
pub mod geometry;
pub mod rules;
pub mod siege;
pub mod state;
pub mod statuses;
pub mod targeting;

pub use commands::{ActionIntent, Event, ResolvedAction};
pub use content::{ActionKind, Registry};
pub use geometry::{Bounds, Direction, Pos};
pub use rules::RuleError;
pub use state::Game;

pub mod expansion;

pub mod multiplayer;

pub mod revival;

pub mod mana;

pub mod ai;

pub mod full_board;

pub mod simulation;
