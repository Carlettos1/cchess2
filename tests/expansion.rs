mod support;
use cchess_engine::{
    cards::CardChoice,
    geometry::Relative,
    state::{Placement, Player, Snapshot},
    *,
};
use support::*;

#[test]
fn seeded_rays_preview_replay_and_rejected_targets_do_not_consume_rng() {
    let mut g = game(&[("crazy_pawn", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    let before = g.snapshot();
    let direction = g.random_direction(source).unwrap();
    let legal = g.legal_actions(source);
    assert_eq!(g.snapshot(), before);
    let moves: Vec<_> = legal
        .into_iter()
        .filter(|i| matches!(i, ActionIntent::Piece { .. }))
        .collect();
    assert_eq!(moves.len(), 2);
    for intent in &moves {
        let ActionIntent::Piece { target, .. } = intent else {
            unreachable!()
        };
        assert_eq!(
            ((target.x - 3).signum(), (target.y - 3).signum()),
            direction
        );
    }
    let invalid = ActionIntent::Piece {
        player: 0,
        source,
        kind: ActionKind::Move,
        target: Pos::new(3 - direction.0, 3 - direction.1),
    };
    assert!(g.apply(invalid).is_err());
    assert_eq!(g.snapshot(), before);
    let preview = g.preview(&moves[0]).unwrap();
    assert_eq!(g.preview(&moves[0]).unwrap(), preview);
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(moves[0].clone()).unwrap(), preview);
    assert_eq!(replay.apply(moves[0].clone()).unwrap(), preview);
    assert_eq!(replay.snapshot(), g.snapshot());
    assert!(preview
        .events
        .iter()
        .any(|e| matches!(e,Event::RandomDirectionChosen { direction:d,.. } if *d==direction)));
    let saved = ron::to_string(&g.snapshot()).unwrap();
    assert_eq!(ron::from_str::<Snapshot>(&saved).unwrap(), g.snapshot());
}

#[test]
fn random_rays_respect_blockers_and_capture_on_the_only_open_ray() {
    let mut placements = vec![("crazy_pawn", 0, 3, 3), ("pawn", 1, 3, 5)];
    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1)] {
        placements.push(("wall", 0, 3 + dx, 3 + dy));
    }
    let mut g = game(&placements, &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    assert_eq!(g.random_direction(source), Some((0, 1)));
    assert_eq!(
        g.legal_actions(source)
            .iter()
            .filter(|a| matches!(a, ActionIntent::Piece { .. }))
            .count(),
        2
    );
    act(&mut g, source, ActionKind::Take, Pos::new(3, 5));
    assert_eq!(g.piece(source).unwrap().position, Pos::new(3, 5));
}

#[test]
fn seeds_produce_different_directions_and_cannot_change_after_play() {
    let g = game(&[("crazy_pawn", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    let directions: std::collections::BTreeSet<_> = (0..32)
        .map(|seed| g.clone().with_seed(seed).unwrap().random_direction(source))
        .collect();
    assert!(directions.len() > 4);
    let mut played = g;
    pass(&mut played);
    assert!(played.with_seed(1).is_err());
}

#[test]
fn sacrifice_removes_without_damage_then_draws_two_next_owner_turn() {
    let mut g = game(
        &[("crazy_pawn", 0, 3, 3)],
        &["mana", "fire", "ice"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    let trace = cast(&mut g, source, "sacrifice", Some(Pos::new(3, 3)));
    assert!(g.piece(source).is_none());
    assert!(!trace
        .events
        .iter()
        .any(|e| matches!(e, Event::DamageDealt { .. })));
    assert!(g.player(0).unwrap().cards.hand.is_empty());
    assert_eq!(g.scheduled().len(), 1);
    pass(&mut g);
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 2);
    assert!(g.scheduled().is_empty());
}

#[test]
fn cannon_uses_square_range_without_moving_or_capturing() {
    let mut g = game(
        &[
            ("cannon", 0, 0, 0),
            ("wall", 0, 1, 1),
            ("pawn", 1, 5, 5),
            ("pawn", 1, 6, 0),
            ("paladin", 1, 2, 0),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 0, 0);
    for (kind, target) in [
        (ActionKind::Take, Pos::new(5, 5)),
        (ActionKind::Attack, Pos::new(6, 0)),
        (ActionKind::Attack, Pos::new(2, 0)),
        (ActionKind::Move, Pos::new(2, 0)),
    ] {
        assert!(g
            .validate(&ActionIntent::Piece {
                player: 0,
                source,
                kind,
                target
            })
            .is_err());
    }
    act(&mut g, source, ActionKind::Attack, Pos::new(5, 5));
    assert!(g.at(Pos::new(5, 5)).is_none());
    assert_eq!(g.piece(source).unwrap().position, Pos::new(0, 0));
}

#[test]
fn broadside_rotates_and_hits_six_sides_including_allies() {
    for forward in [Direction::N, Direction::E, Direction::S, Direction::W] {
        let mut places = vec![Placement::new("ship", 0, Pos::new(3, 3))];
        for (index, r) in [
            Relative::F,
            Relative::FR,
            Relative::R,
            Relative::BR,
            Relative::B,
            Relative::BL,
            Relative::L,
            Relative::FL,
        ]
        .iter()
        .enumerate()
        {
            let (dx, dy) = r.absolute(forward);
            places.push(Placement::new(
                "wall",
                (index % 2) as u8,
                Pos::new(3 + dx, 3 + dy),
            ));
        }
        let mut g = Game::new(
            Registry::built_in().unwrap(),
            Bounds {
                width: 8,
                height: 8,
            },
            vec![Player::new(0, forward), Player::new(1, Direction::S)],
            places,
        )
        .unwrap();
        let source = id(&g, 3, 3);
        cast(&mut g, source, "broadside", None);
        assert_eq!(g.pieces().count(), 3);
        for r in [Relative::F, Relative::B] {
            let (dx, dy) = r.absolute(forward);
            assert!(g.at(Pos::new(3 + dx, 3 + dy)).is_some());
        }
    }
}

#[test]
fn ship_ray_is_blocked_and_take_is_adjacent() {
    let g = game(
        &[
            ("ship", 0, 3, 3),
            ("wall", 0, 4, 4),
            ("pawn", 1, 3, 5),
            ("pawn", 1, 4, 3),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    for (kind, target, valid) in [
        (ActionKind::Move, Pos::new(0, 0), true),
        (ActionKind::Move, Pos::new(5, 5), false),
        (ActionKind::Take, Pos::new(3, 5), false),
        (ActionKind::Take, Pos::new(4, 3), true),
    ] {
        assert_eq!(
            g.validate(&ActionIntent::Piece {
                player: 0,
                source,
                kind,
                target
            })
            .is_ok(),
            valid
        );
    }
}

#[test]
fn warlock_spawns_only_on_empty_buildable_adjacent_magic_tiles() {
    let mut g = game(
        &[("warlock", 0, 3, 3), ("wall", 0, 2, 2)],
        &["mana"],
        &["mana"],
        0,
    )
    .with_terrain(
        vec![
            Pos::new(2, 2),
            Pos::new(2, 3),
            Pos::new(4, 3),
            Pos::new(7, 7),
        ],
        vec![Pos::new(4, 3)],
    )
    .unwrap();
    let source = id(&g, 3, 3);
    cast(&mut g, source, "portals", None);
    assert_eq!(g.at(Pos::new(2, 3)).unwrap().archetype, "portal");
    assert_eq!(g.pieces().filter(|p| p.archetype == "portal").count(), 1);
    assert!(g.at(Pos::new(2, 3)).unwrap().state.acted);
    assert_eq!(g.player(0).unwrap().mana, 17);
}

#[test]
fn warlock_without_magic_targets_fails_atomically() {
    let mut g = game(&[("warlock", 0, 3, 3)], &["mana"], &["mana"], 0);
    let before = g.snapshot();
    let source = id(&g, 3, 3);
    assert!(g
        .apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "portals".into(),
            target: None
        })
        .is_err());
    assert_eq!(g.snapshot(), before);
}

#[test]
fn portal_locks_all_summons_until_completion_and_rechecks_occupancy() {
    let mut g = game(
        &[("portal", 0, 3, 3), ("rook", 1, 4, 7)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    cast(&mut g, source, "summon_succubus", Some(Pos::new(4, 3)));
    let rook = id(&g, 4, 7);
    act(&mut g, rook, ActionKind::Move, Pos::new(4, 3));
    assert!(g.legal_actions(source).is_empty());
    pass(&mut g);
    pass(&mut g);
    assert!(g.scheduled().is_empty());
    assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "rook");
    assert!(!g.legal_actions(source).is_empty());
}

#[test]
fn portal_summon_survives_source_death_and_preserves_owner() {
    let mut g = game(
        &[("portal", 0, 3, 3), ("rook", 1, 3, 7)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    cast(&mut g, source, "summon_mandragora", Some(Pos::new(4, 3)));
    let rook = id(&g, 3, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(3, 3));
    let summoned = g.at(Pos::new(4, 3)).unwrap();
    assert_eq!(summoned.archetype, "mandragora");
    assert_eq!(summoned.owner, 0);
    assert!(summoned.state.acted);
    assert_eq!(g.player(0).unwrap().mana, 21);
}

#[test]
fn cancelling_pending_summon_unlocks_portal() {
    let mut g = game(&[("portal", 0, 3, 3)], &["cancel_event"], &["mana"], 1);
    let source = id(&g, 3, 3);
    cast(&mut g, source, "summon_witch", Some(Pos::new(4, 3)));
    pass(&mut g);
    assert!(g.legal_actions(source).is_empty());
    let job = g.scheduled()[0].id;
    play(&mut g, "cancel_event", CardChoice::Scheduled(job));
    assert!(!g.legal_actions(source).is_empty());
}

#[test]
fn failed_random_capture_rolls_back_rng_costs_and_removal() {
    use cchess_engine::content::{EffectDef, TriggerDef, TriggerEvent};
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "wall")
            .unwrap()
            .triggers
            .push(TriggerDef {
                id: "overflow".into(),
                event: TriggerEvent::Removed,
                effects: vec![EffectDef::AddMana(1); 1100],
            });
    });
    let mut placements = vec![("crazy_pawn", 0, 3, 3), ("wall", 1, 3, 4)];
    for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (1, 1)] {
        placements.push(("wall", 0, 3 + dx, 3 + dy));
    }
    let mut g = setup(reg, &placements, &["mana"], &["mana"], 0, 0);
    let source = id(&g, 3, 3);
    let before = g.snapshot();
    assert_eq!(
        g.apply(ActionIntent::Piece {
            player: 0,
            source,
            kind: ActionKind::Take,
            target: Pos::new(3, 4)
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g.snapshot(), before);
}

#[test]
fn broadside_respects_warlock_immunity_but_normal_capture_can_remove_it() {
    let mut g = game(
        &[("ship", 0, 3, 3), ("warlock", 1, 4, 3), ("wall", 1, 2, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let ship = id(&g, 3, 3);
    cast(&mut g, ship, "broadside", None);
    assert!(g.at(Pos::new(4, 3)).is_some());
    assert!(g.at(Pos::new(2, 3)).is_none());
    pass(&mut g);
    act(&mut g, ship, ActionKind::Take, Pos::new(4, 3));
    assert_eq!(g.player(1).unwrap().mana, 21);
}
