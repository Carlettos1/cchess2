use cchess_ai::{heuristic, AiPolicy, GameAi};
use cchess_core::{
    state::{Placement, Player},
    ActionIntent, ActionKind, Bounds, Direction, Game, Pos, Registry,
};

fn game(pieces: &[(&str, u8, i32, i32)]) -> Game {
    Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
        pieces
            .iter()
            .map(|&(name, owner, x, y)| Placement::new(name, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
    .with_ai(vec![(0, AiPolicy::Heuristic)], 42)
    .unwrap()
}

#[test]
fn prefers_valuable_capture_over_first_target() {
    let g = game(&[("rook", 0, 3, 3), ("pawn", 1, 0, 3), ("queen", 1, 3, 6)]);
    assert!(matches!(
        g.choose_ai_action(),
        Some(ActionIntent::Piece {
            kind: ActionKind::Take,
            target: Pos { x: 3, y: 6 },
            ..
        })
    ));
}

#[test]
fn king_capture_dominates_material() {
    let mut g = game(&[
        ("king", 0, 7, 0),
        ("rook", 0, 3, 3),
        ("queen", 1, 0, 3),
        ("king", 1, 3, 6),
    ]);
    g.play_ai_step().unwrap().unwrap();
    assert_eq!(g.winner(), Some(0));
    assert_eq!(heuristic::evaluate(&g, 0), -heuristic::evaluate(&g, 1));
    assert!(g.choose_ai_action().is_none());
}

#[test]
fn avoids_a_pawn_capture_that_exposes_queen_to_rook() {
    let g = game(&[("queen", 0, 3, 3), ("pawn", 1, 3, 5), ("rook", 1, 7, 5)]);
    let action = g.choose_ai_action().unwrap();
    assert!(!matches!(
        action,
        ActionIntent::Piece {
            target: Pos { x: 3, y: 5 },
            ..
        }
    ));
}

#[test]
fn promotion_selects_queen() {
    let mut g = game(&[("pawn", 0, 0, 7), ("pawn", 1, 7, 6)]);
    assert!(
        matches!(g.choose_ai_action(), Some(ActionIntent::ChoosePromotion { archetype, .. }) if archetype == "queen")
    );
    g.play_ai_step().unwrap().unwrap();
    assert!(g.pending_promotion().is_none());
    assert_eq!(g.at(Pos::new(0, 7)).unwrap().archetype, "queen");
}

#[test]
fn previews_are_pure_deterministic_and_replayable() {
    let mut g = game(&[("rook", 0, 0, 0), ("pawn", 1, 0, 4)]);
    let before = g.clone();
    let action = g.choose_ai_action().unwrap();
    assert_eq!(g.choose_ai_action(), Some(action.clone()));
    assert_eq!(g, before);
    let mut replay = before;
    assert_eq!(
        g.play_ai_step().unwrap().unwrap(),
        replay.apply(action).unwrap()
    );
    assert_eq!(g, replay);
    assert!(g.choose_ai_action().is_none()); // Next seat is human.
}

#[test]
fn immobile_position_passes() {
    let mut g = game(&[("wall", 0, 0, 0), ("pawn", 1, 7, 6)]);
    assert_eq!(
        g.choose_ai_action(),
        Some(ActionIntent::EndTurn { player: 0 })
    );
    g.play_ai_step().unwrap();
    assert_eq!(g.turn().player, 1);
}

#[test]
fn two_and_four_player_presets_produce_legal_repeatable_actions() {
    for four in [false, true] {
        let registry = Registry::built_in().unwrap();
        let base = if four {
            Game::four_player(registry)
        } else {
            Game::classical(registry)
        }
        .unwrap();
        let seats = base
            .players()
            .iter()
            .map(|p| (p.id, AiPolicy::Heuristic))
            .collect();
        let mut g = base.with_ai(seats, 7).unwrap();
        for _ in 0..4 {
            let before = g.clone();
            let action = g.choose_ai_action().unwrap();
            assert_eq!(g.choose_ai_action(), Some(action.clone()));
            assert_eq!(g, before);
            g.apply(action).unwrap();
        }
    }
}
