//! Baseline policies use only public legal actions and the ordinary resolver.
use crate::state::{Game, PlayerId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPolicy {
    Random,
    First,
    Heuristic,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiController {
    pub policy: AiPolicy,
    pub seed: u64,
}

impl Game {
    /// Unlisted seats remain human-controlled. Configure before the first action.
    pub fn with_ai(mut self, seats: Vec<(PlayerId, AiPolicy)>, seed: u64) -> Result<Self, String> {
        if !self.log().is_empty() {
            return Err("configure AI seats before play".into());
        }
        let mut controllers = BTreeMap::new();
        for (player, policy) in seats {
            if self.player(player).is_none()
                || controllers
                    .insert(
                        player,
                        AiController {
                            policy,
                            seed: seed ^ u64::from(player).wrapping_mul(0x9e3779b97f4a7c15),
                        },
                    )
                    .is_some()
            {
                return Err("AI seats must name distinct existing players".into());
            }
        }
        self.state.ai = controllers;
        Ok(self)
    }
    pub fn ai_controller(&self, player: PlayerId) -> Option<&AiController> {
        self.state.ai.get(&player)
    }
}
