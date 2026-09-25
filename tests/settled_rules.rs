mod support;
use cchess_engine::{clock::ClockUnit, content::EffectDef, geometry::Pattern, state::Snapshot, *};
use support::*;

#[test]
fn normal_economy_is_one_mana_then_one_per_two_personal_turns() {
    let mut g = Game::classical(Registry::built_in().unwrap()).unwrap();
    for owner in 0..2 {
        assert_eq!(g.player(owner).unwrap().mana, 1);
    }
    for ordinal in 1..=7 {
        for owner in 0..2 {
            assert_eq!(g.turn().player, owner);
            assert_eq!(g.owner_turn(owner), Some(ordinal));
            assert_eq!(
                g.player(owner).unwrap().mana,
                1 + ((ordinal - 1) / 2) as u32
            );
            pass(&mut g);
        }
    }
}

#[test]
fn draw_one_per_turn_and_extra_draw_actions_accumulate() {
    let mut g = Game::classical(Registry::built_in().unwrap()).unwrap();
    let opening = g.player(0).unwrap().cards.hand.len();
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().cards.hand.len(), opening + 1);
    let mut g = game(&[("imp", 0, 3, 3)], &["mana", "mana", "mana"], &["mana"], 0);
    let imp = id(&g, 3, 3);
    cast(&mut g, imp, "draw", Some(Pos::new(3, 3)));
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 1);
    pass(&mut g);
    cast(&mut g, imp, "draw", Some(Pos::new(3, 3)));
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 2);
}

#[test]
fn cross_board_has_160_cells_and_all_four_corners_are_unplayable() {
    let g = Game::four_player(Registry::built_in().unwrap()).unwrap();
    assert_eq!(
        g.bounds().positions().filter(|p| g.is_playable(*p)).count(),
        160
    );
    for x in 0..14 {
        for y in 0..14 {
            assert_eq!(
                g.is_playable(Pos::new(x, y)),
                (3..11).contains(&x) || (3..11).contains(&y)
            );
        }
    }
    assert!(g.pieces().all(|p| g.is_playable(p.position)));
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn rays_cannot_cross_removed_corners() {
    let reg = registry(|pieces, cards| {
        let rook = pieces.iter_mut().find(|p| p.id == "rook").unwrap();
        for rule in &mut rook.action_rules {
            rule.target.pattern =
                Pattern::InfiniteRay(cchess_engine::geometry::SubDirection::ALL.to_vec());
        }
        let card = cards.iter_mut().find(|c| c.id == "summon_wall").unwrap();
        card.cost.mana = 0;
    });
    let mut g = Game::four_player(reg).unwrap();
    pass(&mut g);
    let rook = id(&g, 0, 3);
    for target in [Pos::new(1, 2), Pos::new(2, 1), Pos::new(3, 0)] {
        for kind in [ActionKind::Move, ActionKind::Take] {
            assert!(g
                .validate(&ActionIntent::Piece {
                    player: 1,
                    source: rook,
                    kind,
                    target
                })
                .is_err());
        }
    }
    assert!(g
        .validate(&ActionIntent::Ability {
            player: 1,
            source: rook,
            ability: "throw".into(),
            target: Some(Pos::new(-1, 3))
        })
        .is_err());
}

#[test]
fn start_of_movement_death_interrupts_action_without_undoing_the_clock() {
    let reg = registry(|pieces, _| {
        let pawn = pieces.iter_mut().find(|p| p.id == "pawn").unwrap();
        pawn.triggers = vec![cchess_engine::content::TriggerDef {
            id: "doomed".into(),
            event: cchess_engine::content::TriggerEvent::Spawned,
            effects: vec![EffectDef::ScheduleOn {
                clock: ClockUnit::Movements,
                after: 1,
                effects: vec![EffectDef::RemoveSource],
            }],
        }];
    });
    let mut g = setup(reg, &[("pawn", 0, 3, 1)], &["mana"], &["mana"], 0, 0);
    let pawn = id(&g, 3, 1);
    let trace = act(&mut g, pawn, ActionKind::Move, Pos::new(3, 2));
    assert!(g.piece(pawn).is_none());
    assert_eq!(g.clock().movements, 1);
    assert_eq!(g.turn().remaining_actions, 1);
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::CostsPaid { .. })));
    assert!(g.scheduled().is_empty());
}

#[test]
fn cross_board_end_pattern_stops_at_the_arm_edge_and_spawning_respects_mask() {
    use cchess_engine::{
        geometry::Relative,
        targeting::{resolve, TargetFilter, TargetOrigin, TargetSpec},
    };
    let g = Game::four_player(Registry::built_in().unwrap()).unwrap();
    let source = id(&g, 0, 3);
    let spec = TargetSpec {
        origin: TargetOrigin::Source,
        pattern: Pattern::EndOfBoard(Relative::L).normalize().unwrap(),
        filters: vec![],
        count: 1,
    };
    // Owner 1 faces east; left is north. a11 is its arm's end, not a14.
    assert_eq!(resolve(&g, source, 1, &spec), vec![Pos::new(0, 10)]);
    let spec = TargetSpec {
        origin: TargetOrigin::Board,
        pattern: Pattern::SelfOnly.normalize().unwrap(),
        filters: vec![TargetFilter::Empty],
        count: 1,
    };
    assert!(resolve(&g, source, 1, &spec)
        .into_iter()
        .all(|p| g.is_playable(p)));
}

#[test]
fn area_cast_reselects_targets_after_movement_start_spawns() {
    use cchess_engine::{
        cards::CardChoice,
        content::{TriggerDef, TriggerEvent},
        geometry::Relative,
        state::StatusKind,
    };
    let reg = registry(|pieces, _| {
        let pawn = pieces.iter_mut().find(|p| p.id == "pawn").unwrap();
        pawn.triggers = vec![TriggerDef {
            id: "reinforce".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::ScheduleOn {
                clock: ClockUnit::Movements,
                after: 2,
                effects: vec![EffectDef::SpawnRelative {
                    archetype: "wall".into(),
                    directions: vec![Relative::R],
                }],
            }],
        }];
    });
    let mut g = setup(
        reg,
        &[("magician", 0, 1, 1), ("pawn", 1, 3, 3)],
        &["ice"],
        &["mana"],
        1,
        0,
    );
    play(&mut g, "ice", CardChoice::None);
    let caster = id(&g, 1, 1);
    cast(&mut g, caster, "ice", None);
    let wall = id(&g, 2, 3);
    assert!(g.has_status(wall, StatusKind::Ice));
}
