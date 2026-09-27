//! Baseline policies use only public legal actions and the ordinary resolver.
use crate::state::{Game, PlayerId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiPolicy {
    Random,
    First,
    Heuristic,
    Mcts,
    Evolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiGenome {
    pub evaluation: i32,
    pub capture: i32,
    pub attack: i32,
    pub movement: i32,
    pub ability: i32,
    pub card: i32,
    pub patience: i32,
}
impl Default for AiGenome {
    fn default() -> Self {
        Self {
            evaluation: 100,
            capture: 80,
            attack: 60,
            movement: 0,
            ability: 20,
            card: 20,
            patience: -30,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiController {
    pub policy: AiPolicy,
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genome: Option<AiGenome>,
}

impl Game {
    /// Unlisted seats remain human-controlled. Configure before the first action.
    pub fn with_ai(mut self, seats: Vec<(PlayerId, AiPolicy)>, seed: u64) -> Result<Self, String> {
        if self.action_count() != 0 {
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
                            genome: None,
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
    /// Configure evolved policies. Keeping the genome in the controller makes
    /// saves, undo and replay independent of the original stamp file.
    pub fn with_evolved_ai(
        mut self,
        seats: Vec<(PlayerId, AiGenome)>,
        seed: u64,
    ) -> Result<Self, String> {
        if self.action_count() != 0 {
            return Err("configure AI seats before play".into());
        }
        let mut controllers = BTreeMap::new();
        for (player, genome) in seats {
            if self.player(player).is_none()
                || controllers
                    .insert(
                        player,
                        AiController {
                            policy: AiPolicy::Evolved,
                            seed: seed ^ u64::from(player).wrapping_mul(0x9e3779b97f4a7c15),
                            genome: Some(genome),
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
