use cchess_ai::{
    mcts::{search, SearchConfig},
    AiPolicy, GameAi,
};
use cchess_core::{
    state::{Placement, Player},
    Bounds, Direction, Game, Pos, Registry,
};

fn position(pieces: &[(&str, u8, i32, i32)]) -> Game {
    Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        vec![Player::new(7, Direction::N), Player::new(21, Direction::S)],
        pieces
            .iter()
            .map(|&(name, owner, x, y)| Placement::new(name, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
    .with_ai(vec![(7, AiPolicy::Mcts), (21, AiPolicy::Mcts)], 91)
    .unwrap()
}

#[test]
fn immediately_winning_capture_is_selected_and_terminal_search_stops() {
    let mut g = position(&[
        ("king", 7, 7, 0),
        ("rook", 7, 3, 3),
        ("queen", 21, 0, 3),
        ("king", 21, 3, 6),
    ]);
    g.play_ai_step().unwrap().unwrap();
    assert_eq!(g.winner(), Some(7));
    assert!(search(&g, SearchConfig::default()).action.is_none());
}

#[test]
fn seeded_search_is_pure_replayable_and_obeys_work_budget() {
    let mut g = position(&[("rook", 7, 0, 0), ("pawn", 21, 0, 4), ("pawn", 21, 7, 6)]);
    let original = g.clone();
    let config = SearchConfig {
        iterations: 64,
        rollout_depth: 4,
    };
    let a = search(&g, config);
    let b = search(&g, config);
    assert_eq!(a.action, b.action);
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.rollout_actions, b.rollout_actions);
    assert_eq!(a.iterations, 64);
    assert!(a.nodes <= 65);
    assert!(a.rollout_actions <= 64 * 4);
    assert!(
        a.max_tree_depth > 1,
        "must revisit children, not just sample root actions"
    );
    assert_eq!(a.failed_actions, 0);
    assert_eq!(g, original);
    let action = a.action.unwrap();
    let mut replay = original;
    assert_eq!(
        g.apply(action.clone()).unwrap(),
        replay.apply(action).unwrap()
    );
    assert_eq!(g, replay);
}

#[test]
fn promotion_and_immobile_positions_produce_valid_actions() {
    for pieces in [
        vec![("pawn", 7, 0, 7), ("pawn", 21, 7, 6)],
        vec![("wall", 7, 0, 0), ("pawn", 21, 7, 6)],
    ] {
        let mut g = position(&pieces);
        let action = search(
            &g,
            SearchConfig {
                iterations: 16,
                rollout_depth: 2,
            },
        )
        .action
        .unwrap();
        g.apply(action).unwrap();
        assert!(g.pending_promotion().is_none());
    }
}

#[test]
fn zero_budget_does_no_work() {
    let g = position(&[("rook", 7, 0, 0), ("pawn", 21, 7, 6)]);
    let result = search(
        &g,
        SearchConfig {
            iterations: 0,
            rollout_depth: 8,
        },
    );
    assert!(result.action.is_none());
    assert_eq!(result.nodes, 0);
    assert_eq!(result.rollout_actions, 0);
}

#[test]
fn four_player_and_extra_action_turns_use_actual_actor() {
    let base = Game::four_player(Registry::built_in().unwrap()).unwrap();
    let seats = base
        .players()
        .iter()
        .map(|p| (p.id, AiPolicy::Mcts))
        .collect();
    let mut g = base.with_ai(seats, 22).unwrap();
    for _ in 0..4 {
        let result = search(
            &g,
            SearchConfig {
                iterations: 12,
                rollout_depth: 3,
            },
        );
        assert_eq!(result.failed_actions, 0);
        g.apply(result.action.unwrap()).unwrap();
    }
    let mut players = vec![Player::new(7, Direction::N), Player::new(21, Direction::S)];
    players[0].actions_per_turn = 3;
    let mut g = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        players,
        vec![
            Placement::new("rook", 7, Pos::new(0, 0)),
            Placement::new("pawn", 7, Pos::new(4, 1)),
            Placement::new("pawn", 21, Pos::new(7, 6)),
        ],
    )
    .unwrap();
    let result = search(
        &g,
        SearchConfig {
            iterations: 32,
            rollout_depth: 4,
        },
    );
    assert_eq!(result.failed_actions, 0);
    g.apply(result.action.unwrap()).unwrap();
}
