mod support;
use cchess_engine::{
    cards::{CardChoice as C, CardMatcher, CardZone, TriggerLimit},
    clock::ClockUnit,
    content::EffectDef,
    state::{ActionSource, Snapshot},
    *,
};
use support::*;

#[test]
fn turn_start_promotion_resumes_without_repeating_timers_or_clock_ticks() {
    let reg = registry(|_, cards| {
        let target = cards
            .iter()
            .find(|c| c.id == "summon_knight")
            .unwrap()
            .target
            .clone();
        let c = cards.iter_mut().find(|c| c.id == "supply_cache").unwrap();
        c.target = target;
        c.effects = vec![EffectDef::ScheduleOn {
            clock: ClockUnit::OwnerTurns,
            after: 1,
            effects: vec![EffectDef::SpawnTarget("pawn".into())],
        }];
    });
    let mut g = setup(
        reg,
        &[("rook", 0, 0, 6)],
        &["supply_cache"],
        &["mana"],
        1,
        0,
    );
    play(&mut g, "supply_cache", C::Square(Pos::new(0, 7)));
    pass(&mut g);
    pass(&mut g);
    let clock = g.clock().clone();
    let pending = g.pending_promotion().unwrap().clone();
    let trace = g
        .apply(ActionIntent::ChoosePromotion {
            player: 0,
            token: pending.token,
            archetype: "queen".into(),
        })
        .unwrap();
    assert_eq!(g.clock(), &clock);
    assert_eq!(g.turn().player, 0);
    assert_eq!(g.turn().remaining_actions, 1);
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::ClockAdvanced { .. } | Event::TurnStarted { .. })));
}

#[test]
fn haste_clamps_every_clock_to_now_and_executes_exactly_once() {
    for clock in [
        ClockUnit::OwnerTurns,
        ClockUnit::Movements,
        ClockUnit::Turns,
        ClockUnit::Rounds,
    ] {
        let reg = registry(|_, cards| {
            cards
                .iter_mut()
                .find(|c| c.id == "supply_cache")
                .unwrap()
                .effects = vec![EffectDef::ScheduleOn {
                clock,
                after: 4,
                effects: vec![EffectDef::AddMana(3)],
            }];
            cards
                .iter_mut()
                .find(|c| c.id == "event_haste")
                .unwrap()
                .effects = vec![EffectDef::ModifyScheduledEvent(100)];
        });
        let mut g = setup(reg, &[], &["supply_cache", "event_haste"], &["mana"], 2, 0);
        play(&mut g, "supply_cache", C::None);
        let job = g.scheduled()[0].id;
        let now = g.clock_value(0, clock) + u64::from(clock == ClockUnit::Movements);
        let trace = play(&mut g, "event_haste", C::Scheduled(job));
        assert!(trace
            .events
            .iter()
            .any(|e| matches!(e,Event::ScheduledChanged{after,..}if *after==now)));
        assert_eq!(
            trace
                .events
                .iter()
                .filter(|e| matches!(e, Event::ScheduledReady { .. }))
                .count(),
            1
        );
        assert!(g.scheduled().is_empty());
        assert_eq!(g.player(0).unwrap().mana, 21);
        for _ in 0..8 {
            pass(&mut g);
        }
        assert_eq!(g.player(0).unwrap().mana, 21);
    }
}

#[test]
fn movements_turns_rounds_and_owner_turns_fire_on_distinct_start_boundaries() {
    for clock in [
        ClockUnit::Movements,
        ClockUnit::Turns,
        ClockUnit::Rounds,
        ClockUnit::OwnerTurns,
    ] {
        let reg = registry(|_, cards| {
            cards
                .iter_mut()
                .find(|c| c.id == "supply_cache")
                .unwrap()
                .effects = vec![EffectDef::ScheduleOn {
                clock,
                after: 1,
                effects: vec![EffectDef::AddMana(3)],
            }]
        });
        let mut g = setup(reg, &[], &["supply_cache", "mana"], &["mana"], 2, 0);
        play(&mut g, "supply_cache", C::None);
        assert_eq!(g.scheduled().len(), 1);
        if clock == ClockUnit::Movements {
            play(&mut g, "mana", C::None);
        } else {
            pass(&mut g);
            if clock != ClockUnit::Turns {
                assert_eq!(g.scheduled().len(), 1);
                pass(&mut g);
            }
        }
        assert!(g.scheduled().is_empty());
        assert_eq!(
            g.player(0).unwrap().mana,
            if clock == ClockUnit::Movements {
                24
            } else {
                22
            }
        );
    }
}

#[test]
fn movement_clock_uses_accepted_primary_actions_not_action_cost_or_failed_inputs() {
    let mut g = game(
        &[("rook", 0, 0, 0)],
        &["forced_march", "mana"],
        &["mana"],
        2,
    );
    play(&mut g, "forced_march", C::None);
    play(&mut g, "mana", C::None);
    assert_eq!(g.clock().movements, 2);
    assert_eq!(g.clock().turns, 0);
    let rook = id(&g, 0, 0);
    act(&mut g, rook, ActionKind::Move, Pos::new(0, 1));
    assert_eq!(g.clock().movements, 3);
    assert_eq!(g.clock().turns, 0);
    let before = g.clone();
    assert!(g
        .apply(ActionIntent::Piece {
            player: 0,
            source: rook,
            kind: ActionKind::Move,
            target: Pos::new(0, 2)
        })
        .is_err());
    assert_eq!(g, before);
    pass(&mut g);
    assert_eq!(g.clock().movements, 3);
    assert_eq!(g.clock().turns, 1);
}
#[test]
fn movement_delay_one_waits_for_next_movement_start() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "supply_cache")
            .unwrap()
            .effects = vec![EffectDef::ScheduleOn {
            clock: ClockUnit::Movements,
            after: 1,
            effects: vec![EffectDef::AddMana(3)],
        }]
    });
    let mut g = setup(reg, &[], &["supply_cache", "mana"], &["mana"], 2, 0);
    play(&mut g, "supply_cache", C::None);
    assert_eq!(g.scheduled().len(), 1);
    assert_eq!(g.player(0).unwrap().mana, 19);
    let trace = play(&mut g, "mana", C::None);
    let ready = trace
        .events
        .iter()
        .position(|e| matches!(e, Event::ScheduledReady { .. }))
        .unwrap();
    let start = trace
        .events
        .iter()
        .position(|e| matches!(e, Event::ActionStarted { .. }))
        .unwrap();
    assert!(ready < start);
    assert!(g.scheduled().is_empty());
}

#[test]
fn due_owner_work_and_card_income_run_after_turn_started() {
    let mut g = game(&[("mandragora", 0, 3, 3)], &["more_mana"], &["mana"], 1);
    play(&mut g, "more_mana", C::None);
    let plant = id(&g, 3, 3);
    cast(&mut g, plant, "harvest", Some(Pos::new(3, 3)));
    let action = pass(&mut g);
    let ready = action
        .events
        .iter()
        .position(|e| matches!(e, Event::ScheduledReady { .. }))
        .unwrap();
    let start = action
        .events
        .iter()
        .position(|e| matches!(e, Event::TurnStarted { player: 0, .. }))
        .unwrap();
    let trigger = action
        .events
        .iter()
        .position(|e| matches!(e, Event::CardTriggerFired { .. }))
        .unwrap();
    assert!(start < ready && ready < trigger);
    assert_eq!(g.player(0).unwrap().mana, 20);
}
#[test]
fn round_clock_fires_once_per_cycle_and_global_turn_work_can_belong_to_inactive_player() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "more_mana").unwrap();
        c.triggers[0].event = CardMatcher::Clock(ClockUnit::Rounds);
        cards
            .iter_mut()
            .find(|c| c.id == "supply_cache")
            .unwrap()
            .effects = vec![EffectDef::ScheduleOn {
            clock: ClockUnit::Turns,
            after: 1,
            effects: vec![EffectDef::AddMana(2)],
        }];
    });
    let mut g = setup(
        reg,
        &[],
        &["more_mana", "supply_cache"],
        &["supply_cache"],
        2,
        0,
    );
    play(&mut g, "more_mana", C::None);
    pass(&mut g);
    play(&mut g, "supply_cache", C::None);
    let before = g.player(1).unwrap().mana.clone();
    pass(&mut g);
    assert_eq!(g.player(1).unwrap().mana, before + 2);
    assert_eq!(g.player(0).unwrap().mana, 19);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 19);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().mana, 20);
}
#[test]
fn clock_trigger_limits_and_priority_are_deterministic() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "more_mana").unwrap();
        c.triggers[0].event = CardMatcher::Clock(ClockUnit::Movements);
        c.triggers[0].limit = TriggerLimit::OncePerTurn;
        let mut second = c.triggers[0].clone();
        second.id = "first".into();
        second.priority = -10;
        second.effects = vec![EffectDef::AddMana(2)];
        c.triggers.push(second);
    });
    let mut g = setup(reg, &[], &["more_mana", "mana", "mana"], &["mana"], 3, 0);
    let trace = play(&mut g, "more_mana", C::None);
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::CardTriggerFired { .. })));
    let trace = play(&mut g, "mana", C::None);
    let ids: Vec<_> = trace
        .events
        .iter()
        .filter_map(|e| {
            if let Event::CardTriggerFired { trigger, .. } = e {
                Some(trigger.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids, vec!["first", "income"]);
    let trace = play(&mut g, "mana", C::None);
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::CardTriggerFired { .. })));
    pass(&mut g);
    let trace = play(&mut g, "mana", C::None);
    assert_eq!(
        trace
            .events
            .iter()
            .filter(|e| matches!(e, Event::CardTriggerFired { .. }))
            .count(),
        2
    );
}

#[test]
fn promotion_is_a_continuation_not_an_extra_clock_movement() {
    let mut g = game(&[("pawn", 0, 0, 6)], &["mana"], &["mana"], 1);
    let pawn = id(&g, 0, 6);
    act(&mut g, pawn, ActionKind::Move, Pos::new(0, 7));
    assert_eq!(g.clock().movements, 1);
    let p = g.pending_promotion().unwrap().clone();
    g.apply(ActionIntent::ChoosePromotion {
        player: 0,
        token: p.token,
        archetype: "queen".into(),
    })
    .unwrap();
    assert_eq!((g.clock().movements, g.clock().turns), (1, 1));
    let mut initial = game(&[("pawn", 0, 0, 7)], &["mana"], &["mana"], 1);
    let p = initial.pending_promotion().unwrap().clone();
    initial
        .apply(ActionIntent::ChoosePromotion {
            player: 0,
            token: p.token,
            archetype: "queen".into(),
        })
        .unwrap();
    assert_eq!(initial.clock().movements, 0);
}
#[test]
fn pass_does_not_tick_movement_or_resolve_its_jobs() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "supply_cache")
            .unwrap()
            .effects = vec![EffectDef::ScheduleOn {
            clock: ClockUnit::Movements,
            after: 1,
            effects: vec![EffectDef::AddMana(3)],
        }]
    });
    let mut g = setup(reg, &[], &["supply_cache"], &["mana"], 1, 0);
    play(&mut g, "supply_cache", C::None);
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.clock().movements, 1);
    assert_eq!(g.clock().turns, 2);
    assert_eq!(g.scheduled().len(), 1);
}

#[test]
fn due_jobs_keep_stable_order_and_nested_schedules_wait_for_later_ticks() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "supply_cache")
            .unwrap()
            .effects = vec![
            EffectDef::ScheduleOn {
                clock: ClockUnit::OwnerTurns,
                after: 1,
                effects: vec![EffectDef::ScheduleOn {
                    clock: ClockUnit::OwnerTurns,
                    after: 1,
                    effects: vec![EffectDef::AddMana(4)],
                }],
            },
            EffectDef::ScheduleOn {
                clock: ClockUnit::OwnerTurns,
                after: 1,
                effects: vec![EffectDef::AddMana(2)],
            },
        ]
    });
    let mut g = setup(reg, &[], &["supply_cache"], &["mana"], 1, 0);
    let card = hand(&g, "supply_cache");
    play(&mut g, "supply_cache", C::None);
    pass(&mut g);
    let a = pass(&mut g);
    assert_eq!(
        a.events
            .iter()
            .filter_map(|e| if let Event::ScheduledReady { id } = e {
                Some(*id)
            } else {
                None
            })
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(g.scheduled().len(), 1);
    assert_eq!(g.player(0).unwrap().mana, 21);
    pass(&mut g);
    let a = pass(&mut g);
    assert!(g.scheduled().is_empty());
    assert_eq!(g.player(0).unwrap().mana, 25);
    assert!(a.events.iter().any(|e|matches!(e,Event::ManaGranted{cause,amount:4,..}if cause.source==ActionSource::Card(card)&&cause.action_id==1)));
}
#[test]
fn hasted_target_is_bound_to_entity_identity_and_rechecks_protection() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "supply_cache").unwrap();
        c.target = cchess_engine::cards::CardTarget::Square(cchess_engine::targeting::TargetSpec {
            origin: cchess_engine::targeting::TargetOrigin::Board,
            pattern: cchess_engine::geometry::Pattern::SelfOnly,
            filters: vec![
                cchess_engine::targeting::TargetFilter::Enemy,
                cchess_engine::targeting::TargetFilter::Occupied,
            ],
            count: 1,
        });
        c.effects = vec![EffectDef::Schedule {
            after: 2,
            effects: vec![EffectDef::DamageTarget(1)],
        }];
    });
    let mut g = setup(
        reg,
        &[("pawn", 1, 3, 5), ("paladin", 1, 2, 5)],
        &["supply_cache", "event_haste"],
        &["invulnerability"],
        2,
        0,
    );
    play(&mut g, "supply_cache", C::Square(Pos::new(3, 5)));
    let job = g.scheduled()[0].id;
    pass(&mut g);
    play(&mut g, "invulnerability", C::None);
    let paladin = id(&g, 2, 5);
    cast(&mut g, paladin, "ward", Some(Pos::new(3, 5)));
    let a = play(&mut g, "event_haste", C::Scheduled(job));
    assert!(g.at(Pos::new(3, 5)).is_some());
    assert!(a
        .events
        .iter()
        .any(|e| matches!(e, Event::EffectSkipped { .. })));
    assert!(g.scheduled().is_empty());
}
#[test]
fn clock_trigger_failure_rolls_back_the_action_and_trigger_use_records() {
    let reg = registry(|_, cards| {
        let c = cards.iter_mut().find(|c| c.id == "more_mana").unwrap();
        c.triggers[0].event = CardMatcher::Clock(ClockUnit::Movements);
        c.triggers[0].effects = vec![EffectDef::Sequence(vec![EffectDef::AddMana(1); 1024])];
    });
    let mut g = setup(reg, &[], &["more_mana", "mana"], &["mana"], 2, 0);
    play(&mut g, "more_mana", C::None);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::PlayCard {
            player: 0,
            card: hand(&g, "mana"),
            target: C::None
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g, before);
}
#[test]
fn cancelled_and_consumed_sources_remain_serializable_with_clock_replay() {
    let initial = Game::cards_demo(Registry::built_in().unwrap()).unwrap();
    let mut g = initial.clone();
    for _ in 0..20 {
        pass(&mut g);
    }
    play(&mut g, "supply_cache", C::None);
    let job = g
        .scheduled()
        .iter()
        .find(|j| matches!(j.cause.source, ActionSource::Card(_)))
        .unwrap()
        .id;
    play(&mut g, "event_haste", C::Scheduled(job));
    play(&mut g, "more_mana", C::None);
    pass(&mut g);
    play(&mut g, "supply_cache", C::None);
    pass(&mut g);
    pass(&mut g);
    pass(&mut g);
    assert!(g.card_instances().any(|c| c.zone == CardZone::Discard));
    assert!(g.clock().rounds >= 2);
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
