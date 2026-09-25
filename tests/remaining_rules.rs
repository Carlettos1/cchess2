mod support;
use cchess_engine::{
    cards::CardChoice,
    content::{ActionCost, EffectDef, Tag, TriggerDef, TriggerEvent},
    geometry::Pattern,
    state::{Placement, Player, Snapshot, StatusKind},
    targeting::{TargetFilter, TargetOrigin, TargetSpec},
    *,
};
use support::*;

#[test]
fn cards_cannot_discount_immune_abilities_but_still_discount_other_heroes() {
    let mut g = game(
        &[("magician", 0, 3, 3), ("queen", 0, 0, 0), ("pawn", 1, 3, 4)],
        &["fire", "royal_decree"],
        &["mana"],
        2,
    );
    let mage = id(&g, 3, 3);
    let queen = id(&g, 0, 0);
    play(&mut g, "fire", CardChoice::None);
    play(&mut g, "royal_decree", CardChoice::None);
    assert_eq!(
        g.effective_ability_cost(
            mage,
            ActionCost {
                actions: 1,
                mana: 2
            }
        )
        .mana,
        2
    );
    assert_eq!(
        g.effective_ability_cost(
            queen,
            ActionCost {
                actions: 1,
                mana: 2
            }
        )
        .mana,
        1
    );
    let before = g.player(0).unwrap().mana.clone();
    let trace = cast(&mut g, mage, "fire", None);
    assert_eq!(g.player(0).unwrap().mana, before - 2);
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::AbilityDiscountApplied { .. })));
}

#[test]
fn global_card_effects_skip_allied_and_enemy_immune_pieces() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "insight").unwrap();
        c.effects = vec![EffectDef::ForEach {
            target: TargetSpec {
                origin: TargetOrigin::Board,
                pattern: Pattern::SelfOnly,
                filters: vec![TargetFilter::Occupied],
                count: 1,
            },
            effects: vec![EffectDef::AddStatus {
                kind: StatusKind::Ice,
                duration: 3,
            }],
        }];
    });
    let mut g = setup(
        reg,
        &[
            ("paladin", 0, 3, 3),
            ("warlock", 1, 5, 5),
            ("rook", 0, 1, 1),
            ("rook", 1, 6, 6),
        ],
        &["insight"],
        &["mana"],
        1,
        0,
    );
    play(&mut g, "insight", CardChoice::None);
    for p in g.pieces() {
        assert_eq!(
            g.has_status(p.id, StatusKind::Ice),
            !g.has_tag(p.id, Tag::Immune)
        );
    }
}

#[test]
fn mana_grants_income_costs_and_replay_work_above_u32_limit() {
    let players = [Direction::N, Direction::S]
        .into_iter()
        .enumerate()
        .map(|(id, d)| {
            let mut p = Player::new(id as u8, d);
            p.mana = u32::MAX.into();
            p
        })
        .collect();
    let initial = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        players,
        vec![Placement::new("bishop", 0, Pos::new(3, 3))],
    )
    .unwrap()
    .with_decks(
        vec![(0, deck(&["mana", "fire"])), (1, deck(&["mana", "fire"]))],
        0,
        2,
        0,
        false,
    )
    .unwrap();
    let mut g = initial.clone();
    play(&mut g, "mana", CardChoice::None);
    assert_eq!(g.player(0).unwrap().mana.to_string(), "4294967297");
    play(&mut g, "fire", CardChoice::None);
    assert_eq!(g.player(0).unwrap().mana.to_string(), "4294967295");
    let bishop = id(&g, 3, 3);
    cast(&mut g, bishop, "sidestep", Some(Pos::new(3, 4)));
    assert_eq!(g.player(0).unwrap().mana.to_string(), "4294967295"); // Sidestep is free.
    for _ in 0..3 {
        pass(&mut g);
    }
    assert_eq!(g.player(0).unwrap().mana.to_string(), "4294967296");
    let mut replay = initial;
    for entry in g.log() {
        assert_eq!(replay.apply(entry.intent.clone()).unwrap(), *entry);
    }
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn all_presets_start_with_one_mana_and_rectangular_four_player_games_are_rejected() {
    for constructor in [
        Game::classical,
        Game::abilities_demo,
        Game::events_demo,
        Game::siege_demo,
        Game::cards_demo,
        Game::expansion_demo,
        Game::four_player,
    ] {
        let g = constructor(Registry::built_in().unwrap()).unwrap();
        assert!(g.players().iter().all(|p| p.mana == 1));
    }
    let players = || {
        [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .enumerate()
            .map(|(id, d)| Player::new(id as u8, d))
            .collect()
    };
    assert!(Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8
        },
        players(),
        vec![]
    )
    .is_err());
    assert!(Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 14,
            height: 14
        },
        players(),
        vec![Placement::new("rook", 0, Pos::new(0, 0))]
    )
    .is_err());
}

#[test]
fn lethal_start_status_records_turn_and_round_ticks_before_elimination() {
    let reg = registry(|pieces, _| {
        let mut king = pieces.iter().find(|p| p.id == "king").unwrap().clone();
        king.id = "burning_king".into();
        king.tags.remove(&Tag::Immune);
        king.triggers = vec![TriggerDef {
            id: "burn".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::AddStatus {
                kind: StatusKind::Fire,
                duration: 1,
            }],
        }];
        pieces.push(king);
    });
    for count in [2, 4] {
        let players = [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .take(count)
            .enumerate()
            .map(|(id, d)| Player::new(id as u8, d))
            .collect();
        let mut g = Game::new(
            reg.clone(),
            Bounds {
                width: 14,
                height: 14,
            },
            players,
            (0..count)
                .map(|id| {
                    Placement::new(
                        if id == 0 { "burning_king" } else { "king" },
                        id as u8,
                        Pos::new(3 + id as i32, 5),
                    )
                })
                .collect(),
        )
        .unwrap();
        for _ in 0..count - 1 {
            pass(&mut g);
        }
        let trace = pass(&mut g);
        assert_eq!(g.clock().rounds, 1);
        assert_eq!(g.clock().turns, if count == 2 { 2 } else { 5 });
        let wrap = trace
            .events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    Event::ClockAdvanced {
                        clock: cchess_engine::clock::ClockUnit::Rounds,
                        value: 1
                    }
                )
            })
            .unwrap();
        let removal = trace
            .events
            .iter()
            .position(|e| matches!(e, Event::PieceRemoved { .. }))
            .unwrap();
        assert!(wrap < removal);
    }
}

#[test]
fn targeted_cards_cannot_buff_or_disable_immune_allies_or_enemies() {
    use cchess_engine::cards::CardTarget;
    for status in [StatusKind::Ice, StatusKind::Invulnerable] {
        let reg = registry(|_, cards| {
            let card = cards.iter_mut().find(|c| c.id == "insight").unwrap();
            card.target = CardTarget::Square(TargetSpec {
                origin: TargetOrigin::Board,
                pattern: Pattern::SelfOnly,
                filters: vec![TargetFilter::Occupied],
                count: 1,
            });
            card.effects = vec![EffectDef::AddStatus {
                kind: status,
                duration: 3,
            }];
        });
        let mut g = setup(
            reg,
            &[("paladin", 0, 3, 3), ("warlock", 1, 5, 5)],
            &["insight"],
            &["mana"],
            1,
            0,
        );
        let before = g.snapshot();
        for target in [Pos::new(3, 3), Pos::new(5, 5)] {
            let intent = ActionIntent::PlayCard {
                player: 0,
                card: hand(&g, "insight"),
                target: CardChoice::Square(target),
            };
            assert_eq!(g.apply(intent), Err(RuleError::ProtectedTarget));
            assert_eq!(g.snapshot(), before);
        }
    }
}
