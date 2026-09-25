use cchess_engine::{
    content::{
        AbilityDef, ActionCost, EffectDef, PieceArchetype, Selection, Tag, TriggerDef,
        TriggerEvent, CLASSICAL_SOURCES, CUSTOM_SOURCES,
    },
    geometry::{Pattern, Relative},
    state::{Placement, Player, Snapshot, StatusKind},
    targeting::{TargetFilter, TargetOrigin, TargetSpec},
    *,
};

fn definitions() -> Vec<PieceArchetype> {
    CLASSICAL_SOURCES
        .iter()
        .chain(CUSTOM_SOURCES.iter())
        .map(|(_, s)| ron::from_str(s).unwrap())
        .collect()
}
fn registry(edit: impl FnOnce(&mut Vec<PieceArchetype>)) -> Registry {
    let mut defs = definitions();
    edit(&mut defs);
    let texts: Vec<_> = defs
        .iter()
        .map(|d| (d.id.clone(), ron::to_string(d).unwrap()))
        .collect();
    Registry::load(texts.iter().map(|(id, text)| (id.as_str(), text.as_str())))
        .unwrap()
        .with_cards(cchess_engine::cards::CARD_SOURCES.iter().copied())
        .unwrap()
}
fn player(id: u8, forward: Direction) -> Player {
    let mut p = Player::new(id, forward);
    p.mana = 20.into();
    p.mana_per_turn = 0;
    p.board_cards.insert("invulnerability".into());
    p
}
fn setup_with(reg: Registry, pieces: &[(&str, u8, i32, i32)], players: Vec<Player>) -> Game {
    Game::new(
        reg,
        Bounds {
            width: 8,
            height: 8,
        },
        players,
        pieces
            .iter()
            .map(|&(id, owner, x, y)| Placement::new(id, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
}
fn setup(pieces: &[(&str, u8, i32, i32)]) -> Game {
    setup_with(
        Registry::built_in().unwrap(),
        pieces,
        vec![player(0, Direction::N), player(1, Direction::S)],
    )
}
fn id(g: &Game, x: i32, y: i32) -> u64 {
    g.at(Pos::new(x, y)).unwrap().id
}
fn cast(g: &mut Game, source: u64, ability: &str, target: Option<Pos>) -> ResolvedAction {
    g.apply(ActionIntent::Ability {
        player: g.turn().player,
        source,
        ability: ability.into(),
        target,
    })
    .unwrap()
}
fn pass(g: &mut Game) -> ResolvedAction {
    g.apply(ActionIntent::EndTurn {
        player: g.turn().player,
    })
    .unwrap()
}
fn act(g: &mut Game, source: u64, kind: ActionKind, target: Pos) -> ResolvedAction {
    g.apply(ActionIntent::Piece {
        player: g.turn().player,
        source,
        kind,
        target,
    })
    .unwrap()
}
fn self_ability(id: &str, effects: Vec<EffectDef>) -> AbilityDef {
    AbilityDef {
        id: id.into(),
        display_name: id.into(),
        target: TargetSpec {
            origin: TargetOrigin::Source,
            pattern: Pattern::SelfOnly,
            filters: vec![TargetFilter::Friendly, TargetFilter::Occupied],
            count: 1,
        },
        selection: Selection::One,
        conditions: vec![],
        cost: ActionCost {
            actions: 1,
            mana: 1,
        },
        cooldown: 0,
        once: false,
        consume_cards: vec![],
        effects,
    }
}

#[test]
fn mandragora_pays_on_next_two_owner_turns_even_after_death() {
    let mut g = setup(&[("mandragora", 0, 0, 0), ("archer", 1, 0, 6)]);
    let source = id(&g, 0, 0);
    let archer = id(&g, 0, 6);
    let original = cast(&mut g, source, "harvest", Some(Pos::new(0, 0)));
    assert_eq!(g.player(0).unwrap().mana, 20);
    assert_eq!(
        g.scheduled().iter().map(|j| j.due_tick).collect::<Vec<_>>(),
        vec![2, 3]
    );
    act(&mut g, archer, ActionKind::Attack, Pos::new(0, 0));
    // One Demonic removal reward plus the first delayed payout.
    assert_eq!(g.player(0).unwrap().mana, 22);
    assert!(g.piece(source).is_none());
    assert_eq!(g.scheduled().len(), 1);
    pass(&mut g);
    let resolved = pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 24);
    assert!(g.scheduled().is_empty());
    let cause = resolved
        .events
        .iter()
        .find_map(|e| {
            if let Event::ManaGranted {
                amount: 2, cause, ..
            } = e
            {
                Some(cause)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(cause.action_id, original.id);
    assert_eq!(
        cause.source,
        cchess_engine::state::ActionSource::Piece(source)
    );
    assert!(cause.scheduled.is_some());
    for _ in 0..4 {
        pass(&mut g);
    }
    assert_eq!(g.player(0).unwrap().mana, 24);
}
#[test]
fn delayed_jobs_use_owners_not_global_turns_for_either_player() {
    let mut g = setup(&[("mandragora", 1, 3, 6)]);
    let source = id(&g, 3, 6);
    pass(&mut g);
    cast(&mut g, source, "harvest", Some(Pos::new(3, 6)));
    assert_eq!(g.turn().player, 0);
    assert_eq!(g.player(1).unwrap().mana, 20);
    pass(&mut g);
    assert_eq!(g.player(1).unwrap().mana, 21);
    assert_eq!(g.owner_turn(1), Some(2));
    pass(&mut g);
    assert_eq!(g.player(1).unwrap().mana, 21);
    pass(&mut g);
    assert_eq!(g.player(1).unwrap().mana, 23);
}
#[test]
fn tesla_resolves_live_enemy_structures_before_turn_started_even_if_caster_dies() {
    let reg = registry(|d| {
        let mut p = d.iter().find(|p| p.id == "rook").unwrap().clone();
        p.id = "immune_rook".into();
        p.tags.insert(Tag::Immune);
        d.push(p);
    });
    let mut g = setup_with(
        reg,
        &[
            ("tesla_tower", 0, 0, 0),
            ("rook", 0, 1, 0),
            ("archer", 1, 0, 6),
            ("rook", 1, 7, 7),
            ("immune_rook", 1, 6, 7),
            ("rook", 1, 5, 7),
            ("paladin", 1, 1, 6),
            ("pawn", 1, 4, 4),
        ],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let tesla = id(&g, 0, 0);
    let archer = id(&g, 0, 6);
    let paladin = id(&g, 1, 6);
    let victim = id(&g, 7, 7);
    cast(&mut g, tesla, "overload", Some(Pos::new(0, 0)));
    cast(&mut g, paladin, "ward", Some(Pos::new(5, 7)));
    assert!(!g.has_status(victim, StatusKind::Deactivate));
    pass(&mut g);
    let resolved = act(&mut g, archer, ActionKind::Attack, Pos::new(0, 0));
    assert!(g.piece(tesla).is_none());
    assert_eq!(
        g.piece(victim).unwrap().statuses[&StatusKind::Deactivate].remaining,
        6
    );
    for (x, y) in [(1, 0), (6, 7), (5, 7), (4, 4)] {
        assert!(!g.has_status(id(&g, x, y), StatusKind::Deactivate));
    }
    let ready = resolved
        .events
        .iter()
        .position(|e| matches!(e, Event::ScheduledReady { .. }))
        .unwrap();
    let start = resolved
        .events
        .iter()
        .position(|e| matches!(e, Event::TurnStarted { .. }))
        .unwrap();
    assert!(start < ready);
    assert!(g.piece(victim).unwrap().statuses[&StatusKind::Deactivate]
        .source
        .scheduled
        .is_some());
}
#[test]
fn spider_spawn_trigger_hatches_same_entity_after_two_owner_starts() {
    let mut g = setup(&[("spider", 0, 3, 3)]);
    let spider = id(&g, 3, 3);
    let action = cast(&mut g, spider, "lay_egg", Some(Pos::new(3, 3)));
    let egg = id(&g, 3, 4);
    assert_ne!(egg, spider);
    assert_eq!(g.piece(egg).unwrap().archetype, "spider_egg");
    assert_eq!(g.scheduled().len(), 1);
    let spawn = action
        .events
        .iter()
        .position(|e| matches!(e, Event::PieceSpawned { .. }))
        .unwrap();
    let trigger = action
        .events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::TriggerFired {
                    event: TriggerEvent::Spawned,
                    ..
                }
            )
        })
        .unwrap();
    assert!(spawn < trigger);
    pass(&mut g);
    assert_eq!(g.piece(egg).unwrap().archetype, "spider_egg");
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.piece(egg).unwrap().archetype, "spider");
    assert!(g.piece(egg).unwrap().state.acted);
    assert!(g.legal_actions(egg).is_empty());
    assert!(g.scheduled().is_empty());
    assert!(g.dead().is_empty());
    pass(&mut g);
    pass(&mut g);
    assert!(!g.legal_actions(egg).is_empty());
}
#[test]
fn killed_egg_never_hatches_and_demonic_reward_occurs_once() {
    let mut g = setup(&[("spider", 0, 3, 3), ("archer", 1, 3, 7)]);
    let spider = id(&g, 3, 3);
    let archer = id(&g, 3, 7);
    cast(&mut g, spider, "lay_egg", Some(Pos::new(3, 3)));
    let egg = id(&g, 3, 4);
    act(&mut g, archer, ActionKind::Attack, Pos::new(3, 4));
    assert_eq!(g.player(0).unwrap().mana, 20);
    assert!(g.piece(egg).is_none());
    pass(&mut g);
    let action = pass(&mut g);
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e, Event::EffectSkipped { .. })));
    assert!(g.scheduled().is_empty());
    assert!(g.at(Pos::new(3, 4)).is_none());
    assert_eq!(g.player(0).unwrap().mana, 20);
    assert_eq!(g.log().iter().flat_map(|a|&a.events).filter(|e|matches!(e,Event::TriggerFired{piece,event:TriggerEvent::Removed,..} if *piece==egg)).count(),1);
}
#[test]
fn initial_eggs_also_schedule_hatching_and_spawn_direction_rotates() {
    let mut g = setup(&[("spider_egg", 0, 2, 2)]);
    assert_eq!(g.scheduled().len(), 1);
    assert!(!g.setup_events().is_empty());
    for _ in 0..4 {
        pass(&mut g);
    }
    assert_eq!(g.at(Pos::new(2, 2)).unwrap().archetype, "spider");
    for (forward, to) in [
        (Direction::N, (3, 4)),
        (Direction::E, (4, 3)),
        (Direction::S, (3, 2)),
        (Direction::W, (2, 3)),
    ] {
        let mut g = setup_with(
            Registry::built_in().unwrap(),
            &[("spider", 0, 3, 3)],
            vec![player(0, forward), player(1, Direction::S)],
        );
        let spider = id(&g, 3, 3);
        cast(&mut g, spider, "lay_egg", Some(Pos::new(3, 3)));
        assert_eq!(g.at(Pos::new(to.0, to.1)).unwrap().archetype, "spider_egg");
    }
}
#[test]
fn blocked_and_off_board_egg_placement_is_atomic() {
    for pieces in [
        vec![("spider", 0, 3, 3), ("pawn", 0, 3, 4)],
        vec![("spider", 0, 3, 7)],
    ] {
        let mut g = setup(&pieces);
        let source = id(&g, pieces[0].2, pieces[0].3);
        let before = g.clone();
        let intent = ActionIntent::Ability {
            player: 0,
            source,
            ability: "lay_egg".into(),
            target: Some(g.piece(source).unwrap().position),
        };
        assert_eq!(g.apply(intent), Err(RuleError::InvalidTarget));
        assert_eq!(g, before);
    }
}
#[test]
fn witch_union_movement_and_area_damage_match_the_catalogue() {
    let mut g = setup(&[
        ("witch", 0, 3, 3),
        ("pawn", 0, 3, 4),
        ("mandragora", 1, 4, 3),
        ("king", 1, 4, 4),
    ]);
    let witch = id(&g, 3, 3);
    let pawn = id(&g, 3, 4);
    let mandragora = id(&g, 4, 3);
    let legal = g.legal_actions(witch);
    assert!(legal.iter().any(|a|matches!(a,ActionIntent::Piece{kind:ActionKind::Move,target,..} if *target==Pos::new(2,2))));
    assert!(!legal.iter().any(|a|matches!(a,ActionIntent::Piece{kind:ActionKind::Move,target,..} if *target==Pos::new(1,1))));
    cast(&mut g, witch, "burst", None);
    assert!(g.piece(pawn).is_none());
    assert!(g.piece(mandragora).is_none());
    assert!(g.at(Pos::new(4, 4)).is_some());
    assert_eq!(g.player(1).unwrap().mana, 21);
}
#[test]
fn archer_taxicab_attack_does_not_move_or_use_square_range() {
    let mut g = setup(&[("archer", 0, 0, 0), ("pawn", 1, 3, 3), ("pawn", 1, 4, 3)]);
    let archer = id(&g, 0, 0);
    let legal = g.legal_actions(archer);
    assert!(legal.iter().any(|a|matches!(a,ActionIntent::Piece{kind:ActionKind::Move,target,..} if *target==Pos::new(2,0))));
    assert!(!legal.iter().any(|a|matches!(a,ActionIntent::Piece{kind:ActionKind::Move,target,..} if *target==Pos::new(2,2))));
    assert_eq!(
        g.validate(&ActionIntent::Piece {
            player: 0,
            source: archer,
            kind: ActionKind::Attack,
            target: Pos::new(4, 3)
        }),
        Err(RuleError::InvalidTarget)
    );
    act(&mut g, archer, ActionKind::Attack, Pos::new(3, 3));
    assert_eq!(g.piece(archer).unwrap().position, Pos::new(0, 0));
    assert!(g.at(Pos::new(3, 3)).is_none());
}
fn delayed_damage_registry() -> Registry {
    registry(|defs| {
        let mut a = self_ability(
            "delayed",
            vec![EffectDef::Schedule {
                after: 1,
                effects: vec![EffectDef::DamageTarget(1)],
            }],
        );
        a.target = TargetSpec {
            origin: TargetOrigin::Source,
            pattern: Pattern::AreaSquare(7),
            filters: vec![TargetFilter::Enemy, TargetFilter::Occupied],
            count: 1,
        };
        defs.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .abilities
            .push(a);
    })
}
#[test]
fn delayed_selected_targets_follow_identity_and_recheck_protection() {
    let mut g = setup_with(
        delayed_damage_registry(),
        &[("rook", 0, 0, 0), ("pawn", 1, 3, 5)],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let caster = id(&g, 0, 0);
    let pawn = id(&g, 3, 5);
    cast(&mut g, caster, "delayed", Some(Pos::new(3, 5)));
    act(&mut g, pawn, ActionKind::Move, Pos::new(3, 4));
    assert!(g.piece(pawn).is_none());
    assert_eq!(g.dead()[0].piece.position, Pos::new(3, 4));
    let mut g = setup_with(
        delayed_damage_registry(),
        &[("rook", 0, 0, 0), ("pawn", 1, 3, 5), ("paladin", 1, 2, 5)],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let caster = id(&g, 0, 0);
    let paladin = id(&g, 2, 5);
    let pawn = id(&g, 3, 5);
    cast(&mut g, caster, "delayed", Some(Pos::new(3, 5)));
    let action = cast(&mut g, paladin, "ward", Some(Pos::new(3, 5)));
    assert!(g.piece(pawn).is_some());
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e,Event::EffectSkipped{reason,..} if reason=="target protected")));
}
#[test]
fn dead_selected_target_does_not_retarget_a_new_occupant() {
    let mut black = player(1, Direction::S);
    black.actions_per_turn = 2;
    let mut g = setup_with(
        delayed_damage_registry(),
        &[
            ("rook", 0, 0, 0),
            ("pawn", 1, 3, 5),
            ("witch", 1, 2, 5),
            ("rook", 1, 3, 7),
        ],
        vec![player(0, Direction::N), black],
    );
    let caster = id(&g, 0, 0);
    let witch = id(&g, 2, 5);
    let replacement = id(&g, 3, 7);
    cast(&mut g, caster, "delayed", Some(Pos::new(3, 5)));
    cast(&mut g, witch, "burst", None);
    act(&mut g, replacement, ActionKind::Move, Pos::new(3, 5));
    assert!(g.piece(replacement).is_some());
    assert_eq!(g.piece(replacement).unwrap().state.damage, 0);
}
#[test]
fn same_due_turn_orders_jobs_by_id_and_changed_source_cancels_old_transform() {
    let reg = registry(|defs| {
        let egg = defs.iter_mut().find(|d| d.id == "spider_egg").unwrap();
        egg.triggers = vec![TriggerDef {
            id: "change".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::Sequence(vec![
                EffectDef::Schedule {
                    after: 1,
                    effects: vec![EffectDef::AddMana(2)],
                },
                EffectDef::Schedule {
                    after: 1,
                    effects: vec![EffectDef::TransformSource("witch".into())],
                },
                EffectDef::Schedule {
                    after: 2,
                    effects: vec![EffectDef::TransformSource("spider".into())],
                },
            ])],
        }];
    });
    let mut g = setup_with(
        reg,
        &[("spider_egg", 0, 2, 2)],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    pass(&mut g);
    let action = pass(&mut g);
    assert_eq!(
        action
            .events
            .iter()
            .filter_map(|e| if let Event::ScheduledReady { id } = e {
                Some(*id)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(g.at(Pos::new(2, 2)).unwrap().archetype, "witch");
    pass(&mut g);
    let action = pass(&mut g);
    assert_eq!(g.at(Pos::new(2, 2)).unwrap().archetype, "witch");
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e, Event::EffectSkipped { .. })));
}
#[test]
fn resolution_cap_rolls_back_costs_mana_entities_and_logs() {
    let reg = registry(|defs| {
        defs.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .abilities
            .push(self_ability(
                "overflow",
                vec![EffectDef::Sequence(vec![
                    EffectDef::AddMana(1);
                    cchess_engine::events::MAX_RESOLUTION_ITEMS
                        + 1
                ])],
            ));
    });
    let mut g = setup_with(
        reg,
        &[("rook", 0, 0, 0)],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let source = id(&g, 0, 0);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "overflow".into(),
            target: Some(Pos::new(0, 0))
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g, before);
}
#[test]
fn scheduled_capacity_failure_rolls_back_partially_enqueued_jobs() {
    let reg = registry(|defs| {
        defs.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .abilities
            .push(self_ability(
                "fill",
                vec![EffectDef::Sequence(vec![
                    EffectDef::Schedule {
                        after: 10000,
                        effects: vec![EffectDef::AddMana(1)]
                    };
                    600
                ])],
            ));
    });
    let mut g = setup_with(
        reg,
        &[("rook", 0, 0, 0)],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let source = id(&g, 0, 0);
    cast(&mut g, source, "fill", Some(Pos::new(0, 0)));
    pass(&mut g);
    assert_eq!(g.scheduled().len(), 600);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "fill".into(),
            target: Some(Pos::new(0, 0))
        }),
        Err(RuleError::ScheduleLimit)
    );
    assert_eq!(g, before);
}
#[test]
fn recursive_spawn_triggers_terminate_with_transaction_rollback() {
    let reg = registry(|defs| {
        let mut copy = defs.iter().find(|d| d.id == "spider").unwrap().clone();
        copy.id = "replicator".into();
        copy.triggers = vec![TriggerDef {
            id: "replicate".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::SpawnRelative {
                archetype: "replicator".into(),
                directions: vec![Relative::F, Relative::R, Relative::B, Relative::L],
            }],
        }];
        defs.push(copy);
        defs.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .abilities
            .push(self_ability(
                "seed",
                vec![EffectDef::SpawnRelative {
                    archetype: "replicator".into(),
                    directions: vec![Relative::F],
                }],
            ));
    });
    let mut g = Game::new(
        reg,
        Bounds {
            width: 64,
            height: 64,
        },
        vec![player(0, Direction::N), player(1, Direction::S)],
        vec![Placement::new("rook", 0, Pos::new(32, 32))],
    )
    .unwrap();
    let source = id(&g, 32, 32);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "seed".into(),
            target: Some(Pos::new(32, 32))
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g, before);
}
#[test]
fn scheduled_state_roundtrips_and_replays_with_identical_jobs_and_events() {
    let initial = Game::events_demo(Registry::built_in().unwrap()).unwrap();
    let mut g = initial.clone();
    let mandragora = id(&g, 1, 1);
    cast(&mut g, mandragora, "harvest", Some(Pos::new(1, 1)));
    assert!(!g.scheduled().is_empty());
    let snapshot = ron::to_string(&g.snapshot()).unwrap();
    assert_eq!(ron::from_str::<Snapshot>(&snapshot).unwrap(), g.snapshot());
    for _ in 0..8 {
        pass(&mut g);
    }
    let mut replay = initial;
    for entry in g.log() {
        assert_eq!(&replay.apply(entry.intent.clone()).unwrap(), entry);
    }
    assert_eq!(g, replay);
}
