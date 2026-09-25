use cchess_engine::{
    content::{
        ActionCost, ActionRule, Condition, EffectDef, PieceArchetype, Tag, CLASSICAL_SOURCES,
    },
    geometry::{Pattern, Relative},
    state::{Placement, Player, Snapshot},
    targeting::{self, TargetFilter, TargetOrigin, TargetSpec},
    *,
};
use std::collections::BTreeSet;

fn registry_with(edit: impl FnOnce(&mut Vec<PieceArchetype>)) -> Registry {
    let mut definitions: Vec<PieceArchetype> = CLASSICAL_SOURCES
        .iter()
        .map(|(_, text)| ron::from_str(text).unwrap())
        .collect();
    edit(&mut definitions);
    let sources: Vec<_> = definitions
        .iter()
        .map(|d| (format!("{}.ron", d.id), ron::to_string(d).unwrap()))
        .collect();
    Registry::load(sources.iter().map(|(n, s)| (n.as_str(), s.as_str()))).unwrap()
}
fn scenario(registry: Registry, pieces: &[(&str, u8, i32, i32)]) -> Game {
    Game::new(
        registry,
        Bounds {
            width: 8,
            height: 8,
        },
        vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
        pieces
            .iter()
            .map(|&(id, owner, x, y)| Placement::new(id, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
}
fn act(game: &Game, from: (i32, i32), kind: ActionKind, to: (i32, i32)) -> ActionIntent {
    ActionIntent::Piece {
        player: game.turn().player,
        source: game.at(Pos::new(from.0, from.1)).unwrap().id,
        kind,
        target: Pos::new(to.0, to.1),
    }
}
fn pass(game: &mut Game) {
    game.apply(ActionIntent::EndTurn {
        player: game.turn().player,
    })
    .unwrap();
}
fn assert_rejected(game: &mut Game, intent: ActionIntent, error: RuleError) {
    let before = game.clone();
    assert_eq!(game.apply(intent), Err(error));
    assert_eq!(*game, before);
}

#[test]
fn six_classical_assets_load_and_starting_position_has_twenty_moves() {
    let registry = Registry::classical().unwrap();
    assert_eq!(registry.iter().count(), 6);
    let game = Game::classical(registry).unwrap();
    assert_eq!(game.pieces().count(), 32);
    assert_eq!(
        game.pieces().map(|p| p.id).collect::<BTreeSet<_>>().len(),
        32
    );
    let moves: Vec<_> = game
        .pieces()
        .flat_map(|p| game.legal_actions(p.id))
        .filter(|a| matches!(a, ActionIntent::Piece { .. }))
        .collect();
    assert_eq!(moves.len(), 20);
    for action in moves {
        let mut branch = game.clone();
        branch.apply(action).unwrap();
        assert_eq!(branch.turn().player, 1);
    }
}
#[test]
fn arbitrary_archetype_ids_work_without_rust_variants() {
    let registry = registry_with(|d| {
        let mut p = d[1].clone();
        p.id = "custom_knight".into();
        d.push(p);
    });
    let game = scenario(registry, &[("custom_knight", 0, 3, 3)]);
    assert_eq!(
        game.legal_actions(game.at(Pos::new(3, 3)).unwrap().id)
            .iter()
            .filter(|a| matches!(a, ActionIntent::Piece { .. }))
            .count(),
        8
    );
}
#[test]
fn pawn_can_double_later_but_cannot_jump_or_capture_forward() {
    let mut game = scenario(Registry::classical().unwrap(), &[("pawn", 0, 3, 1)]);
    game.apply(act(&game, (3, 1), ActionKind::Move, (3, 3)))
        .unwrap();
    pass(&mut game);
    game.apply(act(&game, (3, 3), ActionKind::Move, (3, 5)))
        .unwrap();
    let game = scenario(
        Registry::classical().unwrap(),
        &[("pawn", 0, 3, 1), ("pawn", 1, 3, 2)],
    );
    assert_eq!(
        game.validate(&act(&game, (3, 1), ActionKind::Move, (3, 3))),
        Err(RuleError::InvalidTarget)
    );
    assert_eq!(
        game.validate(&act(&game, (3, 1), ActionKind::Take, (3, 2))),
        Err(RuleError::InvalidTarget)
    );
}
#[test]
fn black_pawn_orientation_and_diagonal_capture() {
    let mut game = scenario(
        Registry::classical().unwrap(),
        &[("pawn", 0, 2, 4), ("pawn", 1, 3, 5)],
    );
    pass(&mut game);
    game.apply(act(&game, (3, 5), ActionKind::Take, (2, 4)))
        .unwrap();
    assert_eq!(game.at(Pos::new(2, 4)).unwrap().owner, 1);
    assert_eq!(game.dead().len(), 1);
}
#[test]
fn east_and_west_forward_work_in_piece_rules() {
    for (forward, target) in [
        (Direction::E, Pos::new(5, 3)),
        (Direction::W, Pos::new(1, 3)),
    ] {
        let mut game = Game::new(
            Registry::classical().unwrap(),
            Bounds {
                width: 8,
                height: 8,
            },
            vec![Player::new(0, forward), Player::new(1, Direction::N)],
            vec![Placement::new("pawn", 0, Pos::new(3, 3))],
        )
        .unwrap();
        let source = game.at(Pos::new(3, 3)).unwrap().id;
        game.apply(ActionIntent::Piece {
            player: 0,
            source,
            kind: ActionKind::Move,
            target,
        })
        .unwrap();
    }
}
#[test]
fn ownership_invalid_targets_and_missing_sources_are_atomic() {
    let mut game = Game::classical(Registry::classical().unwrap()).unwrap();
    let mut intent = act(&game, (0, 1), ActionKind::Move, (0, 2));
    if let ActionIntent::Piece { player, .. } = &mut intent {
        *player = 1;
    }
    assert_rejected(
        &mut game,
        intent,
        RuleError::WrongPlayer {
            expected: 0,
            actual: 1,
        },
    );
    let intent = act(&game, (0, 6), ActionKind::Move, (0, 5));
    assert_rejected(&mut game, intent, RuleError::NotOwner);
    let intent = act(&game, (0, 1), ActionKind::Move, (-1, 2));
    assert_rejected(&mut game, intent, RuleError::OutOfBounds(Pos::new(-1, 2)));
    let intent = act(&game, (0, 1), ActionKind::Move, (0, 1));
    assert_rejected(&mut game, intent, RuleError::SelfTarget);
    let intent = act(&game, (0, 0), ActionKind::Take, (0, 1));
    assert_rejected(&mut game, intent, RuleError::InvalidTarget);
    assert_rejected(
        &mut game,
        ActionIntent::Piece {
            player: 0,
            source: 999,
            kind: ActionKind::Move,
            target: Pos::new(0, 2),
        },
        RuleError::SourceMissing(999),
    );
    let intent = act(&game, (0, 1), ActionKind::Attack, (0, 6));
    assert_rejected(&mut game, intent, RuleError::UnsupportedAction);
}
#[test]
fn tough_survivor_prevents_capture_movement_and_later_damage_kills() {
    let registry = registry_with(|d| {
        d.iter_mut()
            .find(|p| p.id == "pawn")
            .unwrap()
            .base_stats
            .tough = 2
    });
    let mut game = scenario(registry, &[("rook", 0, 0, 0), ("pawn", 1, 0, 3)]);
    let source = game.at(Pos::new(0, 0)).unwrap().id;
    let target = game.at(Pos::new(0, 3)).unwrap().id;
    let first = game
        .apply(act(&game, (0, 0), ActionKind::Take, (0, 3)))
        .unwrap();
    assert_eq!(game.piece(source).unwrap().position, Pos::new(0, 0));
    assert_eq!(game.piece(target).unwrap().state.damage, 1);
    assert!(!first
        .events
        .iter()
        .any(|e| matches!(e, Event::PieceMoved { .. } | Event::PieceRemoved { .. })));
    pass(&mut game);
    let second = game
        .apply(act(&game, (0, 0), ActionKind::Take, (0, 3)))
        .unwrap();
    assert_eq!(game.piece(source).unwrap().position, Pos::new(0, 3));
    assert!(game.piece(target).is_none());
    let removed = second
        .events
        .iter()
        .position(|e| matches!(e, Event::PieceRemoved { .. }))
        .unwrap();
    let moved = second
        .events
        .iter()
        .position(|e| matches!(e, Event::PieceMoved { .. }))
        .unwrap();
    assert!(removed < moved);
    assert_eq!(
        game.dead()[0].source,
        cchess_engine::state::ActionSource::Piece(source)
    );
    assert_eq!(game.dead()[0].piece.id, target);
}
fn add_attack(d: &mut [PieceArchetype]) {
    let rook = d.iter_mut().find(|p| p.id == "rook").unwrap();
    rook.action_rules.push(ActionRule {
        kind: ActionKind::Attack,
        target: TargetSpec {
            origin: TargetOrigin::Source,
            pattern: Pattern::AreaSquare(7),
            filters: vec![TargetFilter::Enemy, TargetFilter::Occupied],
            count: 1,
        },
        conditions: vec![],
        cost: ActionCost::default(),
        effects: vec![EffectDef::DamageTarget(1)],
    });
}
#[test]
fn attack_damages_without_moving_and_heroic_rejects_it_but_not_take() {
    let registry = registry_with(|d| add_attack(d));
    let mut game = scenario(registry.clone(), &[("rook", 0, 0, 0), ("pawn", 1, 0, 3)]);
    game.apply(act(&game, (0, 0), ActionKind::Attack, (0, 3)))
        .unwrap();
    assert!(game.at(Pos::new(0, 0)).is_some());
    assert!(game.at(Pos::new(0, 3)).is_none());
    let mut game = scenario(registry, &[("rook", 0, 0, 0), ("queen", 1, 0, 3)]);
    let intent = act(&game, (0, 0), ActionKind::Attack, (0, 3));
    assert_rejected(&mut game, intent, RuleError::ProtectedTarget);
    game.apply(act(&game, (0, 0), ActionKind::Take, (0, 3)))
        .unwrap();
}
#[test]
fn immune_does_not_block_take_or_attack() {
    let registry = registry_with(|d| {
        add_attack(d);
        d.iter_mut()
            .find(|p| p.id == "pawn")
            .unwrap()
            .tags
            .insert(Tag::Immune);
    });
    for kind in [ActionKind::Take, ActionKind::Attack] {
        let mut game = scenario(registry.clone(), &[("rook", 0, 0, 0), ("pawn", 1, 0, 3)]);
        game.apply(act(&game, (0, 0), kind, (0, 3))).unwrap();
        assert_eq!(game.dead().len(), 1);
    }
}
#[test]
fn costs_are_checked_then_deducted_once() {
    let registry = registry_with(|d| {
        d.iter_mut().find(|p| p.id == "rook").unwrap().action_rules[0].cost = ActionCost {
            actions: 2,
            mana: 3,
        }
    });
    let mut player = Player::new(0, Direction::N);
    player.actions_per_turn = 3;
    player.mana = 3.into();
    let mut game = Game::new(
        registry.clone(),
        Bounds {
            width: 8,
            height: 8,
        },
        vec![player, Player::new(1, Direction::S)],
        vec![Placement::new("rook", 0, Pos::new(0, 0))],
    )
    .unwrap();
    game.apply(act(&game, (0, 0), ActionKind::Move, (0, 1)))
        .unwrap();
    assert_eq!(game.turn().remaining_actions, 1);
    assert_eq!(game.player(0).unwrap().mana, 0);
    let mut game = scenario(registry.clone(), &[("rook", 0, 0, 0)]);
    let intent = act(&game, (0, 0), ActionKind::Move, (0, 1));
    assert_rejected(
        &mut game,
        intent,
        RuleError::InsufficientActions {
            required: 2,
            available: 1,
        },
    );
    let mut player = Player::new(0, Direction::N);
    player.actions_per_turn = 2;
    let mut game = Game::new(
        registry,
        Bounds {
            width: 8,
            height: 8,
        },
        vec![player, Player::new(1, Direction::S)],
        vec![Placement::new("rook", 0, Pos::new(0, 0))],
    )
    .unwrap();
    let intent = act(&game, (0, 0), ActionKind::Move, (0, 1));
    assert_rejected(
        &mut game,
        intent,
        RuleError::InsufficientMana {
            required: 3,
            available: 1,
        },
    );
}
#[test]
fn acted_state_resets_only_at_start_of_owners_next_turn() {
    let mut p = Player::new(0, Direction::N);
    p.actions_per_turn = 2;
    let mut game = Game::new(
        Registry::classical().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        vec![p, Player::new(1, Direction::S)],
        vec![Placement::new("rook", 0, Pos::new(0, 0))],
    )
    .unwrap();
    game.apply(act(&game, (0, 0), ActionKind::Move, (0, 1)))
        .unwrap();
    assert_eq!(game.turn().player, 0);
    let intent = act(&game, (0, 1), ActionKind::Move, (0, 2));
    assert_rejected(&mut game, intent, RuleError::AlreadyActed);
    assert!(game
        .legal_actions(game.at(Pos::new(0, 1)).unwrap().id)
        .is_empty());
    pass(&mut game);
    assert!(game.at(Pos::new(0, 1)).unwrap().state.acted);
    pass(&mut game);
    assert!(!game.at(Pos::new(0, 1)).unwrap().state.acted);
    assert_eq!(game.turn().remaining_actions, 2);
}
#[test]
fn composite_condition_failure_is_structured_and_atomic() {
    let condition = Condition::And(vec![
        Condition::HasNotActed,
        Condition::Or(vec![
            Condition::HasTag(Tag::Demonic),
            Condition::Not(Box::new(Condition::HasTag(Tag::Structure))),
        ]),
    ]);
    let registry = registry_with(|d| {
        d.iter_mut().find(|p| p.id == "rook").unwrap().action_rules[0].conditions =
            vec![condition.clone()]
    });
    let mut game = scenario(registry, &[("rook", 0, 0, 0)]);
    let intent = act(&game, (0, 0), ActionKind::Move, (0, 1));
    assert_rejected(&mut game, intent, RuleError::ConditionFailed(condition));
}
#[test]
fn king_capture_finishes_match_and_preserves_final_capture_event_order() {
    let mut game = scenario(
        Registry::classical().unwrap(),
        &[("rook", 0, 0, 0), ("king", 0, 7, 0), ("king", 1, 0, 7)],
    );
    let action = game
        .apply(act(&game, (0, 0), ActionKind::Take, (0, 7)))
        .unwrap();
    assert_eq!(game.winner(), Some(0));
    assert_eq!(action.events.last(), Some(&Event::GameWon { player: 0 }));
    assert!(!action
        .events
        .iter()
        .any(|e| matches!(e, Event::TurnStarted { .. })));
    assert_rejected(
        &mut game,
        ActionIntent::EndTurn { player: 0 },
        RuleError::GameOver,
    );
    assert!(game.pieces().all(|p| game.legal_actions(p.id).is_empty()));
}
#[test]
fn friendly_origin_targeting_deduplicates_and_filters_empty_neighbors() {
    let game = scenario(
        Registry::classical().unwrap(),
        &[("pawn", 0, 2, 2), ("pawn", 0, 3, 2), ("pawn", 1, 4, 2)],
    );
    let spec = TargetSpec {
        origin: TargetOrigin::AnyFriendlyPiece,
        pattern: Pattern::FullStep.normalize().unwrap(),
        filters: vec![TargetFilter::Empty],
        count: 1,
    };
    let result = targeting::resolve(&game, 0, 0, &spec);
    let expected: BTreeSet<_> = [Pos::new(2, 2), Pos::new(3, 2)]
        .into_iter()
        .flat_map(|p| {
            Pattern::FullStep
                .normalize()
                .unwrap()
                .targets(game.bounds(), p, Direction::N, |_| false)
        })
        .filter(|p| game.at(*p).is_none())
        .collect();
    assert_eq!(result, expected.into_iter().collect::<Vec<_>>());
    assert_eq!(result.len(), 9);
}
#[test]
fn replay_is_deterministic_and_long_sequences_preserve_state_invariants() {
    let initial = Game::classical(Registry::classical().unwrap()).unwrap();
    let mut first = initial.clone();
    let mut seed = 42_u64;
    for _ in 0..200 {
        if first.winner().is_some() {
            break;
        }
        let legal: Vec<_> = first
            .pieces()
            .flat_map(|p| first.legal_actions(p.id))
            .collect();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let intent = if legal.is_empty() {
            ActionIntent::EndTurn {
                player: first.turn().player,
            }
        } else {
            legal[(seed as usize) % legal.len()].clone()
        };
        first.apply(intent).unwrap();
        let positions: BTreeSet<_> = first.pieces().map(|p| p.position).collect();
        assert_eq!(positions.len(), first.pieces().count());
        assert!(first
            .pieces()
            .all(|p| first.bounds().contains(p.position) && p.state.damage < p.stats.tough));
    }
    let mut replay = initial;
    for entry in first.log() {
        assert_eq!(&replay.apply(entry.intent.clone()).unwrap(), entry);
    }
    assert_eq!(first, replay);
    let encoded = ron::to_string(&first.snapshot()).unwrap();
    assert_eq!(
        ron::from_str::<Snapshot>(&encoded).unwrap(),
        first.snapshot()
    );
    assert_eq!(
        ron::to_string(first.log()).unwrap(),
        ron::to_string(replay.log()).unwrap()
    );
}
#[test]
fn invalid_setup_cannot_create_overlapping_or_unowned_pieces() {
    let registry = Registry::classical().unwrap();
    let players = vec![Player::new(0, Direction::N), Player::new(1, Direction::S)];
    for placements in [
        vec![Placement::new("missing", 0, Pos::new(0, 0))],
        vec![Placement::new("pawn", 2, Pos::new(0, 0))],
        vec![Placement::new("pawn", 0, Pos::new(8, 0))],
        vec![
            Placement::new("pawn", 0, Pos::new(0, 0)),
            Placement::new("king", 1, Pos::new(0, 0)),
        ],
    ] {
        assert!(Game::new(
            registry.clone(),
            Bounds {
                width: 8,
                height: 8
            },
            players.clone(),
            placements
        )
        .is_err());
    }
}
#[test]
fn target_filters_apply_together() {
    let game = scenario(
        Registry::classical().unwrap(),
        &[("pawn", 0, 2, 2), ("pawn", 1, 3, 3), ("king", 1, 1, 3)],
    );
    let source = game.at(Pos::new(2, 2)).unwrap().id;
    let spec = TargetSpec {
        origin: TargetOrigin::Source,
        pattern: Pattern::RelativeStep(vec![Relative::FL, Relative::FR])
            .normalize()
            .unwrap(),
        filters: vec![
            TargetFilter::Occupied,
            TargetFilter::Enemy,
            TargetFilter::HasTag(Tag::Royal),
            TargetFilter::Archetype("king".into()),
        ],
        count: 1,
    };
    assert_eq!(
        targeting::resolve(&game, source, 0, &spec),
        vec![Pos::new(1, 3)]
    );
}
