mod support;
use cchess_engine::{
    content::{EffectDef, PieceArchetype, Selection, Tag, TriggerDef, TriggerEvent},
    geometry::SubDirection,
    state::{Placement, Player, Snapshot, StatusKind},
    *,
};
use support::*;

fn shove(g: &Game, source: u64, target: Pos) -> ActionIntent {
    ActionIntent::Ability {
        player: g.turn().player,
        source,
        ability: "shove".into(),
        target: Some(target),
    }
}

#[test]
fn catalogue_stats_taxicab_moves_and_heroic_attack_protection() {
    for (name, tough) in [("golem", 3), ("ogre", 2)] {
        let g = game(
            &[(name, 0, 3, 3), ("wall", 0, 3, 4)],
            &["mana"],
            &["mana"],
            0,
        );
        let source = id(&g, 3, 3);
        assert_eq!(g.piece(source).unwrap().stats.tough, tough);
        assert!(g.has_tag(source, Tag::Demonic));
        assert_eq!(g.has_tag(source, Tag::Heroic), name == "golem");
        for (target, valid) in [
            (Pos::new(3, 5), true),
            (Pos::new(4, 4), true),
            (Pos::new(5, 4), false),
        ] {
            assert_eq!(
                g.validate(&ActionIntent::Piece {
                    player: 0,
                    source,
                    kind: ActionKind::Move,
                    target
                })
                .is_ok(),
                valid
            );
        }
        let mut g = game(
            &[("cannon", 0, 0, 0), (name, 1, 2, 0)],
            &["mana"],
            &["mana"],
            0,
        );
        let intent = ActionIntent::Piece {
            player: 0,
            source: id(&g, 0, 0),
            kind: ActionKind::Attack,
            target: Pos::new(2, 0),
        };
        assert_eq!(g.apply(intent).is_ok(), name == "ogre");
    }
}

#[test]
fn toughness_requires_repeated_damage_before_take_moves_and_death_reward() {
    for (name, tough) in [("golem", 3), ("ogre", 2)] {
        let mut g = game(
            &[("rook", 0, 0, 0), (name, 1, 0, 3)],
            &["mana"],
            &["mana"],
            0,
        );
        let source = id(&g, 0, 0);
        for damage in 1..=tough {
            act(&mut g, source, ActionKind::Take, Pos::new(0, 3));
            if damage < tough {
                assert_eq!(g.piece(source).unwrap().position, Pos::new(0, 0));
                assert_eq!(g.at(Pos::new(0, 3)).unwrap().state.damage, damage);
                assert_eq!(g.player(1).unwrap().mana, 20);
                pass(&mut g);
            }
        }
        assert_eq!(g.piece(source).unwrap().position, Pos::new(0, 3));
        assert_eq!(g.player(1).unwrap().mana, 21);
    }
}

#[test]
fn shove_pushes_three_squares_in_all_directions_without_transport_limits() {
    for d in SubDirection::ALL {
        let (dx, dy) = d.vector();
        for owner in [0, 1] {
            let mut g = Game::new(
                Registry::built_in().unwrap(),
                Bounds {
                    width: 10,
                    height: 10,
                },
                vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
                vec![
                    Placement::new("ogre", 0, Pos::new(4, 4)),
                    Placement::new("golem", owner, Pos::new(4 + dx, 4 + dy)),
                ],
            )
            .unwrap();
            let source = id(&g, 4, 4);
            let target = id(&g, 4 + dx, 4 + dy);
            assert_eq!(g.piece(target).unwrap().stats.transportable, None);
            g.apply(shove(&g, source, Pos::new(4 + dx, 4 + dy)))
                .unwrap();
            assert_eq!(
                g.piece(target).unwrap().position,
                Pos::new(4 + 4 * dx, 4 + 4 * dy)
            );
            assert_eq!(g.piece(target).unwrap().owner, owner);
            assert!(!g.piece(target).unwrap().state.acted);
            assert_eq!(g.piece(source).unwrap().position, Pos::new(4, 4));
        }
    }
}

#[test]
fn shove_collision_stops_before_blocker_and_damages_both() {
    let mut g = game(
        &[("ogre", 0, 2, 2), ("golem", 1, 3, 2), ("wall", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    let target = id(&g, 3, 2);
    g.apply(shove(&g, source, Pos::new(3, 2))).unwrap();
    assert_eq!(g.piece(target).unwrap().position, Pos::new(4, 2));
    assert_eq!(g.piece(target).unwrap().state.damage, 1);
    assert_eq!(g.piece(source).unwrap().state.damage, 0);
    assert!(g.at(Pos::new(5, 2)).is_none());
}

#[test]
fn protected_blocker_still_stops_push_and_payload_dies_at_impact() {
    let mut g = game(
        &[("ogre", 0, 2, 2), ("imp", 0, 3, 2), ("warlock", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    let target = id(&g, 3, 2);
    g.apply(shove(&g, source, Pos::new(3, 2))).unwrap();
    assert!(g.piece(target).is_none());
    assert_eq!(g.dead().last().unwrap().piece.position, Pos::new(4, 2));
    assert_eq!(g.at(Pos::new(5, 2)).unwrap().state.damage, 0);
    assert_eq!(g.player(0).unwrap().mana, 21);
}

#[test]
fn edge_clamps_distance_without_damage_and_outward_edge_push_fails_atomically() {
    let mut g = game(
        &[("ogre", 0, 4, 2), ("golem", 0, 5, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 4, 2);
    let target = id(&g, 5, 2);
    g.apply(shove(&g, source, Pos::new(5, 2))).unwrap();
    assert_eq!(g.piece(target).unwrap().position, Pos::new(7, 2));
    assert_eq!(g.piece(target).unwrap().state.damage, 0);
    let mut g = game(
        &[("ogre", 0, 6, 2), ("golem", 0, 7, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let before = g.snapshot();
    assert!(g.apply(shove(&g, id(&g, 6, 2), Pos::new(7, 2))).is_err());
    assert_eq!(g.snapshot(), before);
}

#[test]
fn empty_distant_self_and_immune_targets_are_rejected() {
    let mut g = game(
        &[("ogre", 0, 3, 3), ("golem", 0, 3, 5), ("warlock", 1, 4, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    let before = g.snapshot();
    for target in [
        Pos::new(2, 3),
        Pos::new(3, 5),
        Pos::new(3, 3),
        Pos::new(4, 3),
    ] {
        assert!(g.apply(shove(&g, source, target)).is_err());
        assert_eq!(g.snapshot(), before);
    }
}

#[test]
fn shove_preserves_disabled_payload_status_and_rejects_disabled_ogre() {
    for disabled in ["golem", "ogre"] {
        let reg = registry(|pieces, _| {
            pieces
                .iter_mut()
                .find(|p| p.id == disabled)
                .unwrap()
                .triggers
                .push(TriggerDef {
                    id: "ice".into(),
                    event: TriggerEvent::Spawned,
                    effects: vec![EffectDef::AddStatus {
                        kind: StatusKind::Ice,
                        duration: 3,
                    }],
                })
        });
        let mut g = setup(
            reg,
            &[("ogre", 0, 2, 2), ("golem", 0, 3, 2)],
            &["mana"],
            &["mana"],
            0,
            0,
        );
        let source = id(&g, 2, 2);
        let target = id(&g, 3, 2);
        let result = g.apply(shove(&g, source, Pos::new(3, 2)));
        assert_eq!(result.is_ok(), disabled == "golem");
        if disabled == "golem" {
            assert!(g.has_status(target, StatusKind::Ice));
            assert_eq!(g.piece(target).unwrap().position, Pos::new(6, 2));
        }
    }
}

#[test]
fn shove_preview_replay_serialization_and_cooldown_are_consistent() {
    let mut g = game(
        &[("ogre", 0, 2, 2), ("golem", 0, 3, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 2, 2);
    let intent = shove(&g, source, Pos::new(3, 2));
    let before = g.snapshot();
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(intent.clone()).unwrap(), preview);
    assert_eq!(replay.apply(intent).unwrap(), preview);
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
    pass(&mut g);
    assert_eq!(
        g.piece(source).unwrap().state.cooldowns["shove"].remaining,
        1
    );
}

#[test]
fn shove_promotes_friendly_and_enemy_pawns_on_their_respective_edges() {
    for (owner, sy, ty, end) in [(0, 4, 5, 7), (1, 3, 2, 0)] {
        let mut g = game(
            &[("ogre", 0, 3, sy), ("pawn", owner, 3, ty)],
            &["mana"],
            &["mana"],
            0,
        );
        let source = id(&g, 3, sy);
        let target = id(&g, 3, ty);
        g.apply(shove(&g, source, Pos::new(3, ty))).unwrap();
        assert_eq!(g.piece(target).unwrap().position, Pos::new(3, end));
        assert_eq!(g.pending_promotion().unwrap().piece, target);
        assert_eq!(g.turn().player, owner);
    }
}

#[test]
fn push_schema_rejects_zero_range_damage_and_unsupported_selection() {
    let base: PieceArchetype = ron::from_str(include_str!("../assets/ogre.ron")).unwrap();
    for case in 0..4 {
        let mut p = base.clone();
        match case {
            0 => {
                p.abilities[0].effects = vec![EffectDef::Push {
                    distance: 0,
                    collision_damage: 1,
                }]
            }
            1 => {
                p.abilities[0].effects = vec![EffectDef::Push {
                    distance: 3,
                    collision_damage: 0,
                }]
            }
            2 => p.abilities[0].selection = Selection::All,
            _ => {
                p.abilities[0].effects = vec![EffectDef::Schedule {
                    after: 1,
                    effects: p.abilities[0].effects.clone(),
                }]
            }
        }
        let s = ron::to_string(&p).unwrap();
        assert!(Registry::load([("ogre.ron", s.as_str())]).is_err());
    }
}

#[test]
fn collision_trigger_failure_rolls_back_entire_shove() {
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
        &[("ogre", 0, 2, 2), ("golem", 0, 3, 2), ("wall", 1, 5, 2)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let before = g.snapshot();
    let intent = shove(&g, id(&g, 2, 2), Pos::new(3, 2));
    assert_eq!(g.apply(intent), Err(RuleError::ResolutionLimit));
    assert_eq!(g.snapshot(), before);
}

#[test]
fn shove_collision_integrates_with_four_player_elimination() {
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "king")
            .unwrap()
            .tags
            .remove(&Tag::Immune);
    });
    let mut g = Game::new(
        reg,
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
            Placement::new("ogre", 0, Pos::new(5, 5)),
            Placement::new("golem", 1, Pos::new(6, 5)),
            Placement::new("king", 2, Pos::new(8, 5)),
            Placement::new("king", 0, Pos::new(10, 10)),
            Placement::new("king", 1, Pos::new(10, 9)),
            Placement::new("king", 3, Pos::new(3, 10)),
        ],
    )
    .unwrap();
    let source = id(&g, 5, 5);
    g.apply(shove(&g, source, Pos::new(6, 5))).unwrap();
    assert!(g.is_eliminated(2));
    assert!(!g.is_over());
    assert_eq!(g.turn().player, 1);
    assert_eq!(g.at(Pos::new(7, 5)).unwrap().archetype, "golem");
}

#[test]
fn portals_summon_golem_and_ogre_with_their_stats_and_spawn_lock() {
    for (name, delay, tough) in [("golem", 4, 3), ("ogre", 3, 2)] {
        let mut g = game(&[("portal", 0, 3, 3)], &["mana"], &["mana"], 0);
        let source = id(&g, 3, 3);
        cast(
            &mut g,
            source,
            &format!("summon_{name}"),
            Some(Pos::new(4, 3)),
        );
        for _ in 0..(2 * delay - 1) {
            pass(&mut g);
        }
        let piece = g.at(Pos::new(4, 3)).unwrap();
        assert_eq!(piece.archetype, name);
        assert_eq!(piece.stats.tough, tough);
        assert!(piece.state.acted);
        assert!(g.scheduled().is_empty());
    }
}
