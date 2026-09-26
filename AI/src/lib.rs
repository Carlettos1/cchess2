//! Deterministic AI policies built on the public rules engine API.
pub use cchess_core::ai::{AiController, AiPolicy};
use cchess_core::{cards::MatchRng, ActionIntent, ActionKind, Game, ResolvedAction, RuleError};
use std::collections::BTreeSet;

pub mod heuristic;
pub mod mcts;

/// AI action selection and execution for a game. Import this trait to use its methods.
pub trait GameAi {
    fn ai_legal_actions(&self) -> Vec<ActionIntent>;
    fn choose_ai_action(&self) -> Option<ActionIntent>;
    fn play_ai_step(&mut self) -> Result<Option<ResolvedAction>, RuleError>;
    fn play_ai_turn(&mut self, max_actions: usize) -> Result<Vec<ResolvedAction>, RuleError>;
}

impl GameAi for Game {
    /// All distinct actionable choices; passing is a fallback, not an early exit.
    fn ai_legal_actions(&self) -> Vec<ActionIntent> {
        if self.is_over() {
            return vec![];
        }
        if let Some(pending) = self.pending_promotion() {
            return self.legal_actions(pending.piece);
        }
        let owner = self.turn().player;
        let mut actions = Vec::new();
        for card in &self.player(owner).unwrap().cards.hand {
            actions.extend(self.legal_card_plays(*card));
        }
        for piece in self.pieces().filter(|p| p.owner == owner) {
            actions.extend(self.legal_actions(piece.id));
        }
        // Membership only: retain first-seen order so seeds still select the same action.
        // Borrowed keys avoid copying ability names into the membership set.
        let mut seen = BTreeSet::new();
        let mut unique: Vec<_> = actions
            .iter()
            .filter(|action| seen.insert(*action))
            .cloned()
            .collect();
        if unique.is_empty() {
            unique.push(ActionIntent::EndTurn { player: owner });
        }
        unique
    }
    /// Pure selection: repeated previews don't consume random state. The serialized
    /// seed and committed action index define each independent random decision.
    fn choose_ai_action(&self) -> Option<ActionIntent> {
        let controller = self.ai_controller(self.turn().player)?;
        match controller.policy {
            AiPolicy::First => first_ai_action(self),
            AiPolicy::Heuristic => heuristic::choose_action(self),
            AiPolicy::Mcts => mcts::search(self, mcts::SearchConfig::default()).action,
            AiPolicy::Random => {
                let actions = self.ai_legal_actions();
                if actions.is_empty() {
                    return None;
                }
                let mut rng = MatchRng::new(
                    controller
                        .seed
                        .wrapping_add(self.action_count().wrapping_mul(0xd1342543de82ef95)),
                );
                Some(actions[rng.below(actions.len() as u64)].clone())
            }
        }
    }
    fn play_ai_step(&mut self) -> Result<Option<ResolvedAction>, RuleError> {
        self.choose_ai_action()
            .map(|intent| self.apply(intent))
            .transpose()
    }
    /// Run one seat's turn. A caller-supplied work budget permits responsive UIs;
    /// reaching it pauses without discarding remaining actions or forcing a pass.
    fn play_ai_turn(&mut self, max_actions: usize) -> Result<Vec<ResolvedAction>, RuleError> {
        let turn = self.turn().number;
        let mut actions = Vec::new();
        while self.turn().number == turn && actions.len() < max_actions {
            let Some(action) = self.play_ai_step()? else {
                break;
            };
            actions.push(action);
        }
        Ok(actions)
    }
}

fn first_ai_action(game: &Game) -> Option<ActionIntent> {
    if game.is_over() {
        return None;
    }
    if let Some(pending) = game.pending_promotion() {
        return game.legal_actions(pending.piece).into_iter().next();
    }
    let owner = game.turn().player;
    for card in &game.player(owner)?.cards.hand {
        if let Some(action) = game.legal_card_plays(*card).into_iter().next() {
            return Some(action);
        }
    }
    for kind in [ActionKind::Take, ActionKind::Attack] {
        for piece in game.pieces().filter(|p| p.owner == owner) {
            if let Some(action) = game.legal_piece_actions(piece.id, kind).into_iter().next() {
                return Some(action);
            }
        }
    }
    for piece in game.pieces().filter(|p| p.owner == owner) {
        if let Some(action) = game.legal_abilities(piece.id).into_iter().next() {
            return Some(action);
        }
    }
    for piece in game.pieces().filter(|p| p.owner == owner) {
        if let Some(action) = game
            .legal_piece_actions(piece.id, ActionKind::Move)
            .into_iter()
            .next()
        {
            return Some(action);
        }
    }
    Some(ActionIntent::EndTurn { player: owner })
}
