//! Lightweight search branches that retain rule state and action numbering,
//! without copying or accumulating historical action traces.
use crate::{ActionIntent, Game, ResolvedAction, RuleError};
use std::ops::Deref;

/// An isolated search branch. Read-only game APIs are available through Deref;
/// apply uses the same transactional resolver as a live game. Its log is empty,
/// and its snapshots omit history, so use the live Game for saves/replay logs.
/// No conversion back into a live Game is exposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simulation(Game);

impl Game {
    /// Number of committed actions, including actions on a history-free branch.
    pub fn action_count(&self) -> u64 {
        self.simulation_actions.unwrap_or(self.log().len() as u64)
    }

    /// Share compiled content and copy current state only. RNG, effects, action
    /// IDs and rollback behavior remain identical to an ordinary Game clone.
    pub fn simulation(&self) -> Simulation {
        Simulation(Self {
            simulation_actions: Some(self.action_count()),
            registry: self.registry.clone(),
            state: self.state.clone_for_resolution(),
            resolver: Default::default(),
            occupancy: self.occupancy.clone(),
        })
    }
}
impl Deref for Simulation {
    type Target = Game;
    fn deref(&self) -> &Game {
        &self.0
    }
}
impl Simulation {
    pub fn apply(&mut self, intent: ActionIntent) -> Result<ResolvedAction, RuleError> {
        self.0.apply(intent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cards::MatchRng, Registry};

    #[test]
    fn simulation_matches_live_events_rng_state_and_ids_without_history() {
        for mut live in [
            Game::classical(Registry::built_in().unwrap()).unwrap(),
            Game::expansion_demo(Registry::built_in().unwrap()).unwrap(),
            Game::four_player(Registry::built_in().unwrap()).unwrap(),
        ] {
            // Start with committed history, not just a fresh position.
            for _ in 0..5 {
                live.apply(ActionIntent::EndTurn {
                    player: live.turn().player,
                })
                .unwrap();
            }
            let mut sim = live.simulation();
            let mut rng = MatchRng::new(51);
            for _ in 0..60 {
                if live.is_over() {
                    break;
                }
                let mut actions = Vec::new();
                for piece in live.pieces().filter(|p| p.owner == live.turn().player) {
                    actions.extend(live.legal_actions(piece.id));
                }
                for card in &live.player(live.turn().player).unwrap().cards.hand {
                    actions.extend(live.legal_card_plays(*card));
                }
                if actions.is_empty() {
                    actions.push(ActionIntent::EndTurn {
                        player: live.turn().player,
                    });
                }
                let action = actions.swap_remove(rng.below(actions.len() as u64));
                assert_eq!(live.apply(action.clone()), sim.apply(action));
                assert_eq!(live.action_count(), sim.action_count());
                assert_eq!(live.state.clone_for_resolution(), sim.0.state);
                assert!(sim.log().is_empty());
                // Nested forks retain the action counter and reproduce all state.
                assert_eq!(sim, sim.simulation());
                let before = sim.clone();
                assert!(sim.apply(ActionIntent::EndTurn { player: 255 }).is_err());
                assert_eq!(before, sim);
            }
        }
    }
}
