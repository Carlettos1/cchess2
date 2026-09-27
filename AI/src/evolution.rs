//! Deterministic, lightweight evolution of distinct heuristic opponents.
//!
//! A [`Stamp`] is a portable checkpoint: it contains the policy genome, its
//! measured ability, an observed personality, and lineage.  The evolutionary
//! runner lives in `src/bin/evolve.rs`; this module is also usable by games and
//! alternative tournament front ends.
use crate::{heuristic, GameAi};
use cchess_core::{ai::AiGenome, ActionIntent, ActionKind, Game};
use serde::{Deserialize, Serialize};

pub type Genome = AiGenome;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Personality {
    /// Per-thousand action frequencies, making stamps stable and float-free.
    pub aggression: u16,
    pub mobility: u16,
    pub creativity: u16,
    pub patience: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    pub id: String,
    pub generation: u32,
    pub parent: Option<String>,
    /// Elo-style ability estimate. New agents start at 1000.
    pub rating: i32,
    pub record: Record,
    pub personality: Personality,
    pub genome: Genome,
}

impl Stamp {
    pub fn founder(id: impl Into<String>, genome: Genome) -> Self {
        Self {
            id: id.into(),
            generation: 0,
            parent: None,
            rating: 1000,
            record: Record::default(),
            personality: Personality::default(),
            genome,
        }
    }

    pub fn child(&self, id: impl Into<String>, seed: u64) -> Self {
        Self {
            id: id.into(),
            generation: self.generation + 1,
            parent: Some(self.id.clone()),
            rating: 1000,
            record: Record::default(),
            personality: Personality::default(),
            genome: mutate(&self.genome, seed),
        }
    }
}

/// Pick an action without mutating the game. Equal scores preserve canonical
/// legal-action order, so identical stamps, positions and seeds are repeatable.
pub fn choose_action(game: &Game, genome: &Genome) -> Option<ActionIntent> {
    if game.is_over() {
        return None;
    }
    let player = game.turn().player;
    let mut actions = game.ai_legal_actions();
    let pass = ActionIntent::EndTurn { player };
    if game.pending_promotion().is_none()
        && !actions.contains(&pass)
        && game.validate(&pass).is_ok()
    {
        actions.push(pass);
    }
    let mut best = None;
    let mut best_score = i128::MIN;
    for action in actions {
        let mut next = game.simulation();
        if next.apply(action.clone()).is_err() {
            continue;
        }
        let position = i128::from(heuristic::evaluate(&next, player));
        let score = position * i128::from(genome.evaluation)
            + i128::from(action_bias(&action, genome)) * 100;
        if score > best_score {
            best_score = score;
            best = Some(action);
        }
    }
    best
}

fn action_bias(action: &ActionIntent, genome: &Genome) -> i32 {
    match action {
        ActionIntent::Piece {
            kind: ActionKind::Take,
            ..
        } => genome.capture,
        ActionIntent::Piece {
            kind: ActionKind::Attack,
            ..
        } => genome.attack,
        ActionIntent::Piece {
            kind: ActionKind::Move,
            ..
        } => genome.movement,
        ActionIntent::Piece {
            kind: ActionKind::Ability,
            ..
        } => genome.ability,
        ActionIntent::Piece {
            kind: ActionKind::PlayCard,
            ..
        } => genome.card,
        ActionIntent::ScheduledAbility { .. }
        | ActionIntent::Transport { .. }
        | ActionIntent::Ability { .. } => genome.ability,
        ActionIntent::PlayCard { .. } => genome.card,
        ActionIntent::EndTurn { .. } => genome.patience,
        ActionIntent::ChoosePromotion { .. } => genome.capture,
    }
}

/// Deterministic bounded mutation. Keeping genes compact prevents a lucky
/// lineage from escaping meaningful selection through integer extremes.
pub fn mutate(parent: &Genome, seed: u64) -> Genome {
    let mut rng = SplitMix64(seed);
    let mut change = |value: i32, radius: i32| {
        let span = (radius * 2 + 1) as u64;
        (value + rng.below(span) as i32 - radius).clamp(-500, 500)
    };
    Genome {
        evaluation: change(parent.evaluation, 12).clamp(10, 250),
        capture: change(parent.capture, 35),
        attack: change(parent.attack, 35),
        movement: change(parent.movement, 35),
        ability: change(parent.ability, 35),
        card: change(parent.card, 35),
        patience: change(parent.patience, 35),
    }
}

#[derive(Clone, Copy)]
struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActionTally {
    pub total: u32,
    pub aggressive: u32,
    pub moves: u32,
    pub creative: u32,
    pub passes: u32,
}

impl ActionTally {
    pub fn observe(&mut self, action: &ActionIntent) {
        self.total += 1;
        match action {
            ActionIntent::Piece {
                kind: ActionKind::Take | ActionKind::Attack,
                ..
            } => self.aggressive += 1,
            ActionIntent::Piece {
                kind: ActionKind::Move,
                ..
            } => self.moves += 1,
            ActionIntent::Piece {
                kind: ActionKind::Ability | ActionKind::PlayCard,
                ..
            }
            | ActionIntent::ScheduledAbility { .. }
            | ActionIntent::Transport { .. }
            | ActionIntent::Ability { .. }
            | ActionIntent::PlayCard { .. } => self.creative += 1,
            ActionIntent::EndTurn { .. } => self.passes += 1,
            ActionIntent::ChoosePromotion { .. } => self.creative += 1,
        }
    }

    pub fn personality(self) -> Personality {
        let rate = |n| ((u64::from(n) * 1000) / u64::from(self.total.max(1))) as u16;
        Personality {
            aggression: rate(self.aggressive),
            mobility: rate(self.moves),
            creativity: rate(self.creative),
            patience: rate(self.passes),
        }
    }
}

/// Symmetric Elo update with a deliberately small, integer-only approximation.
/// `score_a` is 1000 for a win, 500 for a draw, and 0 for a loss.
pub fn update_ratings(a: &mut Stamp, b: &mut Stamp, score_a: i32) {
    let expected_a = 500 + (a.rating - b.rating).clamp(-400, 400);
    let delta = 24 * (score_a.clamp(0, 1000) - expected_a) / 1000;
    a.rating += delta;
    b.rating -= delta;
    match score_a {
        1000 => {
            a.record.wins += 1;
            b.record.losses += 1;
        }
        0 => {
            a.record.losses += 1;
            b.record.wins += 1;
        }
        _ => {
            a.record.draws += 1;
            b.record.draws += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cchess_core::{ai::AiPolicy, state::Snapshot, Registry};

    #[test]
    fn mutation_is_repeatable_and_bounded() {
        let a = mutate(&Genome::default(), 77);
        assert_eq!(a, mutate(&Genome::default(), 77));
        assert_ne!(a, mutate(&Genome::default(), 78));
        assert!((10..=250).contains(&a.evaluation));
    }

    #[test]
    fn stamps_round_trip() {
        let stamp = Stamp::founder("patient", Genome::default()).child("patient-g1", 4);
        let text = ron::to_string(&stamp).unwrap();
        assert_eq!(ron::from_str::<Stamp>(&text).unwrap(), stamp);
    }

    #[test]
    fn evolved_controller_is_playable_and_snapshot_self_contained() {
        let genome = mutate(&Genome::default(), 91);
        let game = Game::classical(Registry::built_in().unwrap())
            .unwrap()
            .with_evolved_ai(vec![(0, genome.clone())], 7)
            .unwrap();
        let controller = game.ai_controller(0).unwrap();
        assert_eq!(controller.policy, AiPolicy::Evolved);
        assert_eq!(controller.genome.as_ref(), Some(&genome));
        assert!(game.choose_ai_action().is_some());
        let encoded = ron::to_string(&game.snapshot()).unwrap();
        let restored: Snapshot = ron::from_str(&encoded).unwrap();
        assert_eq!(restored, game.snapshot());
    }
}
