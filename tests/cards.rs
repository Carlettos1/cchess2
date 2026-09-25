mod support;
use cchess_engine::{
    cards::{CardChoice as C, CardDef, CardKind, CardTarget, CardZone, TriggerLimit},
    content::{ActionCost, Condition, EffectDef},
    state::{ActionSource, Placement, Player, Snapshot},
    *,
};
use std::collections::BTreeSet;
use support::*;

fn zones_are_consistent(g: &Game) {
    let mut seen = BTreeSet::new();
    for owner in [0, 1] {
        let p = g.player(owner).unwrap();
        for zone in [
            CardZone::Deck,
            CardZone::Hand,
            CardZone::Discard,
            CardZone::OnBoard,
            CardZone::Consumed,
        ] {
            for id in p.cards.get(zone) {
                assert!(seen.insert(*id));
                let c = g.card_instance(*id).unwrap();
                assert_eq!((c.owner, c.zone), (owner, zone));
            }
        }
        let active = p
            .cards
            .on_board
            .iter()
            .map(|id| g.card_instance(*id).unwrap().definition.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(active, p.board_cards);
    }
    assert_eq!(seen.len(), g.card_instances().count());
}

#[test]
fn card_damage_uses_card_identity_and_immunity_even_when_card_and_piece_ids_overlap() {
    let reg = registry(|pieces, cards| {
        pieces
            .iter_mut()
            .find(|p| p.id == "wall")
            .unwrap()
            .tags
            .insert(cchess_engine::content::Tag::Heroic);
        let c = cards.iter_mut().find(|c| c.id == "insight").unwrap();
        c.target = CardTarget::Square(cchess_engine::targeting::TargetSpec {
            origin: cchess_engine::targeting::TargetOrigin::Board,
            pattern: cchess_engine::geometry::Pattern::SelfOnly,
            filters: vec![
                cchess_engine::targeting::TargetFilter::Enemy,
                cchess_engine::targeting::TargetFilter::Occupied,
            ],
            count: 1,
        });
        c.effects = vec![EffectDef::DamageTarget(1)];
    });
    let mut g = setup(
        reg,
        &[("super_pawn", 1, 3, 3), ("wall", 1, 4, 3)],
        &["insight"],
        &["mana"],
        1,
        0,
    );
    let card = hand(&g, "insight");
    assert_eq!(card, id(&g, 3, 3));
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card,
            target: C::Square(Pos::new(3, 3))
        }),
        Err(RuleError::ProtectedTarget)
    );
    assert_eq!(g, before);
    let trace = play(&mut g, "insight", C::Square(Pos::new(4, 3)));
    assert!(g.at(Pos::new(4, 3)).is_none());
    assert!(trace.events.iter().any(|e|matches!(e,Event::DamageDealt{source:ActionSource::Card(id),kind:ActionKind::PlayCard,..}if *id==card)));
    assert_eq!(g.dead()[0].source, ActionSource::Card(card));
}

#[test]
fn fire_from_a_discarded_card_preserves_card_source_on_expiration() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "insight").unwrap();
        c.target = CardTarget::Square(cchess_engine::targeting::TargetSpec {
            origin: cchess_engine::targeting::TargetOrigin::Board,
            pattern: cchess_engine::geometry::Pattern::SelfOnly,
            filters: vec![
                cchess_engine::targeting::TargetFilter::Enemy,
                cchess_engine::targeting::TargetFilter::Occupied,
            ],
            count: 1,
        });
        c.effects = vec![EffectDef::AddStatus {
            kind: cchess_engine::state::StatusKind::Fire,
            duration: 1,
        }];
    });
    let mut g = setup(reg, &[("wall", 1, 3, 3)], &["insight"], &["mana"], 1, 0);
    let card = hand(&g, "insight");
    play(&mut g, "insight", C::Square(Pos::new(3, 3)));
    let trace = pass(&mut g);
    assert!(g.at(Pos::new(3, 3)).is_none());
    assert_eq!(g.card_instance(card).unwrap().zone, CardZone::Discard);
    assert!(trace.events.iter().any(|e|matches!(e,Event::DamageDealt{source:ActionSource::Card(id),kind:ActionKind::PlayCard,..}if *id==card)));
}

#[test]
fn card_unlock_references_must_name_registered_persistent_cards() {
    for reference in ["missing", "mana"] {
        let reg = Registry::built_in().unwrap();
        let mut def = reg.card("mana").unwrap().definition.clone();
        def.id = "new_card".into();
        def.conditions = vec![Condition::And(vec![Condition::CardOnBoard(
            reference.into(),
        )])];
        let text = ron::to_string(&def).unwrap();
        let errors = reg
            .with_cards([("new_card.ron", text.as_str())])
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.file == "new_card.ron" && e.field == "conditions"));
    }
}
#[test]
fn ordered_decks_deal_unique_instances_and_draw_at_turn_start() {
    let mut g = setup(
        Registry::built_in().unwrap(),
        &[],
        &["mana", "fire", "ice"],
        &["ice", "mana", "fire"],
        1,
        1,
    );
    assert_eq!(
        g.card_instance(g.player(0).unwrap().cards.hand[0])
            .unwrap()
            .definition,
        "mana"
    );
    assert_eq!(
        g.card_instance(g.player(1).unwrap().cards.hand[0])
            .unwrap()
            .definition,
        "ice"
    );
    zones_are_consistent(&g);
    pass(&mut g);
    assert_eq!(g.player(1).unwrap().cards.hand.len(), 2);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 2);
    zones_are_consistent(&g);
}
#[test]
fn shuffle_is_seeded_replayable_and_independent_of_deck_argument_order() {
    let make = |seed, reverse| {
        let g = Game::new(
            Registry::built_in().unwrap(),
            Bounds {
                width: 8,
                height: 8,
            },
            vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
            vec![],
        )
        .unwrap();
        let d = deck(&[
            "mana",
            "fire",
            "ice",
            "forced_march",
            "insight",
            "blood_pact",
            "second_wind",
            "dispel",
        ]);
        let mut decks = vec![(0, d.clone()), (1, d)];
        if reverse {
            decks.reverse();
        }
        g.with_decks(decks, seed, 2, 0, true).unwrap()
    };
    assert_eq!(make(42, false), make(42, true));
    assert_ne!(
        make(42, false).player(0).unwrap().cards,
        make(99, false).player(0).unwrap().cards
    );
    let g = make(42, false);
    assert!(g
        .setup_events()
        .iter()
        .any(|e| matches!(e, Event::DeckShuffled { .. })));
    zones_are_consistent(&g);
}
#[test]
fn utility_discard_and_empty_deck_draw_do_not_recycle_or_lose_cards() {
    let mut g = game(&[], &["insight", "mana"], &["mana"], 1);
    let insight = hand(&g, "insight");
    let trace = play(&mut g, "insight", C::None);
    assert_eq!(g.card_instance(insight).unwrap().zone, CardZone::Discard);
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 1);
    assert!(g.player(0).unwrap().cards.deck.is_empty());
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e, Event::DrawSkipped { count: 1, .. })));
    assert_eq!(g.turn().remaining_actions, 1);
    assert_eq!(g.clock().movements, 1);
    zones_are_consistent(&g);
}
#[test]
fn wrong_owner_missing_card_wrong_zone_and_wrong_targets_are_atomic() {
    let mut g = game(&[], &["mana", "fire"], &["mana"], 2);
    let other = g.player(1).unwrap().cards.hand[0];
    let own = hand(&g, "mana");
    for (card, target) in [
        (other, C::None),
        (999, C::None),
        (own, C::Square(Pos::new(0, 0))),
    ] {
        let before = g.clone();
        assert!(g
            .apply(ActionIntent::PlayCard {
                player: 0,
                card,
                target
            })
            .is_err());
        assert_eq!(g, before);
    }
    play(&mut g, "mana", C::None);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: own,
            target: C::None
        }),
        Err(RuleError::InvalidCard)
    );
    assert_eq!(g, before);
}
#[test]
fn summon_targets_are_unique_empty_adjacencies_and_summons_start_acted() {
    let mut g = game(
        &[("pawn", 0, 2, 2), ("pawn", 0, 3, 2), ("pawn", 1, 7, 7)],
        &["summon_knight"],
        &["mana"],
        1,
    );
    let card = hand(&g, "summon_knight");
    let legal = g.legal_card_plays(card);
    let squares: Vec<_> = legal
        .iter()
        .filter_map(|a| {
            if let ActionIntent::PlayCard {
                target: C::Square(p),
                ..
            } = a
            {
                Some(*p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(squares.len(), squares.iter().collect::<BTreeSet<_>>().len());
    assert!(squares.contains(&Pos::new(2, 3)));
    assert!(!squares.contains(&Pos::new(7, 6)));
    assert!(!squares.contains(&Pos::new(2, 2)));
    let action = play(&mut g, "summon_knight", C::Square(Pos::new(2, 3)));
    let knight = id(&g, 2, 3);
    assert!(g.piece(knight).unwrap().state.acted);
    assert_eq!(g.card_instance(card).unwrap().zone, CardZone::Discard);
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e,Event::CardPlayed{card:id,..}if *id==card)));
    pass(&mut g);
    assert!(!g.legal_actions(knight).is_empty());
    zones_are_consistent(&g);
}
#[test]
fn summon_validation_checks_costs_empty_placement_and_owner_before_paying() {
    let mut g = game(&[("pawn", 0, 0, 0)], &["summon_knight"], &["mana"], 1);
    let card = hand(&g, "summon_knight");
    for target in [Pos::new(0, 0), Pos::new(7, 7), Pos::new(-1, 0)] {
        let before = g.clone();
        assert!(g
            .apply(ActionIntent::PlayCard {
                player: 0,
                card,
                target: C::Square(target)
            })
            .is_err());
        assert_eq!(g, before);
    }
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "summon_knight")
            .unwrap()
            .cost
            .mana = 21
    });
    let mut g = setup(
        reg,
        &[("pawn", 0, 0, 0)],
        &["summon_knight"],
        &["mana"],
        1,
        0,
    );
    let before = g.clone();
    assert!(matches!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "summon_knight"),
            target: C::Square(Pos::new(1, 1))
        }),
        Err(RuleError::InsufficientMana { .. })
    ));
    assert_eq!(g, before);
}
#[test]
fn forced_march_adds_actions_without_resetting_acted_and_second_wind_resets_only_acted() {
    let mut g = game(
        &[("rook", 0, 0, 0)],
        &["forced_march", "second_wind"],
        &["mana"],
        2,
    );
    let rook = id(&g, 0, 0);
    play(&mut g, "forced_march", C::None);
    assert_eq!(g.turn().remaining_actions, 3);
    act(&mut g, rook, ActionKind::Move, Pos::new(0, 1));
    assert_eq!(g.turn().remaining_actions, 2);
    assert!(g.legal_actions(rook).is_empty());
    play(&mut g, "second_wind", C::Square(Pos::new(0, 1)));
    assert_eq!(g.turn().remaining_actions, 2);
    assert!(!g.piece(rook).unwrap().state.acted);
    act(&mut g, rook, ActionKind::Move, Pos::new(0, 2));
    assert_eq!(g.turn().remaining_actions, 1);
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.turn().remaining_actions, 1);
}
#[test]
fn second_wind_does_not_reset_ability_cooldowns() {
    let mut g = game(
        &[("bishop", 0, 3, 3)],
        &["forced_march", "second_wind"],
        &["mana"],
        2,
    );
    let bishop = id(&g, 3, 3);
    play(&mut g, "forced_march", C::None);
    cast(&mut g, bishop, "sidestep", Some(Pos::new(3, 4)));
    play(&mut g, "second_wind", C::Square(Pos::new(3, 4)));
    assert_eq!(
        g.piece(bishop).unwrap().state.cooldowns["sidestep"].remaining,
        2
    );
    assert!(g.legal_abilities(bishop).is_empty());
    assert!(!g.legal_actions(bishop).is_empty());
}
#[test]
fn on_board_cards_unlock_real_abilities_and_duplicate_copies_cannot_stack() {
    let mut g = game(
        &[("magician", 0, 3, 3), ("pawn", 1, 3, 4)],
        &["fire", "fire"],
        &["mana"],
        2,
    );
    let magician = id(&g, 3, 3);
    assert!(g.legal_abilities(magician).is_empty());
    let fire = hand(&g, "fire");
    play(&mut g, "fire", C::None);
    assert!(g.has_board_card(0, "fire"));
    assert_eq!(g.card_instance(fire).unwrap().zone, CardZone::OnBoard);
    assert!(!g.legal_abilities(magician).is_empty());
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "fire"),
            target: C::None
        }),
        Err(RuleError::DuplicateBoardCard("fire".into()))
    );
    assert_eq!(g, before);
    cast(&mut g, magician, "fire", None);
    assert!(g.has_status(id(&g, 3, 4), cchess_engine::state::StatusKind::Fire));
    zones_are_consistent(&g);
}
#[test]
fn paladin_consumes_a_real_card_instance_and_cannot_reuse_it() {
    let mut g = game(
        &[("paladin", 0, 3, 3), ("pawn", 0, 3, 4)],
        &["invulnerability"],
        &["mana"],
        1,
    );
    let card = hand(&g, "invulnerability");
    play(&mut g, "invulnerability", C::None);
    let paladin = id(&g, 3, 3);
    cast(&mut g, paladin, "ward", Some(Pos::new(3, 4)));
    assert_eq!(g.card_instance(card).unwrap().zone, CardZone::Consumed);
    assert!(!g.has_board_card(0, "invulnerability"));
    zones_are_consistent(&g);
}
#[test]
fn royal_decree_discount_is_live_and_does_not_mutate_base_abilities() {
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "magician")
            .unwrap()
            .tags
            .remove(&cchess_engine::content::Tag::Immune);
    });
    let mut g = setup(
        reg,
        &[("magician", 0, 3, 3), ("pawn", 1, 3, 4)],
        &["fire", "ice", "royal_decree"],
        &["dispel"],
        3,
        0,
    );
    let magician = id(&g, 3, 3);
    play(&mut g, "fire", C::None);
    play(&mut g, "ice", C::None);
    let decree = hand(&g, "royal_decree");
    play(&mut g, "royal_decree", C::None);
    let before = g.player(0).unwrap().mana.clone();
    let action = cast(&mut g, magician, "fire", None);
    assert_eq!(g.player(0).unwrap().mana, before - 1);
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e,Event::AbilityDiscountApplied{card,amount:1,..}if *card==decree)));
    play(&mut g, "dispel", C::Card(decree));
    pass(&mut g);
    assert!(!g.has_board_card(0, "royal_decree"));
    let before = g.player(0).unwrap().mana.clone();
    cast(&mut g, magician, "ice", None);
    assert_eq!(g.player(0).unwrap().mana, before - 2);
    assert!(g
        .registry()
        .get("magician")
        .unwrap()
        .definition
        .abilities
        .iter()
        .all(|a| a.cost.mana == 2));
    zones_are_consistent(&g);
}
#[test]
fn blood_pact_rewards_each_friendly_demonic_death_in_addition_to_intrinsic_reward() {
    let mut g = game(
        &[
            ("mandragora", 0, 3, 4),
            ("spider_egg", 0, 4, 3),
            ("witch", 1, 3, 3),
        ],
        &["blood_pact"],
        &["mana"],
        1,
    );
    let pact = hand(&g, "blood_pact");
    play(&mut g, "blood_pact", C::None);
    pass(&mut g);
    let witch = id(&g, 3, 3);
    let action = cast(&mut g, witch, "burst", None);
    assert_eq!(g.player(0).unwrap().mana, 22);
    let rewards: Vec<_> = action
        .events
        .iter()
        .filter(
            |e| matches!(e,Event::ManaGranted{cause,..}if cause.source==ActionSource::Card(pact)),
        )
        .collect();
    assert_eq!(rewards.len(), 2);
}
#[test]
fn blood_pact_once_per_action_limit_prevents_duplicate_rewards() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "blood_pact")
            .unwrap()
            .triggers[0]
            .limit = TriggerLimit::OncePerAction
    });
    let mut g = setup(
        reg,
        &[
            ("mandragora", 0, 3, 4),
            ("spider_egg", 0, 4, 3),
            ("witch", 1, 3, 3),
        ],
        &["blood_pact"],
        &["mana"],
        1,
        0,
    );
    play(&mut g, "blood_pact", C::None);
    pass(&mut g);
    let witch = id(&g, 3, 3);
    cast(&mut g, witch, "burst", None);
    assert_eq!(g.player(0).unwrap().mana, 21);
}

#[test]
fn blood_pact_matches_enemy_demonic_deaths_but_not_ordinary_deaths() {
    let mut g = game(
        &[
            ("witch", 0, 3, 3),
            ("mandragora", 1, 3, 4),
            ("pawn", 1, 4, 3),
        ],
        &["blood_pact"],
        &["mana"],
        1,
    );
    play(&mut g, "blood_pact", C::None);
    let witch = id(&g, 3, 3);
    let trace = cast(&mut g, witch, "burst", None);
    assert_eq!(g.player(0).unwrap().mana, 18);
    assert_eq!(g.player(1).unwrap().mana, 21);
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|e| matches!(e, Event::CardTriggerFired { .. }))
            .count(),
        1
    );
}
#[test]
fn more_mana_fires_only_at_its_owners_start_and_stops_after_dispel() {
    let mut g = game(&[], &["more_mana"], &["dispel"], 1);
    let card = hand(&g, "more_mana");
    play(&mut g, "more_mana", C::None);
    assert_eq!(g.player(0).unwrap().mana, 18);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 18);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 19);
    pass(&mut g);
    play(&mut g, "dispel", C::Card(card));
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 19);
}
#[test]
fn event_haste_resolves_discarded_card_work_immediately_with_original_attribution() {
    let mut g = game(
        &[],
        &["supply_cache", "event_haste", "mana", "fire"],
        &["mana"],
        2,
    );
    let cache = hand(&g, "supply_cache");
    let original = play(&mut g, "supply_cache", C::None);
    let job = g.scheduled()[0].id;
    assert_eq!(g.card_instance(cache).unwrap().zone, CardZone::Discard);
    let action = play(&mut g, "event_haste", C::Scheduled(job));
    assert!(g.scheduled().is_empty());
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 2);
    let draws: Vec<_> = action
        .events
        .iter()
        .filter_map(|e| {
            if let Event::CardMoved {
                to: CardZone::Hand,
                cause,
                ..
            } = e
            {
                Some(cause)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(draws.len(), 2);
    assert!(draws.iter().all(|c| c.source == ActionSource::Card(cache)
        && c.action_id == original.id
        && c.scheduled == Some(job)));
    zones_are_consistent(&g);
}
#[test]
fn scheduled_selection_enforces_ownership_and_cancellation_prevents_hatching() {
    let mut g = game(
        &[("spider_egg", 0, 3, 3), ("spider_egg", 1, 4, 4)],
        &["event_haste", "cancel_event"],
        &["mana"],
        2,
    );
    let enemy = g.scheduled().iter().find(|j| j.owner == 1).unwrap().id;
    let own = g.scheduled().iter().find(|j| j.owner == 0).unwrap().id;
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "event_haste"),
            target: C::Scheduled(enemy)
        }),
        Err(RuleError::InvalidScheduledTarget)
    );
    assert_eq!(g, before);
    play(&mut g, "cancel_event", C::Scheduled(own));
    for _ in 0..4 {
        pass(&mut g);
    }
    assert_eq!(g.at(Pos::new(3, 3)).unwrap().archetype, "spider_egg");
    assert_eq!(g.at(Pos::new(4, 4)).unwrap().archetype, "spider");
}
#[test]
fn generated_effect_failure_restores_card_zones_rng_costs_and_clock() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "insight").unwrap();
        c.effects = vec![EffectDef::Sequence(vec![
            EffectDef::ShuffleDeck,
            EffectDef::DrawCard(1),
            EffectDef::Sequence(vec![EffectDef::AddMana(1); 1024]),
        ])];
    });
    let mut g = setup(
        reg,
        &[],
        &["insight", "mana", "fire", "ice"],
        &["mana"],
        1,
        0,
    );
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.log().len(), 2);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "insight"),
            target: C::None
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g, before);
    zones_are_consistent(&g);
}
#[test]
fn shuffle_and_draw_effects_replay_and_snapshot_roundtrip_exactly() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "insight")
            .unwrap()
            .effects = vec![EffectDef::ShuffleDeck, EffectDef::DrawCard(2)]
    });
    let initial = setup(
        reg,
        &[],
        &["insight", "mana", "fire", "ice", "forced_march"],
        &["mana"],
        1,
        0,
    );
    let mut g = initial.clone();
    let intent = ActionIntent::PlayCard {
        player: 0,
        card: hand(&g, "insight"),
        target: C::None,
    };
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g, initial);
    let action = g.apply(intent).unwrap();
    assert_eq!(action, preview);
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e, Event::DeckShuffled { .. })));
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
    let mut replay = initial;
    for a in g.log() {
        assert_eq!(replay.apply(a.intent.clone()).unwrap(), *a);
    }
    assert_eq!(g, replay);
}
#[test]
fn deck_and_card_schema_reject_unknown_references_and_unsafe_sources() {
    let reg = Registry::built_in().unwrap();
    assert!(reg.validate_deck(&deck(&["missing"])).is_err());
    let base = CardDef {
        schema_version: 1,
        id: "invalid".into(),
        display_name: "Invalid".into(),
        kind: CardKind::Utility,
        cost: ActionCost {
            actions: 0,
            mana: 0,
        },
        target: CardTarget::None,
        conditions: vec![],
        effects: vec![EffectDef::MoveSource],
        triggers: vec![],
        ability_discounts: vec![],
    };
    for effects in [
        vec![EffectDef::MoveSource],
        vec![EffectDef::SpawnTarget("missing".into())],
        vec![EffectDef::Schedule {
            after: 0,
            effects: vec![EffectDef::AddMana(1)],
        }],
        vec![EffectDef::ModifyScheduledEvent(2)],
    ] {
        let mut d = base.clone();
        d.effects = effects;
        let text = ron::to_string(&d).unwrap();
        let errors = reg
            .clone()
            .with_cards([("invalid.ron", text.as_str())])
            .unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.file == "invalid.ron" && e.field == "effects"));
    }
    let mut d = base;
    d.effects = vec![EffectDef::AddMana(1)];
    d.conditions = vec![Condition::HasNotActed];
    let text = ron::to_string(&d).unwrap();
    assert!(reg.with_cards([("invalid.ron", text.as_str())]).is_err());
}
#[test]
fn promotion_blocks_card_plays_until_choice_and_summoned_pawns_promote() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "summon_knight")
            .unwrap()
            .effects = vec![EffectDef::SpawnTarget("pawn".into())]
    });
    let mut g = setup(
        reg,
        &[("rook", 0, 0, 6)],
        &["summon_knight", "mana"],
        &["mana"],
        2,
        0,
    );
    play(&mut g, "summon_knight", C::Square(Pos::new(0, 7)));
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "mana"),
            target: C::None
        }),
        Err(RuleError::PromotionPending)
    );
    assert_eq!(g, before);
    let pending = g.pending_promotion().unwrap().clone();
    g.apply(ActionIntent::ChoosePromotion {
        player: 0,
        token: pending.token,
        archetype: "queen".into(),
    })
    .unwrap();
    assert_eq!(g.at(Pos::new(0, 7)).unwrap().archetype, "queen");
    assert_eq!(g.clock().movements, 1);
    assert_eq!(g.clock().turns, 1);
}
#[test]
fn starting_zone_ids_are_not_trusted_and_deck_setup_is_one_time() {
    let mut p = Player::new(0, Direction::N);
    p.cards.hand.push(999);
    assert!(Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8
        },
        vec![p, Player::new(1, Direction::S)],
        Vec::<Placement>::new()
    )
    .is_err());
    let g = game(&[], &["mana"], &["mana"], 1);
    assert!(g
        .with_decks(
            vec![(0, deck(&["mana"])), (1, deck(&["mana"]))],
            1,
            1,
            1,
            false
        )
        .is_err());
}

#[test]
fn lasting_momentum_stacks_persists_and_replays() {
    let initial = game(
        &[("rook", 0, 0, 0), ("rook", 1, 7, 7)],
        &["lasting_momentum", "lasting_momentum"],
        &["mana"],
        2,
    );
    let mut g = initial.clone();
    let intent = ActionIntent::PlayCard {
        player: 0,
        card: hand(&g, "lasting_momentum"),
        target: C::None,
    };
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g, initial);
    assert_eq!(g.apply(intent).unwrap(), preview);
    assert_eq!(g.player(0).unwrap().actions_per_turn, 2);
    assert_eq!(g.turn().remaining_actions, 2);
    assert_eq!(g.player(0).unwrap().mana, 18);
    play(&mut g, "lasting_momentum", C::None);
    assert_eq!(g.player(0).unwrap().actions_per_turn, 3);
    assert_eq!(g.turn().remaining_actions, 3);
    assert_eq!(g.player(1).unwrap().actions_per_turn, 1);
    assert_eq!(g.player(0).unwrap().cards.discard.len(), 2);
    for _ in 0..3 {
        pass(&mut g);
        assert_eq!(g.turn().remaining_actions, 1);
        pass(&mut g);
        assert_eq!(g.turn().remaining_actions, 3);
    }
    let serialized = ron::to_string(&g.snapshot()).unwrap();
    assert_eq!(
        ron::from_str::<Snapshot>(&serialized).unwrap(),
        g.snapshot()
    );
    let mut replay = initial;
    for action in g.log() {
        assert_eq!(replay.apply(action.intent.clone()).unwrap(), *action);
    }
    assert_eq!(replay, g);
}

#[test]
fn lasting_momentum_is_in_default_decks_and_requires_mana() {
    let g = Game::full_board(Registry::built_in().unwrap(), true).unwrap();
    for owner in 0..4 {
        assert_eq!(
            g.card_instances()
                .filter(|c| c.owner == owner && c.definition == "lasting_momentum")
                .count(),
            1
        );
    }
    let mut g = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 8,
            height: 8,
        },
        vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
        vec![],
    )
    .unwrap()
    .with_decks(
        vec![(0, deck(&["lasting_momentum"])), (1, deck(&["mana"]))],
        42,
        1,
        0,
        false,
    )
    .unwrap();
    assert_eq!(g.player(0).unwrap().mana, 1);
    let before = g.clone();
    assert!(g
        .apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "lasting_momentum"),
            target: C::None
        })
        .is_err());
    assert_eq!(g, before);
}
