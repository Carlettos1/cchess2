mod support;
use cchess_engine::{
    content::{EffectDef, PieceArchetype, Selection, Tag},
    geometry::SubDirection,
    state::Snapshot,
    *,
};
use support::*;

fn launch(g: &Game, source: u64, target: Pos, destination: Pos) -> ActionIntent {
    ActionIntent::Transport {
        player: g.turn().player,
        source,
        ability: "launch".into(),
        target,
        destination,
    }
}

#[test]
fn transport_supports_all_eight_directions_ranges_and_both_owners() {
    for direction in SubDirection::ALL {
        let (dx, dy) = direction.vector();
        for range in 1..=3 {
            for owner in [0, 1] {
                let mut g = game(
                    &[("catapult", 0, 3 - dy, 3 + dx), ("imp", owner, 3, 3)],
                    &["mana"],
                    &["mana"],
                    0,
                );
                let source = id(&g, 3 - dy, 3 + dx);
                let carried = id(&g, 3, 3);
                let destination = Pos::new(3 + dx * range, 3 + dy * range);
                let intent = launch(&g, source, Pos::new(3, 3), destination);
                assert!(g.legal_abilities(source).contains(&intent));
                g.apply(intent).unwrap();
                assert_eq!(g.piece(carried).unwrap().position, destination);
                assert_eq!(g.piece(carried).unwrap().owner, owner);
                assert!(!g.piece(carried).unwrap().state.acted);
            }
        }
    }
}

#[test]
fn collision_damages_both_and_stops_before_destroyed_blocker() {
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "imp")
            .unwrap()
            .base_stats
            .tough = 3
    });
    let mut g = setup(
        reg,
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2), ("wall", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let source = id(&g, 2, 2);
    let carried = id(&g, 3, 2);
    let result = g
        .apply(launch(&g, source, Pos::new(3, 2), Pos::new(6, 2)))
        .unwrap();
    assert!(g.at(Pos::new(5, 2)).is_none());
    assert_eq!(g.piece(carried).unwrap().position, Pos::new(4, 2));
    assert_eq!(g.piece(carried).unwrap().state.damage, 1);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| matches!(e, Event::DamageDealt { .. }))
            .count(),
        2
    );
    assert!(result.events.iter().any(|e|matches!(e,Event::TransportCollision{source:s,target,..} if *s==source && *target==carried)));
}

#[test]
fn immediate_collision_can_destroy_payload_and_launcher() {
    let mut g = game(
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    let carried = id(&g, 3, 2);
    g.apply(launch(&g, source, Pos::new(3, 2), Pos::new(1, 2)))
        .unwrap();
    assert!(g.piece(source).is_none());
    assert!(g.piece(carried).is_none());
    assert_eq!(g.player(0).unwrap().mana, 21);
}

#[test]
fn immune_blocker_stops_transport_without_taking_damage() {
    let mut g = game(
        &[
            ("catapult", 0, 2, 2),
            ("imp", 0, 3, 2),
            ("warlock", 1, 5, 2),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    g.apply(launch(&g, source, Pos::new(3, 2), Pos::new(6, 2)))
        .unwrap();
    assert_eq!(g.at(Pos::new(5, 2)).unwrap().state.damage, 0);
    assert!(g.at(Pos::new(3, 2)).is_none());
    assert!(g.at(Pos::new(4, 2)).is_none());
    assert_eq!(g.dead().last().unwrap().piece.position, Pos::new(4, 2));
}

#[test]
fn invalid_transport_is_atomic_and_incomplete_choices_are_rejected() {
    let g = game(
        &[
            ("catapult", 0, 2, 2),
            ("imp", 0, 3, 2),
            ("wall", 0, 2, 3),
            ("warlock", 1, 3, 3),
            ("pawn", 0, 5, 5),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    for (target, destination) in [
        ((3, 2), (3, 2)),
        ((3, 2), (7, 2)),
        ((3, 2), (-1, 2)),
        ((3, 2), (5, 3)),
        ((2, 3), (2, 4)),
        ((3, 3), (3, 4)),
        ((5, 5), (5, 6)),
        ((2, 2), (2, 4)),
    ] {
        let mut copy = g.clone();
        assert!(copy
            .apply(launch(
                &g,
                source,
                Pos::new(target.0, target.1),
                Pos::new(destination.0, destination.1)
            ))
            .is_err());
        assert_eq!(copy.snapshot(), g.snapshot());
    }
    let mut copy = g.clone();
    assert!(copy
        .apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "launch".into(),
            target: Some(Pos::new(3, 2))
        })
        .is_err());
    assert_eq!(copy.snapshot(), g.snapshot());
}

#[test]
fn transport_strength_threshold_and_immunity_are_independent() {
    for (weight, immune, valid) in [(3, false, true), (4, false, false), (1, true, false)] {
        let reg = registry(|pieces, _| {
            let p = pieces.iter_mut().find(|p| p.id == "imp").unwrap();
            p.base_stats.transportable = Some(weight);
            if immune {
                p.tags.insert(Tag::Immune);
            }
        });
        let g = setup(
            reg,
            &[("catapult", 0, 2, 2), ("imp", 0, 3, 2)],
            &["mana"],
            &["mana"],
            0,
            0,
        );
        assert_eq!(
            g.validate(&launch(&g, id(&g, 2, 2), Pos::new(3, 2), Pos::new(5, 2)))
                .is_ok(),
            valid
        );
    }
}

#[test]
fn transport_preview_replay_serialization_and_cooldown_are_consistent() {
    let mut g = game(
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    let intent = launch(&g, source, Pos::new(3, 2), Pos::new(3, 3));
    let before = g.snapshot();
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    let wire = ron::to_string(&intent).unwrap();
    assert_eq!(
        replay.apply(ron::from_str(&wire).unwrap()).unwrap(),
        preview
    );
    assert_eq!(g.apply(intent).unwrap(), preview);
    assert_eq!(replay.snapshot(), g.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
    pass(&mut g);
    assert!(g
        .validate(&launch(&g, source, Pos::new(3, 3), Pos::new(5, 3)))
        .is_err());
}

#[test]
fn transported_friendly_pawn_promotes_before_turn_transfer() {
    let mut g = game(
        &[("catapult", 0, 2, 4), ("pawn", 0, 3, 4)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 4);
    let carried = id(&g, 3, 4);
    g.apply(launch(&g, source, Pos::new(3, 4), Pos::new(3, 7)))
        .unwrap();
    assert_eq!(g.pending_promotion().unwrap().piece, carried);
    assert_eq!(g.turn().player, 0);
}

#[test]
fn collision_royal_removal_uses_normal_victory_and_death_hooks() {
    // The built-in king is Immune; use a vulnerable Royal to exercise victory.
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "king")
            .unwrap()
            .tags
            .remove(&Tag::Immune);
    });
    let mut g = setup(
        reg,
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2), ("king", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let source = id(&g, 2, 2);
    g.apply(launch(&g, source, Pos::new(3, 2), Pos::new(6, 2)))
        .unwrap();
    assert_eq!(g.winner(), Some(0));
    assert_eq!(g.player(0).unwrap().mana, 21);
}

#[test]
fn imp_moves_as_knight_without_take_or_attack() {
    let g = game(
        &[("imp", 0, 3, 3), ("pawn", 1, 4, 5)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    for (kind, target, valid) in [
        (ActionKind::Move, Pos::new(2, 5), true),
        (ActionKind::Move, Pos::new(3, 4), false),
        (ActionKind::Take, Pos::new(4, 5), false),
        (ActionKind::Attack, Pos::new(4, 5), false),
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
fn imp_draw_uses_no_mana_enforces_cooldown_and_handles_empty_deck() {
    let mut g = game(&[("imp", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    cast(&mut g, source, "draw", Some(Pos::new(3, 3)));
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 1);
    assert_eq!(g.player(0).unwrap().mana, 20);
    pass(&mut g);
    assert!(!g.legal_abilities(source).is_empty());
    let trace = cast(&mut g, source, "draw", Some(Pos::new(3, 3)));
    assert_eq!(g.player(0).unwrap().cards.hand.len(), 1);
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e, Event::DrawSkipped { .. })));
}

#[test]
fn portal_can_summon_imp_on_next_owner_turn() {
    let mut g = game(&[("portal", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    cast(&mut g, source, "summon_imp", Some(Pos::new(4, 3)));
    pass(&mut g);
    assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "imp");
    assert!(g.at(Pos::new(4, 3)).unwrap().state.acted);
}

#[test]
fn transport_asset_validation_rejects_missing_choice_and_nested_effects() {
    let base: PieceArchetype = ron::from_str(include_str!("../assets/catapult.ron")).unwrap();
    for case in 0..4 {
        let mut piece = base.clone();
        match case {
            0 => piece.abilities[0].selection = Selection::One,
            1 => {
                piece.abilities[0].effects = vec![EffectDef::Schedule {
                    after: 1,
                    effects: piece.abilities[0].effects.clone(),
                }]
            }
            2 => {
                piece.abilities[0].effects = vec![EffectDef::Transport {
                    max_range: 0,
                    collision_damage: 1,
                }]
            }
            _ => {
                piece.abilities[0].effects = vec![EffectDef::Transport {
                    max_range: 3,
                    collision_damage: 0,
                }]
            }
        }
        let text = ron::to_string(&piece).unwrap();
        assert!(Registry::load([("bad.ron", text.as_str())]).is_err());
    }
}

#[test]
fn collision_trigger_failure_rolls_back_movement_deaths_costs_and_cooldown() {
    use cchess_engine::content::{TriggerDef, TriggerEvent};
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
            })
    });
    let mut g = setup(
        reg,
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2), ("wall", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let before = g.snapshot();
    assert_eq!(
        g.apply(launch(&g, id(&g, 2, 2), Pos::new(3, 2), Pos::new(6, 2))),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g.snapshot(), before);
}

#[test]
fn transport_preserves_statuses_and_does_not_require_payload_to_be_active() {
    use cchess_engine::{
        content::{TriggerDef, TriggerEvent},
        state::StatusKind,
    };
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "imp")
            .unwrap()
            .triggers
            .push(TriggerDef {
                id: "frozen".into(),
                event: TriggerEvent::Spawned,
                effects: vec![EffectDef::AddStatus {
                    kind: StatusKind::Ice,
                    duration: 3,
                }],
            })
    });
    let mut g = setup(
        reg,
        &[("catapult", 0, 2, 2), ("imp", 0, 3, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let carried = id(&g, 3, 2);
    let statuses = g.piece(carried).unwrap().statuses.clone();
    assert!(g.legal_actions(carried).is_empty());
    g.apply(launch(&g, id(&g, 2, 2), Pos::new(3, 2), Pos::new(6, 2)))
        .unwrap();
    assert_eq!(g.piece(carried).unwrap().position, Pos::new(6, 2));
    assert_eq!(g.piece(carried).unwrap().statuses, statuses);
}
