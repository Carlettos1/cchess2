use cchess_engine::GameAi;
mod support;
use cchess_engine::{
    ai::{AiController, AiPolicy},
    state::Snapshot,
    *,
};
use support::*;

fn first(pieces: &[(&str, u8, i32, i32)], cards: &[&str]) -> Game {
    game(pieces, cards, &["mana"], cards.len() as u32)
        .with_ai(vec![(0, AiPolicy::First)], 42)
        .unwrap()
}
#[test]
fn first_policy_checks_categories_in_requested_order() {
    let g = first(&[("rook", 0, 0, 0), ("pawn", 1, 0, 3)], &["mana"]);
    assert!(matches!(
        g.choose_ai_action(),
        Some(ActionIntent::PlayCard { .. })
    ));
    let g = first(&[("rook", 0, 0, 0), ("pawn", 1, 0, 3)], &["fire"]);
    // Fire is also playable and comes before the capture, regardless of utility.
    assert!(matches!(
        g.choose_ai_action(),
        Some(ActionIntent::PlayCard { .. })
    ));
    for (pieces, expected) in [
        (vec![("rook", 0, 0, 0), ("pawn", 1, 0, 3)], ActionKind::Take),
        (
            vec![("cannon", 0, 0, 0), ("bishop", 0, 7, 7), ("pawn", 1, 0, 3)],
            ActionKind::Attack,
        ),
        (vec![("pawn", 0, 3, 1)], ActionKind::Move),
    ] {
        let g = game(&pieces, &["mana"], &["mana"], 0)
            .with_ai(vec![(0, AiPolicy::First)], 0)
            .unwrap();
        assert!(
            matches!(g.choose_ai_action(),Some(ActionIntent::Piece{kind,..}) if kind==expected)
        );
    }
    let g = game(&[("bishop", 0, 3, 3)], &["mana"], &["mana"], 0)
        .with_ai(vec![(0, AiPolicy::First)], 0)
        .unwrap();
    assert!(matches!(
        g.choose_ai_action(),
        Some(ActionIntent::Ability { .. })
    ));
}
#[test]
fn ai_plays_zero_action_cards_extra_movements_and_passes_only_when_stuck() {
    let mut g = first(
        &[("rook", 0, 0, 0), ("pawn", 0, 3, 1), ("pawn", 1, 0, 3)],
        &["forced_march", "mana"],
    );
    let trace = g.play_ai_turn(100).unwrap();
    assert!(matches!(trace[0].intent, ActionIntent::PlayCard { .. }));
    assert!(matches!(trace[1].intent, ActionIntent::PlayCard { .. }));
    assert!(matches!(
        trace[2].intent,
        ActionIntent::Piece {
            kind: ActionKind::Take,
            ..
        }
    ));
    assert!(matches!(
        trace[3].intent,
        ActionIntent::Piece {
            kind: ActionKind::Move,
            ..
        }
    ));
    assert!(matches!(trace[4].intent, ActionIntent::EndTurn { .. }));
    assert_eq!(g.turn().player, 1);
    assert!(g.play_ai_step().unwrap().is_none());
}
#[test]
fn mandatory_promotion_is_resolved_before_other_actions() {
    for policy in [AiPolicy::First, AiPolicy::Random] {
        let mut g = game(&[("pawn", 0, 0, 7)], &["mana"], &["mana"], 1)
            .with_ai(vec![(0, policy)], 4)
            .unwrap();
        assert!(matches!(
            g.choose_ai_action(),
            Some(ActionIntent::ChoosePromotion { .. })
        ));
        g.play_ai_step().unwrap();
        assert!(g.pending_promotion().is_none());
    }
}
#[test]
fn random_choices_are_legal_seeded_serializable_and_do_not_pass_early() {
    let base = game(
        &[("rook", 0, 0, 0), ("pawn", 1, 0, 4)],
        &["mana"],
        &["mana"],
        1,
    );
    let mut choices = std::collections::BTreeSet::new();
    for seed in 0..30 {
        let mut g = base
            .clone()
            .with_ai(vec![(0, AiPolicy::Random)], seed)
            .unwrap();
        let before = g.snapshot();
        let action = g.choose_ai_action().unwrap();
        assert!(g.ai_legal_actions().contains(&action));
        assert!(!matches!(action, ActionIntent::EndTurn { .. }));
        assert_eq!(g.choose_ai_action(), Some(action.clone()));
        assert_eq!(g.snapshot(), before);
        choices.insert(format!("{action:?}"));
        let mut replay = g.clone();
        assert_eq!(
            g.play_ai_step().unwrap().unwrap(),
            replay.apply(action).unwrap()
        );
        assert_eq!(g.snapshot(), replay.snapshot());
        assert_eq!(
            ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
            g.snapshot()
        );
        let controller = g.ai_controller(0).unwrap();
        assert_eq!(
            ron::from_str::<AiController>(&ron::to_string(controller).unwrap()).unwrap(),
            *controller
        );
    }
    assert!(choices.len() > 1);
}
#[test]
fn turn_budget_pauses_without_forcing_pass_or_losing_actions() {
    let mut g = first(&[("pawn", 0, 3, 1)], &["forced_march"]);
    assert_eq!(g.play_ai_turn(1).unwrap().len(), 1);
    assert_eq!(g.turn().player, 0);
    assert_eq!(g.turn().remaining_actions, 3);
    assert!(!g.play_ai_turn(10).unwrap().is_empty());
    assert_eq!(g.turn().player, 1);
}
#[test]
fn empty_legal_set_passes_and_seat_configuration_is_validated() {
    let base = game(&[("wall", 0, 3, 3)], &["mana"], &["mana"], 0);
    let mut g = base
        .clone()
        .with_ai(vec![(0, AiPolicy::Random)], 0)
        .unwrap();
    assert_eq!(
        g.ai_legal_actions(),
        vec![ActionIntent::EndTurn { player: 0 }]
    );
    g.play_ai_step().unwrap();
    assert_eq!(g.turn().player, 1);
    assert!(base.clone().with_ai(vec![(2, AiPolicy::First)], 0).is_err());
    assert!(base
        .with_ai(vec![(0, AiPolicy::First), (0, AiPolicy::Random)], 0)
        .is_err());
}
#[test]
fn both_policies_play_reproducible_two_and_four_player_matches() {
    for four in [false, true] {
        let registry = Registry::built_in().unwrap();
        let game = if four {
            Game::four_player(registry)
        } else {
            Game::classical(registry)
        }
        .unwrap();
        let seats = game
            .players()
            .iter()
            .map(|p| {
                (
                    p.id,
                    if p.id % 2 == 0 {
                        AiPolicy::First
                    } else {
                        AiPolicy::Random
                    },
                )
            })
            .collect();
        let mut g = game.with_ai(seats, 987).unwrap();
        let mut replay = g.clone();
        for _ in 0..80 {
            let Some(action) = g.choose_ai_action() else {
                break;
            };
            assert_eq!(
                g.play_ai_step().unwrap().unwrap(),
                replay.apply(action).unwrap()
            );
            assert_eq!(g.snapshot(), replay.snapshot());
        }
    }
}

#[test]
fn heuristic_controller_roundtrips_and_can_pass_with_playable_actions() {
    // Spending this card gains only two mana; the heuristic values retaining
    // the card more highly. No movable piece provides a better alternative.
    let mut g = game(&[("wall", 0, 0, 0)], &["mana"], &["mana"], 1)
        .with_ai(vec![(0, AiPolicy::Heuristic)], 9)
        .unwrap();
    assert!(g
        .ai_legal_actions()
        .iter()
        .any(|a| matches!(a, ActionIntent::PlayCard { .. })));
    assert_eq!(
        g.choose_ai_action(),
        Some(ActionIntent::EndTurn { player: 0 })
    );
    let snapshot = g.snapshot();
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&snapshot).unwrap()).unwrap(),
        snapshot
    );
    let controller = g.ai_controller(0).unwrap();
    assert_eq!(
        ron::from_str::<AiController>(&ron::to_string(controller).unwrap()).unwrap(),
        *controller
    );
    g.play_ai_step().unwrap().unwrap();
    assert_eq!(g.turn().player, 1);
}
