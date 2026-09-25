mod support;
use cchess_engine::{
    cards::CardChoice,
    content::{EffectDef, PieceArchetype, Selection, Tag},
    state::{Placement, Player, Snapshot, StatusKind},
    *,
};
use support::*;

fn large(pieces: &[(&str, u8, i32, i32)]) -> Game {
    Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 24,
            height: 24,
        },
        vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
        pieces
            .iter()
            .map(|&(name, owner, x, y)| Placement::new(name, owner, Pos::new(x, y)))
            .collect(),
    )
    .unwrap()
}
fn hasten(g: &Game, source: u64, scheduled: u64) -> ActionIntent {
    ActionIntent::ScheduledAbility {
        player: g.turn().player,
        source,
        ability: "hasten".into(),
        scheduled,
    }
}
fn summoning() -> Game {
    let mut g = game(
        &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let portal = id(&g, 3, 3);
    cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
    pass(&mut g);
    g
}

#[test]
fn basilisk_gaze_reaches_three_through_ten_on_each_orthogonal_ray() {
    let mut g = large(&[
        ("basilisk", 0, 11, 11),
        ("wall", 1, 11, 21),
        ("wall", 1, 21, 11),
        ("wall", 1, 8, 11),
        ("wall", 1, 11, 8),
        ("wall", 1, 11, 22),
        ("wall", 1, 12, 12),
    ]);
    let source = id(&g, 11, 11);
    cast(&mut g, source, "gaze", None);
    for (x, y) in [(11, 21), (21, 11), (8, 11), (11, 8)] {
        assert!(g.has_status(id(&g, x, y), StatusKind::Deactivate));
        assert_eq!(
            g.at(Pos::new(x, y)).unwrap().statuses[&StatusKind::Deactivate].remaining,
            2
        );
    }
    assert!(!g.has_status(id(&g, 11, 22), StatusKind::Deactivate));
    assert!(!g.has_status(id(&g, 12, 12), StatusKind::Deactivate));
}

#[test]
fn gaze_stops_at_close_friendly_and_immune_blockers() {
    let mut g = large(&[
        ("basilisk", 0, 11, 11),
        ("wall", 1, 11, 13),
        ("wall", 1, 11, 16),
        ("wall", 0, 15, 11),
        ("wall", 1, 17, 11),
        ("king", 1, 11, 7),
        ("wall", 1, 11, 5),
        ("wall", 1, 7, 11),
    ]);
    let source = id(&g, 11, 11);
    cast(&mut g, source, "gaze", None);
    assert_eq!(g.pieces().filter(|p| !p.statuses.is_empty()).count(), 1);
    assert!(g.has_status(id(&g, 7, 11), StatusKind::Deactivate));
}

#[test]
fn empty_gaze_is_atomic_and_does_not_reach_eleven() {
    let mut g = large(&[("basilisk", 0, 11, 11), ("wall", 1, 22, 11)]);
    let before = g.snapshot();
    let source = id(&g, 11, 11);
    assert!(g
        .apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "gaze".into(),
            target: None
        })
        .is_err());
    assert_eq!(g.snapshot(), before);
}

#[test]
fn basilisk_and_gargoyle_moves_and_captures_obey_ray_limits_and_blockers() {
    for (name, target, valid) in [
        ("basilisk", Pos::new(10, 10), true),
        ("basilisk", Pos::new(5, 10), false),
        ("gargoyle", Pos::new(5, 10), true),
        ("gargoyle", Pos::new(5, 11), false),
        ("gargoyle", Pos::new(6, 6), false),
    ] {
        let g = large(&[(name, 0, 5, 5)]);
        assert_eq!(
            g.validate(&ActionIntent::Piece {
                player: 0,
                source: id(&g, 5, 5),
                kind: ActionKind::Move,
                target
            })
            .is_ok(),
            valid
        );
    }
    for (name, near, far) in [("basilisk", (6, 6), (8, 8)), ("gargoyle", (5, 6), (5, 8))] {
        let mut g = large(&[
            (name, 0, 5, 5),
            ("pawn", 1, near.0, near.1),
            ("pawn", 1, far.0, far.1),
        ]);
        let source = id(&g, 5, 5);
        assert!(g
            .validate(&ActionIntent::Piece {
                player: 0,
                source,
                kind: ActionKind::Take,
                target: Pos::new(far.0, far.1)
            })
            .is_err());
        act(&mut g, source, ActionKind::Take, Pos::new(near.0, near.1));
        assert_eq!(g.piece(source).unwrap().position, Pos::new(near.0, near.1));
    }
}

#[test]
fn mermaid_combines_adjacent_steps_and_knight_leaps_for_move_and_take() {
    let g = large(&[("mermaid", 0, 5, 5)]);
    let source = id(&g, 5, 5);
    assert_eq!(
        g.legal_actions(source)
            .iter()
            .filter(|a| matches!(
                a,
                ActionIntent::Piece {
                    kind: ActionKind::Move,
                    ..
                }
            ))
            .count(),
        16
    );
    for target in [Pos::new(6, 6), Pos::new(6, 7)] {
        let mut g = large(&[("mermaid", 0, 5, 5), ("pawn", 1, target.x, target.y)]);
        let source = id(&g, 5, 5);
        act(&mut g, source, ActionKind::Take, target);
        assert_eq!(g.piece(source).unwrap().position, target);
    }
}

#[test]
fn new_pieces_have_catalogue_tags_and_demonic_death_rewards() {
    for name in ["basilisk", "gargoyle", "mermaid"] {
        let mut g = game(
            &[("rook", 0, 0, 0), (name, 1, 0, 3)],
            &["mana"],
            &["mana"],
            0,
        );
        let target = id(&g, 0, 3);
        assert!(g.has_tag(target, Tag::Demonic));
        assert_eq!(g.has_tag(target, Tag::Biological), name == "mermaid");
        assert_eq!(
            g.piece(target).unwrap().stats.transportable,
            if name == "mermaid" { Some(3) } else { None }
        );
        let source = id(&g, 0, 0);
        act(&mut g, source, ActionKind::Take, Pos::new(0, 3));
        assert_eq!(g.player(1).unwrap().mana, 21);
    }
}

#[test]
fn mermaid_haste_completes_summon_and_unlocks_portal_with_original_ownership() {
    let mut g = summoning();
    let source = id(&g, 2, 2);
    let portal = id(&g, 3, 3);
    let job = g.scheduled()[0].id;
    let intent = hasten(&g, source, job);
    assert!(g.legal_abilities(source).contains(&intent));
    let trace = g.apply(intent).unwrap();
    assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "witch");
    assert!(g.at(Pos::new(4, 3)).unwrap().state.acted);
    assert!(g.scheduled().is_empty());
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e,Event::ScheduledChanged{id,..} if *id==job)));
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e,Event::PieceSpawned{owner,..} if *owner==0)));
    pass(&mut g);
    assert!(!g.legal_abilities(portal).is_empty());
    assert!(g.legal_abilities(source).is_empty());
}

#[test]
fn haste_preview_replay_and_serialization_do_not_mutate_live_state() {
    let mut g = summoning();
    let intent = hasten(&g, id(&g, 2, 2), g.scheduled()[0].id);
    let before = g.snapshot();
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(intent.clone()).unwrap(), preview);
    assert_eq!(
        replay
            .apply(ron::from_str(&ron::to_string(&intent).unwrap()).unwrap())
            .unwrap(),
        preview
    );
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn mermaid_rejects_distant_hostile_missing_and_non_summon_jobs() {
    let mut distant = game(
        &[("portal", 0, 3, 3), ("mermaid", 0, 0, 0)],
        &["mana"],
        &["mana"],
        0,
    );
    let portal = id(&distant, 3, 3);
    cast(&mut distant, portal, "summon_witch", Some(Pos::new(4, 3)));
    pass(&mut distant);
    let before = distant.snapshot();
    let intent = hasten(&distant, id(&distant, 0, 0), distant.scheduled()[0].id);
    assert!(distant.apply(intent).is_err());
    assert_eq!(distant.snapshot(), before);
    let mut hostile = game(
        &[("portal", 0, 3, 3), ("mermaid", 1, 2, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let portal = id(&hostile, 3, 3);
    cast(&mut hostile, portal, "summon_witch", Some(Pos::new(4, 3)));
    assert!(hostile
        .validate(&hasten(
            &hostile,
            id(&hostile, 2, 2),
            hostile.scheduled()[0].id
        ))
        .is_err());
    let g = summoning();
    assert!(g.validate(&hasten(&g, id(&g, 2, 2), 999)).is_err());
    let mut non = game(
        &[("mandragora", 0, 3, 3), ("mermaid", 0, 2, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let mandragora = id(&non, 3, 3);
    cast(&mut non, mandragora, "harvest", Some(Pos::new(3, 3)));
    pass(&mut non);
    assert!(non
        .validate(&hasten(&non, id(&non, 2, 2), non.scheduled()[0].id))
        .is_err());
}

#[test]
fn cancelled_or_dead_portal_summons_cannot_be_selected() {
    let mut g = game(
        &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2)],
        &["cancel_event"],
        &["mana"],
        1,
    );
    let portal = id(&g, 3, 3);
    cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
    pass(&mut g);
    let job = g.scheduled()[0].id;
    play(&mut g, "cancel_event", CardChoice::Scheduled(job));
    assert!(g.validate(&hasten(&g, id(&g, 2, 2), job)).is_err());
    let mut g = game(
        &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2), ("rook", 1, 3, 7)],
        &["mana"],
        &["mana"],
        0,
    );
    let portal = id(&g, 3, 3);
    cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
    let rook = id(&g, 3, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(3, 3));
    assert!(g
        .validate(&hasten(&g, id(&g, 2, 2), g.scheduled()[0].id))
        .is_err());
}

#[test]
fn haste_respects_occupied_destination_and_finishes_the_job() {
    let mut g = game(
        &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2), ("rook", 1, 4, 7)],
        &["mana"],
        &["mana"],
        0,
    );
    let portal = id(&g, 3, 3);
    cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
    let rook = id(&g, 4, 7);
    act(&mut g, rook, ActionKind::Move, Pos::new(4, 3));
    g.apply(hasten(&g, id(&g, 2, 2), g.scheduled()[0].id))
        .unwrap();
    assert!(g.scheduled().is_empty());
    assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "rook");
}

#[test]
fn scheduled_selection_requires_explicit_job_and_valid_schema() {
    let mut g = summoning();
    let source = id(&g, 2, 2);
    assert!(g
        .apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "hasten".into(),
            target: Some(Pos::new(3, 3))
        })
        .is_err());
    let base: PieceArchetype = ron::from_str(include_str!("../assets/mermaid.ron")).unwrap();
    let portal = "(schema_version: 1, id: \"portal\", display_name: \"Portal\", symbol: 'P', action_rules: [])";
    let original = ron::to_string(&base).unwrap();
    assert!(Registry::load([("mermaid.ron", original.as_str()), ("portal.ron", portal)]).is_ok());
    for case in 0..3 {
        let mut p = base.clone();
        match case {
            0 => p.abilities[0].selection = Selection::One,
            1 => p.abilities[0].effects = vec![EffectDef::ModifyScheduledEvent(0)],
            _ => {
                p.abilities[0].effects = vec![EffectDef::Schedule {
                    after: 1,
                    effects: vec![EffectDef::ModifyScheduledEvent(2)],
                }]
            }
        }
        let s = ron::to_string(&p).unwrap();
        assert!(Registry::load([("mermaid.ron", s.as_str()), ("portal.ron", portal)]).is_err());
    }
}

#[test]
fn haste_failure_rolls_back_rescheduling_costs_spawn_and_cooldown() {
    use cchess_engine::content::{TriggerDef, TriggerEvent};
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "witch")
            .unwrap()
            .triggers
            .push(TriggerDef {
                id: "overflow".into(),
                event: TriggerEvent::Spawned,
                effects: vec![EffectDef::AddMana(1); 1100],
            })
    });
    let mut g = setup(
        reg,
        &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let portal = id(&g, 3, 3);
    cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
    pass(&mut g);
    let before = g.snapshot();
    let intent = hasten(&g, id(&g, 2, 2), g.scheduled()[0].id);
    assert_eq!(g.apply(intent), Err(RuleError::ResolutionLimit));
    assert_eq!(g.snapshot(), before);
}

#[test]
fn basilisk_gaze_affects_all_three_opponents_in_four_player_mode() {
    let mut g = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 14,
            height: 14,
        },
        [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .enumerate()
            .map(|(id, d)| Player::new(id as u8, d))
            .collect(),
        vec![
            Placement::new("basilisk", 0, Pos::new(6, 6)),
            Placement::new("gargoyle", 1, Pos::new(9, 6)),
            Placement::new("gargoyle", 2, Pos::new(6, 9)),
            Placement::new("gargoyle", 3, Pos::new(3, 6)),
        ],
    )
    .unwrap();
    let source = id(&g, 6, 6);
    cast(&mut g, source, "gaze", None);
    assert_eq!(
        g.pieces()
            .filter(|p| g.has_status(p.id, StatusKind::Deactivate))
            .count(),
        3
    );
}

#[test]
fn haste_reduces_long_delays_and_clamps_short_delays_to_now() {
    for delay in [2, 6] {
        let reg = registry(|pieces, _| {
            let p = pieces.iter_mut().find(|p| p.id == "portal").unwrap();
            p.abilities
                .iter_mut()
                .find(|a| a.id == "summon_witch")
                .unwrap()
                .effects = vec![EffectDef::Schedule {
                after: delay,
                effects: vec![EffectDef::SpawnTarget("witch".into())],
            }];
        });
        let mut g = setup(
            reg,
            &[("portal", 0, 3, 3), ("mermaid", 0, 2, 2)],
            &["mana"],
            &["mana"],
            0,
            0,
        );
        let portal = id(&g, 3, 3);
        cast(&mut g, portal, "summon_witch", Some(Pos::new(4, 3)));
        pass(&mut g);
        let job = g.scheduled()[0].clone();
        let due = job.due_tick.saturating_sub(2).max(g.owner_turn(0).unwrap());
        let trace = g.apply(hasten(&g, id(&g, 2, 2), job.id)).unwrap();
        assert!(trace
            .events
            .iter()
            .any(|e| matches!(e,Event::ScheduledChanged{after,..} if *after==due)));
        if delay == 2 {
            assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "witch");
        } else {
            assert_eq!(g.scheduled()[0].due_tick, due);
            assert!(g.at(Pos::new(4, 3)).is_none());
        }
    }
}
