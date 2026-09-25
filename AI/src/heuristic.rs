//! One-action lookahead with hand-tuned scores (a pawn is approximately 100).
//! This is not a hidden-information search: simulations use the engine's actual
//! state/RNG. Opponent card identities and deck order are not directly scored.
use crate::GameAi;
use cchess_core::{
    content::Tag,
    state::{PieceInstance, PlayerId, StatusKind},
    ActionIntent, ActionKind, Direction, Game,
};
use std::collections::BTreeSet;

const WIN: i64 = 1_000_000_000;

/// Choose the highest-scoring successfully resolved action, preserving generator
/// order on ties. Only this policy considers an early, legal end of turn.
/// The live game (including its history and random stream) is never modified.
pub fn choose_action(game: &Game) -> Option<ActionIntent> {
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
    let mut best_score = i64::MIN;
    for action in actions {
        let mut next = game.clone();
        if next.apply(action.clone()).is_err() {
            continue;
        }
        let score = evaluate(&next, player);
        if score > best_score {
            best_score = score;
            best = Some(action);
        }
    }
    best
}

/// Evaluate from one seat's perspective. In FFA, all opponents contribute
/// negatively; this is a simple material objective, not a coalition model.
/// Terminal outcomes dominate every nonterminal score.
pub fn evaluate(game: &Game, player: PlayerId) -> i64 {
    if let Some(winner) = game.winner() {
        return if winner == player { WIN } else { -WIN };
    }
    if game.is_eliminated(player) || game.player(player).is_none() {
        return -WIN;
    }
    if game.is_draw() {
        return 0;
    }
    let mut score = 0;
    for piece in game.pieces() {
        let sign = if piece.owner == player { 1 } else { -1 };
        score += sign * (piece_value(game, piece) + position_value(game, piece));
    }
    for seat in game.players().iter().filter(|p| !game.is_eliminated(p.id)) {
        let sign = if seat.id == player { 1 } else { -1 };
        // Cap mana so stockpiles cannot outweigh survival or material indefinitely.
        let mana = i64::from(seat.mana.as_u32().unwrap_or(u32::MAX).min(100));
        score += sign
            * (mana * 5
                + seat.cards.hand.len() as i64 * 20
                + seat.cards.on_board.len() as i64 * 35);
    }
    // Legal ordinary captures by the current actor are immediate pressure.
    // Deduplicate targets; multiple attackers do not multiply a victim's value.
    // This intentionally does not estimate future turns or ability combinations.
    let mut threatened = BTreeSet::new();
    for attacker in game.pieces().filter(|p| p.owner == game.turn().player) {
        for kind in [ActionKind::Take, ActionKind::Attack] {
            for action in game.legal_piece_actions(attacker.id, kind) {
                if let ActionIntent::Piece { target, .. } = action {
                    if let Some(victim) = game.at(target) {
                        if victim.owner != attacker.owner {
                            threatened.insert(victim.id);
                        }
                    }
                }
            }
        }
    }
    for id in threatened {
        let victim = game.piece(id).unwrap();
        let penalty = piece_value(game, victim) * 3 / 4;
        // Enemy-on-enemy pressure is not credited to this seat in FFA.
        if victim.owner == player {
            score -= penalty;
        } else if game.turn().player == player {
            score += penalty;
        }
    }
    score.clamp(-WIN + 1, WIN - 1)
}

fn piece_value(game: &Game, piece: &PieceInstance) -> i64 {
    let base = if game.has_tag(piece.id, Tag::Royal) {
        20_000
    } else {
        match piece.archetype.as_str() {
            "pawn" | "crazy_pawn" => 100,
            "knight" => 320,
            "bishop" => 330,
            "rook" => 500,
            "queen" => 900,
            "wall" => 80,
            // Custom content gets a baseline plus combat stats and abilities.
            _ => 250,
        }
    };
    let definition = &game.registry().get(&piece.archetype).unwrap().definition;
    let mut value = base
        + i64::from(piece.stats.tough.saturating_sub(1).min(100)) * 80
        + i64::from(piece.stats.strength.min(100)) * 20
        + i64::from(piece.stats.impenetrable.min(100)) * 30
        + definition.abilities.len() as i64 * 15;
    let health = piece.stats.tough.saturating_sub(piece.state.damage);
    value = value * i64::from(health) / i64::from(piece.stats.tough.max(1));
    if game.has_tag(piece.id, Tag::Immune) {
        value += 40;
    }
    if game.has_status(piece.id, StatusKind::Invulnerable) {
        value += 60;
    }
    if game.has_status(piece.id, StatusKind::Ice)
        || game.has_status(piece.id, StatusKind::Deactivate)
    {
        value -= value / 4;
    }
    if game.has_status(piece.id, StatusKind::Fire) {
        value -= value / 3;
    }
    value
}

fn position_value(game: &Game, piece: &PieceInstance) -> i64 {
    if game.has_tag(piece.id, Tag::Royal) || game.has_tag(piece.id, Tag::Structure) {
        return 0;
    }
    let bounds = game.bounds();
    let x = i64::from(piece.position.x);
    let y = i64::from(piece.position.y);
    let width = i64::from(bounds.width) - 1;
    let height = i64::from(bounds.height) - 1;
    let mut value = -(2 * x - width).abs() - (2 * y - height).abs();
    if game
        .registry()
        .get(&piece.archetype)
        .unwrap()
        .definition
        .promotion
        .is_some()
    {
        let advance = match game.player(piece.owner).unwrap().forward {
            Direction::N => y,
            Direction::S => height - y,
            Direction::E => x,
            Direction::W => width - x,
        };
        value += advance * 6;
    }
    value
}
