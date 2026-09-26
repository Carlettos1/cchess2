//! Setup terrain and seeded ray selection. Geometry itself remains pure.
use crate::{cards::MatchRng, content::ActionKind, state::EntityId, targeting, Game, Pos};
use std::collections::BTreeSet;

impl Game {
    /// Set the match seed before play. Deck setup also sets the shared RNG seed.
    pub fn with_seed(mut self, seed: u64) -> Result<Self, String> {
        if self.action_count() != 0 {
            return Err("seed must be set before play".into());
        }
        self.state.rng = MatchRng::new(seed);
        Ok(self)
    }

    /// Ordinary squares are buildable; magic must be explicitly marked at setup.
    pub fn with_terrain(mut self, magic: Vec<Pos>, unbuildable: Vec<Pos>) -> Result<Self, String> {
        if self.action_count() != 0
            || magic
                .iter()
                .chain(&unbuildable)
                .any(|p| !self.is_playable(*p))
        {
            return Err("terrain must contain in-bounds squares and be set before play".into());
        }
        self.state.magic_tiles = magic.into_iter().collect();
        self.state.unbuildable_tiles = unbuildable.into_iter().collect();
        Ok(self)
    }

    pub fn is_magic_tile(&self, pos: Pos) -> bool {
        self.state.magic_tiles.contains(&pos)
    }
    pub fn is_buildable_tile(&self, pos: Pos) -> bool {
        self.is_playable(pos) && !self.state.unbuildable_tiles.contains(&pos)
    }

    pub(crate) fn random_directions(&self, source: EntityId) -> Vec<(i32, i32)> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        let definition = self.registry.get(&piece.archetype).unwrap();
        if !definition.definition.random_ray {
            return vec![];
        }
        definition
            .rules
            .iter()
            .filter(|r| matches!(r.kind, ActionKind::Move | ActionKind::Take))
            .flat_map(|r| {
                targeting::resolve(self, source, piece.owner, &r.target)
                    .into_iter()
                    .filter(move |pos| {
                        self.at(*pos)
                            .is_none_or(|p| self.can_receive(source, p.id, r.kind))
                    })
            })
            .map(|p| {
                (
                    (p.x - piece.position.x).signum(),
                    (p.y - piece.position.y).signum(),
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Peek without consuming RNG. Move and Take share a single direction;
    /// only a successful committed action consumes the corresponding sample.
    pub fn random_direction(&self, source: EntityId) -> Option<(i32, i32)> {
        let choices = self.random_directions(source);
        if choices.is_empty() {
            None
        } else {
            let mut rng = self.state.rng.clone();
            Some(choices[rng.below(choices.len() as u64)])
        }
    }
}

impl Game {
    pub fn expansion_demo(registry: crate::Registry) -> Result<Self, String> {
        use crate::{
            cards::DeckDef,
            state::{Placement, Player},
            Bounds, Direction,
        };
        let mut placements = Vec::new();
        for (name, x, y) in [
            ("king", 4, 0),
            ("catapult", 0, 0),
            ("imp", 1, 0),
            ("basilisk", 2, 0),
            ("gargoyle", 6, 0),
            ("mermaid", 2, 1),
            ("ogre", 4, 1),
            ("golem", 5, 1),
            ("oni", 7, 0),
            ("necromancer", 3, 0),
            ("dragon", 5, 0),
            ("crazy_pawn", 1, 2),
            ("cannon", 0, 1),
            ("ship", 5, 2),
            ("warlock", 3, 1),
        ] {
            placements.push(Placement::new(name, 0, Pos::new(x, y)));
            placements.push(Placement::new(name, 1, Pos::new(7 - x, 7 - y)));
        }
        let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
            .into_iter()
            .map(|mut p| {
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p
            })
            .collect();
        let deck: DeckDef = ron::from_str(include_str!("../../assets/decks/starter.ron"))
            .map_err(|e| e.to_string())?;
        Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            players,
            placements,
        )?
        .with_terrain(
            vec![
                Pos::new(2, 2),
                Pos::new(3, 2),
                Pos::new(4, 2),
                Pos::new(3, 5),
                Pos::new(4, 5),
                Pos::new(5, 5),
            ],
            vec![],
        )?
        .with_decks(vec![(0, deck.clone()), (1, deck)], 42, 3, 1, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialized_rng_and_terrain_resume_identical_future_actions() {
        let mut game = Game::expansion_demo(crate::Registry::built_in().unwrap()).unwrap();
        let source = game.at(Pos::new(1, 2)).unwrap().id;
        let action = game
            .legal_actions(source)
            .into_iter()
            .find(|a| matches!(a, crate::ActionIntent::Piece { .. }))
            .unwrap();
        game.apply(action).unwrap();
        let mut resumed = game.clone();
        resumed.state = ron::from_str(&ron::to_string(&game.state).unwrap()).unwrap();
        resumed.rebuild_occupancy();
        for _ in 0..8 {
            let action = game
                .pieces()
                .find(|p| p.owner == game.turn().player && p.archetype == "crazy_pawn")
                .and_then(|p| {
                    game.legal_actions(p.id)
                        .into_iter()
                        .find(|a| matches!(a, crate::ActionIntent::Piece { .. }))
                })
                .unwrap_or(crate::ActionIntent::EndTurn {
                    player: game.turn().player,
                });
            assert_eq!(game.apply(action.clone()), resumed.apply(action));
            assert_eq!(game.snapshot(), resumed.snapshot());
        }
    }
}
