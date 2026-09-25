mod support;
use cchess_engine::{
    cards::CardZone,
    clock::ClockUnit,
    content::{EffectDef, TriggerDef, TriggerEvent},
    state::{Placement, Player, Snapshot},
    *,
};
use support::{act, cast, deck, pass, registry};
fn id(g: &Game, x: i32, y: i32) -> u64 {
    support::id(g, x + 3, y + 3)
}
fn pos(x: i32, y: i32) -> Pos {
    Pos::new(x + 3, y + 3)
}

fn setup(reg: Registry, pieces: &[(&str, u8, i32, i32)]) -> Game {
    Game::new(
        reg,
        Bounds {
            width: 14,
            height: 14,
        },
        [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .enumerate()
            .map(|(id, d)| {
                let mut p = Player::new(id as u8, d);
                p.mana = 20.into();
                p.mana_per_turn = 0;
                p
            })
            .collect(),
        pieces
            .iter()
            .map(|&(name, owner, x, y)| Placement::new(name, owner, pos(x, y)))
            .collect(),
    )
    .unwrap()
}
fn game(pieces: &[(&str, u8, i32, i32)]) -> Game {
    setup(Registry::built_in().unwrap(), pieces)
}
fn doom_registry(clock: ClockUnit, after: u32) -> Registry {
    registry(|pieces, _| {
        let mut p = pieces.iter().find(|p| p.id == "king").unwrap().clone();
        p.id = "doomed_king".into();
        p.tags.remove(&cchess_engine::content::Tag::Immune);
        p.triggers = vec![TriggerDef {
            id: "doom".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::ScheduleOn {
                clock,
                after,
                effects: vec![EffectDef::RemoveSource],
            }],
        }];
        pieces.push(p);
    })
}

#[test]
fn four_armies_rotate_moves_and_decks_round_trip_deterministically() {
    let mut g = Game::four_player(Registry::built_in().unwrap()).unwrap();
    assert_eq!(
        g.bounds(),
        Bounds {
            width: 14,
            height: 14
        }
    );
    assert_eq!(g.pieces().count(), 64);
    for owner in 0..4 {
        assert_eq!(g.pieces().filter(|p| p.owner == owner).count(), 16);
        assert_eq!(g.player(owner).unwrap().cards.hand.len(), 3);
    }
    let mut replay = g.clone();
    for (from, to) in [
        ((3, 1), (3, 3)),
        ((1, 10), (3, 10)),
        ((10, 12), (10, 10)),
        ((12, 3), (10, 3)),
    ] {
        let intent = ActionIntent::Piece {
            player: g.turn().player,
            source: support::id(&g, from.0, from.1),
            kind: ActionKind::Move,
            target: Pos::new(to.0, to.1),
        };
        let preview = g.preview(&intent).unwrap();
        assert_eq!(g.apply(intent.clone()).unwrap(), preview);
        assert_eq!(replay.apply(intent).unwrap(), preview);
    }
    assert_eq!(g.turn().player, 0);
    assert_eq!(g.clock().turns, 4);
    assert_eq!(g.clock().rounds, 1);
    for p in g.players() {
        assert_eq!(p.cards.hand.len(), 4);
        assert_eq!(p.mana, 1);
    }
    assert_eq!(replay.snapshot(), g.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn king_removal_eliminates_one_army_and_skips_its_turn() {
    let mut g = game(&[
        ("rook", 0, 0, 0),
        ("king", 0, 7, 7),
        ("king", 1, 0, 3),
        ("imp", 1, 1, 3),
        ("king", 2, 3, 3),
        ("king", 3, 3, 0),
    ])
    .with_decks(
        (0..4)
            .map(|owner| (owner, deck(&["mana", "fire", "ice"])))
            .collect(),
        1,
        1,
        1,
        false,
    )
    .unwrap();
    let rook = id(&g, 0, 0);
    let result = act(&mut g, rook, ActionKind::Take, pos(0, 3));
    assert!(g.is_eliminated(1));
    assert!(!g.is_over());
    assert_eq!(g.turn().player, 2);
    assert!(!g.pieces().any(|p| p.owner == 1));
    assert!(g
        .card_instances()
        .filter(|c| c.owner == 1)
        .all(|c| c.zone == CardZone::Discard));
    assert_eq!(g.player(1).unwrap().mana, 20); // Cleanup gives no Imp reward.
    assert_eq!(g.owner_turn(1), Some(0));
    assert!(result
        .events
        .contains(&Event::PlayerEliminated { player: 1 }));
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.clock().rounds, 1);
    act(&mut g, rook, ActionKind::Take, pos(3, 3));
    assert!(g.is_eliminated(2));
    assert!(!g.is_over());
    assert_eq!(g.turn().player, 3);
    pass(&mut g);
    let final_action = act(&mut g, rook, ActionKind::Take, pos(3, 0));
    assert_eq!(g.winner(), Some(0));
    assert!(g.is_eliminated(3));
    assert_eq!(
        final_action
            .events
            .iter()
            .filter(|e| matches!(e, Event::GameWon { .. }))
            .count(),
        1
    );
    assert_eq!(
        g.apply(ActionIntent::EndTurn { player: 0 }),
        Err(RuleError::GameOver)
    );
}

#[test]
fn multiple_royals_require_removing_the_last_one() {
    let mut g = game(&[("rook", 0, 0, 0), ("king", 1, 0, 3), ("king", 1, 0, 5)]);
    let rook = id(&g, 0, 0);
    act(&mut g, rook, ActionKind::Take, pos(0, 3));
    assert!(!g.is_eliminated(1));
    assert_eq!(g.turn().player, 1);
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    act(&mut g, rook, ActionKind::Take, pos(0, 5));
    assert!(g.is_eliminated(1));
}

#[test]
fn owner_start_death_skips_player_without_accepting_an_action() {
    let mut g = setup(
        doom_registry(ClockUnit::OwnerTurns, 1),
        &[
            ("king", 0, 7, 7),
            ("doomed_king", 1, 0, 0),
            ("king", 2, 7, 0),
            ("king", 3, 0, 7),
        ],
    );
    let trace = pass(&mut g);
    assert!(g.is_eliminated(1));
    assert_eq!(g.turn().player, 2);
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e, Event::TurnStarted { player: 1, .. })));
    assert_eq!(g.clock().turns, 2);
}

#[test]
fn end_clock_elimination_recalculates_next_seat() {
    let mut g = setup(
        doom_registry(ClockUnit::Turns, 1),
        &[
            ("king", 0, 7, 7),
            ("doomed_king", 1, 0, 0),
            ("king", 2, 7, 0),
            ("king", 3, 0, 7),
        ],
    );
    pass(&mut g);
    assert_eq!(g.turn().player, 2);
    assert!(g.is_eliminated(1));
    assert_eq!(g.owner_turn(1), Some(1));
    assert_eq!(g.clock().turns, 2);
}

#[test]
fn eliminated_first_seat_still_allows_round_wrap() {
    let mut g = setup(
        doom_registry(ClockUnit::Turns, 1),
        &[
            ("doomed_king", 0, 7, 7),
            ("king", 1, 0, 0),
            ("king", 2, 7, 0),
            ("king", 3, 0, 7),
        ],
    );
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    assert!(g.is_eliminated(0));
    assert_eq!(g.turn().player, 1);
    assert_eq!(g.clock().rounds, 1);
}

#[test]
fn eliminating_all_players_in_one_clock_batch_produces_a_draw() {
    let mut g = setup(
        doom_registry(ClockUnit::Turns, 1),
        &[
            ("doomed_king", 0, 7, 7),
            ("doomed_king", 1, 0, 0),
            ("doomed_king", 2, 7, 0),
            ("doomed_king", 3, 0, 7),
        ],
    );
    let trace = pass(&mut g);
    assert!(g.is_draw());
    assert_eq!(g.winner(), None);
    assert!(g.is_over());
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|e| matches!(e, Event::GameDrawn))
            .count(),
        1
    );
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::GameWon { .. })));
    assert_eq!(
        g.apply(ActionIntent::EndTurn { player: 0 }),
        Err(RuleError::GameOver)
    );
}

#[test]
fn elimination_cancels_future_jobs_and_disables_persistent_cards() {
    let mut g = setup(
        doom_registry(ClockUnit::Rounds, 10),
        &[
            ("rook", 0, 0, 0),
            ("king", 0, 7, 7),
            ("doomed_king", 1, 0, 3),
            ("king", 2, 7, 0),
            ("king", 3, 0, 7),
        ],
    )
    .with_decks(
        (0..4)
            .map(|owner| (owner, deck(&["supply_cache"])))
            .collect(),
        1,
        1,
        0,
        false,
    )
    .unwrap();
    pass(&mut g);
    let card = g.player(1).unwrap().cards.hand[0];
    g.apply(ActionIntent::PlayCard {
        player: 1,
        card,
        target: Default::default(),
    })
    .unwrap();
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    let rook = id(&g, 0, 0);
    let trace = act(&mut g, rook, ActionKind::Take, pos(0, 3));
    assert!(g.scheduled().iter().all(|j| j.owner != 1));
    assert!(!g.player(1).unwrap().cards.discard.is_empty());
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e, Event::ScheduledCancelled { .. })));
}

#[test]
fn promotion_supports_every_side_and_resumes_the_next_seat() {
    for (owner, from, to) in [
        (0, (3, 9), (3, 10)),
        (1, (9, 3), (10, 3)),
        (2, (3, -2), (3, -3)),
        (3, (-2, 3), (-3, 3)),
    ] {
        let mut g = game(&[("pawn", owner, from.0, from.1)]);
        for _ in 0..owner {
            pass(&mut g);
        }
        let source = id(&g, from.0, from.1);
        act(&mut g, source, ActionKind::Move, pos(to.0, to.1));
        let pending = g.pending_promotion().unwrap().clone();
        assert_eq!(g.turn().player, owner);
        g.apply(ActionIntent::ChoosePromotion {
            player: owner,
            token: pending.token,
            archetype: "queen".into(),
        })
        .unwrap();
        assert_eq!(g.turn().player, (owner + 1) % 4);
    }
}

#[test]
fn imp_cooldown_counts_owner_turns_not_intervening_opponents() {
    let mut g = game(&[("imp", 0, 3, 3)])
        .with_decks(
            (0..4)
                .map(|owner| (owner, deck(&["mana", "mana"])))
                .collect(),
            1,
            0,
            0,
            false,
        )
        .unwrap();
    let imp = id(&g, 3, 3);
    cast(&mut g, imp, "draw", Some(pos(3, 3)));
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.turn().player, 0);
    assert!(!g.legal_abilities(imp).is_empty());
    for _ in 0..4 {
        pass(&mut g);
    }
    assert!(!g.legal_abilities(imp).is_empty());
}

#[test]
fn custom_player_ids_use_roster_order_and_duplicates_are_rejected() {
    let players: Vec<_> = [9, 4, 7, 2]
        .into_iter()
        .zip([Direction::N, Direction::E, Direction::S, Direction::W])
        .map(|(id, d)| Player::new(id, d))
        .collect();
    let mut g = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 14,
            height: 14,
        },
        players.clone(),
        vec![],
    )
    .unwrap();
    for expected in [4, 7, 2, 9] {
        pass(&mut g);
        assert_eq!(g.turn().player, expected);
    }
    let mut duplicate = players;
    duplicate[3].id = 9;
    assert!(Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 14,
            height: 8
        },
        duplicate,
        vec![]
    )
    .is_err());
}

#[test]
fn area_abilities_and_demonic_rewards_handle_three_opponents() {
    let mut g = game(&[
        ("witch", 0, 3, 3),
        ("imp", 1, 2, 3),
        ("imp", 2, 3, 4),
        ("imp", 3, 4, 3),
    ]);
    let witch = id(&g, 3, 3);
    cast(&mut g, witch, "burst", None);
    assert_eq!(g.pieces().count(), 1);
    for owner in 1..4 {
        assert_eq!(g.player(owner).unwrap().mana, 21);
    }
}

#[test]
fn elimination_preview_and_replay_preserve_live_seats_and_cancellation() {
    let mut g = setup(
        doom_registry(ClockUnit::Rounds, 10),
        &[
            ("rook", 0, 0, 0),
            ("doomed_king", 1, 0, 3),
            ("king", 2, 7, 0),
            ("king", 3, 7, 7),
        ],
    );
    let before = g.snapshot();
    let intent = ActionIntent::Piece {
        player: 0,
        source: id(&g, 0, 0),
        kind: ActionKind::Take,
        target: pos(0, 3),
    };
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(intent.clone()).unwrap(), preview);
    assert_eq!(replay.apply(intent).unwrap(), preview);
    for _ in 0..6 {
        assert_eq!(pass(&mut g), pass(&mut replay));
    }
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn lethal_status_at_turn_end_eliminates_owner_and_advances() {
    use cchess_engine::state::StatusKind;
    let reg = registry(|pieces, _| {
        let p = pieces.iter_mut().find(|p| p.id == "king").unwrap();
        p.tags.remove(&cchess_engine::content::Tag::Immune);
        p.triggers = vec![TriggerDef {
            id: "burning".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::AddStatus {
                kind: StatusKind::Fire,
                duration: 1,
            }],
        }];
    });
    let mut g = setup(
        reg,
        &[
            ("king", 0, 7, 7),
            ("rook", 1, 0, 0),
            ("rook", 2, 7, 0),
            ("rook", 3, 0, 7),
        ],
    );
    for _ in 0..3 {
        pass(&mut g);
    }
    let trace = pass(&mut g);
    assert!(g.is_eliminated(0));
    assert_eq!(g.turn().player, 1);
    assert!(trace
        .events
        .contains(&Event::PlayerEliminated { player: 0 }));
}
