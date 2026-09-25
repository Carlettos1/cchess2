use cchess_engine::{
    content::{
        EffectDef, PieceArchetype, Stat, Tag, TraverseEffect, CLASSICAL_SOURCES, CUSTOM_SOURCES,
    },
    state::{Placement, Player, Snapshot, StatusKind},
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

#[test]
fn new_catalogue_stats_and_movement_match_the_plan() {
    let reg = Registry::built_in().unwrap();
    assert_eq!(reg.iter().count(), 36);
    for (name, strength, impenetrable, transportable) in [
        ("shield_bearer", 0, 0, Some(1)),
        ("builder", 0, 0, Some(3)),
        ("wall", 0, 3, None),
        ("ballista", 2, 0, None),
        ("ram", 4, 0, None),
    ] {
        let p = &reg.get(name).unwrap().definition;
        assert_eq!(
            (
                p.base_stats.strength,
                p.base_stats.impenetrable,
                p.base_stats.transportable
            ),
            (strength, impenetrable, transportable)
        );
    }
    for name in ["ballista", "ram", "builder", "wall"] {
        let g = setup(&[(name, 0, 3, 3), ("pawn", 1, 4, 4)]);
        let legal = g.legal_actions(id(&g, 3, 3));
        let moves: Vec<_> = legal
            .iter()
            .filter_map(|a| match a {
                ActionIntent::Piece {
                    kind: ActionKind::Move,
                    target,
                    ..
                } => Some(*target),
                _ => None,
            })
            .collect();
        if name == "wall" {
            assert!(legal.is_empty());
        } else if name == "builder" {
            assert_eq!(moves.len(), 4);
            assert!(moves.contains(&Pos::new(5, 5))); // leaps over occupied e5
            assert!(legal.iter().any(|a| matches!(a,ActionIntent::Piece{kind:ActionKind::Take,target,..} if *target==Pos::new(4,4))));
        } else {
            assert_eq!(moves.len(), 4);
            assert!(moves.contains(&Pos::new(3, 4)));
            assert!(!legal.iter().any(|a| matches!(
                a,
                ActionIntent::Piece {
                    kind: ActionKind::Take | ActionKind::Attack,
                    ..
                }
            )));
        }
    }
}

#[test]
fn shield_bearer_forward_rules_rotate_and_never_jump_blockers() {
    for forward in [Direction::N, Direction::E, Direction::S, Direction::W] {
        let origin = Pos::new(3, 3);
        let (dx, dy) = forward.rotate((0, 1));
        let blocker = origin.offset(dx, dy).unwrap();
        let (cx, cy) = forward.rotate((1, 1));
        let capture = origin.offset(cx, cy).unwrap();
        let g = setup_with(
            Registry::built_in().unwrap(),
            &[
                ("shield_bearer", 0, 3, 3),
                ("pawn", 0, blocker.x, blocker.y),
                ("pawn", 1, capture.x, capture.y),
            ],
            vec![player(0, forward), player(1, Direction::S)],
        );
        let legal = g.legal_actions(id(&g, 3, 3));
        assert!(!legal.iter().any(|a| matches!(
            a,
            ActionIntent::Piece {
                kind: ActionKind::Move,
                ..
            }
        )));
        assert!(legal.iter().any(|a| matches!(a, ActionIntent::Piece{kind:ActionKind::Take,target,..} if *target==capture)));
        let clear = setup_with(
            Registry::built_in().unwrap(),
            &[("shield_bearer", 0, 3, 3)],
            vec![player(0, forward), player(1, Direction::S)],
        );
        let moves = clear.legal_actions(id(&clear, 3, 3));
        assert_eq!(
            moves
                .iter()
                .filter(|a| matches!(
                    a,
                    ActionIntent::Piece {
                        kind: ActionKind::Move,
                        ..
                    }
                ))
                .count(),
            2
        );
    }
}

#[test]
fn fortify_increments_zero_and_existing_defense_without_affecting_enemies_or_immune() {
    let mut g = setup(&[
        ("shield_bearer", 0, 3, 3),
        ("pawn", 0, 2, 3),
        ("wall", 0, 3, 4),
        ("super_pawn", 0, 2, 2),
        ("pawn", 1, 4, 3),
        ("pawn", 0, 5, 3),
    ]);
    let shield = id(&g, 3, 3);
    let action = cast(&mut g, shield, "fortify", None);
    for (x, y, n) in [
        (2, 3, 1),
        (3, 4, 4),
        (2, 2, 0),
        (4, 3, 0),
        (5, 3, 0),
        (3, 3, 0),
    ] {
        assert_eq!(g.at(Pos::new(x, y)).unwrap().stats.impenetrable, n);
    }
    assert_eq!(
        action
            .events
            .iter()
            .filter(|e| matches!(e, Event::StatModified { .. }))
            .count(),
        2
    );
    // Buff persists across turns, and a later cast stacks by addition.
    for _ in 0..7 {
        pass(&mut g);
    }
    cast(&mut g, shield, "fortify", None);
    assert_eq!(g.at(Pos::new(3, 4)).unwrap().stats.impenetrable, 5);
    assert_eq!(
        g.registry()
            .get("wall")
            .unwrap()
            .definition
            .base_stats
            .impenetrable,
        3
    );
}

#[test]
fn fortify_empty_targets_and_protection_have_no_cost() {
    let mut g = setup(&[("shield_bearer", 0, 3, 3), ("super_pawn", 0, 3, 4)]);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source: id(&g, 3, 3),
            ability: "fortify".into(),
            target: None
        }),
        Err(RuleError::InvalidTarget)
    );
    assert_eq!(g, before);
    let mut g = setup(&[
        ("shield_bearer", 0, 3, 3),
        ("wall", 0, 3, 4),
        ("paladin", 0, 4, 4),
    ]);
    let paladin = id(&g, 4, 4);
    let shield = id(&g, 3, 3);
    cast(&mut g, paladin, "ward", Some(Pos::new(3, 4)));
    pass(&mut g);
    assert!(g.legal_abilities(shield).is_empty());
    assert_eq!(g.at(Pos::new(3, 4)).unwrap().stats.impenetrable, 3);
}

#[test]
fn builders_choose_all_four_facings_independently_of_owner_forward() {
    for owner_forward in [Direction::N, Direction::E, Direction::S, Direction::W] {
        for facing in [Direction::N, Direction::E, Direction::S, Direction::W] {
            let mut g = setup_with(
                Registry::built_in().unwrap(),
                &[("builder", 0, 3, 3)],
                vec![player(0, owner_forward), player(1, Direction::S)],
            );
            let builder = id(&g, 3, 3);
            let (dx, dy) = facing.rotate((0, 1));
            let action = cast(&mut g, builder, "build", Some(Pos::new(3 + dx, 3 + dy)));
            let mut ids = std::collections::BTreeSet::new();
            for offset in [(-1, 1), (0, 1), (1, 1)] {
                let (dx, dy) = facing.rotate(offset);
                let wall = g.at(Pos::new(3 + dx, 3 + dy)).unwrap();
                assert_eq!(wall.archetype, "wall");
                assert_eq!(wall.owner, 0);
                assert_eq!(wall.stats.impenetrable, 3);
                assert!(wall.state.acted);
                assert!(g.legal_actions(wall.id).is_empty());
                ids.insert(wall.id);
            }
            assert_eq!(ids.len(), 3);
            assert_eq!(
                action
                    .events
                    .iter()
                    .filter(|e| matches!(e, Event::PieceSpawned { .. }))
                    .count(),
                3
            );
            assert_eq!(g.player(0).unwrap().mana, 19);
        }
    }
}

#[test]
fn building_is_atomic_for_any_occupied_or_offboard_footprint_square() {
    for occupied_x in [2, 3, 4] {
        let mut g = setup(&[("builder", 0, 3, 3), ("pawn", 1, occupied_x, 4)]);
        let before = g.clone();
        let intent = ActionIntent::Ability {
            player: 0,
            source: id(&g, 3, 3),
            ability: "build".into(),
            target: Some(Pos::new(3, 4)),
        };
        assert!(!g.legal_abilities(id(&g, 3, 3)).contains(&intent));
        assert_eq!(g.apply(intent), Err(RuleError::InvalidTarget));
        assert_eq!(g, before);
    }
    let mut g = setup(&[("builder", 0, 0, 3)]);
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source: id(&g, 0, 3),
            ability: "build".into(),
            target: Some(Pos::new(0, 4))
        }),
        Err(RuleError::InvalidTarget)
    );
    assert_eq!(g, before);
}

fn defense_registry(defense: u32, tough: u32, immune: bool, ram_tough: u32) -> Registry {
    registry(|defs| {
        let wall = defs.iter_mut().find(|p| p.id == "wall").unwrap();
        wall.base_stats.impenetrable = defense;
        wall.base_stats.tough = tough;
        if immune {
            wall.tags.insert(Tag::Immune);
        }
        defs.iter_mut()
            .find(|p| p.id == "ram")
            .unwrap()
            .base_stats
            .tough = ram_tough;
    })
}
fn shot_setup(registry: Registry, source: &str) -> Game {
    setup_with(
        registry,
        &[
            (source, 0, 0, 3),
            ("pawn", 0, 1, 3),
            ("wall", 1, 3, 3),
            ("pawn", 1, 5, 3),
        ],
        vec![player(0, Direction::N), player(1, Direction::S)],
    )
}

#[test]
fn ballista_passes_lower_defense_and_stops_before_equal_or_higher() {
    for defense in [1, 2, 3] {
        let mut g = shot_setup(defense_registry(defense, 1, false, 1), "ballista");
        let ballista = id(&g, 0, 3);
        let wall = id(&g, 3, 3);
        let action = cast(&mut g, ballista, "pierce", Some(Pos::new(1, 3)));
        assert!(g.at(Pos::new(1, 3)).is_none()); // friendly fire
        assert_eq!(g.piece(ballista).unwrap().position, Pos::new(0, 3));
        assert_eq!(g.piece(ballista).unwrap().state.damage, 0);
        if defense < 2 {
            assert!(g.piece(wall).is_none());
            assert!(g.at(Pos::new(5, 3)).is_none());
        } else {
            assert_eq!(g.piece(wall).unwrap().state.damage, 0);
            assert!(g.at(Pos::new(5, 3)).is_some());
        }
        let positions: Vec<_> = action
            .events
            .iter()
            .filter_map(|e| match e {
                Event::TraversalVisited { position, .. } => Some(*position),
                _ => None,
            })
            .collect();
        assert_eq!(
            positions,
            (1..=if defense < 2 { 7 } else { 3 })
                .map(|x| Pos::new(x, 3))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn ram_passes_lower_defense_and_equal_higher_damage_both_on_stop() {
    for defense in [3, 4, 5] {
        let mut g = shot_setup(defense_registry(defense, 2, false, 2), "ram");
        let ram = id(&g, 0, 3);
        let wall = id(&g, 3, 3);
        let action = cast(&mut g, ram, "charge", Some(Pos::new(1, 3)));
        assert_eq!(g.piece(wall).unwrap().state.damage, 1);
        let survivor = g.piece(ram).unwrap();
        assert_eq!(
            survivor.position,
            Pos::new(if defense < 4 { 7 } else { 2 }, 3)
        );
        assert_eq!(survivor.state.damage, if defense < 4 { 0 } else { 1 });
        assert_eq!(g.at(Pos::new(5, 3)).is_some(), defense >= 4);
        let move_index = action
            .events
            .iter()
            .position(|e| matches!(e,Event::PieceMoved{piece,..} if *piece==ram))
            .unwrap();
        if defense >= 4 {
            let hit = action
                .events
                .iter()
                .position(|e| matches!(e,Event::DamageDealt{target,..}if *target==wall))
                .unwrap();
            assert!(move_index < hit);
        }
    }
}

#[test]
fn ram_collision_removes_default_toughness_source_and_blocker_without_entering_blocker_square() {
    let mut g = shot_setup(defense_registry(4, 1, false, 1), "ram");
    let ram = id(&g, 0, 3);
    let wall = id(&g, 3, 3);
    cast(&mut g, ram, "charge", Some(Pos::new(1, 3)));
    assert!(g.piece(ram).is_none());
    assert!(g.piece(wall).is_none());
    assert_eq!(
        g.dead()
            .iter()
            .find(|d| d.piece.id == ram)
            .unwrap()
            .piece
            .position,
        Pos::new(2, 3)
    );
    assert!(g.at(Pos::new(5, 3)).is_some());
}

#[test]
fn protection_prevents_damage_but_only_defense_stops_traversal() {
    for source in ["ballista", "ram"] {
        let strength = if source == "ram" { 4 } else { 2 };
        for defense in [0, strength] {
            let mut g = shot_setup(defense_registry(defense, 1, true, 2), source);
            let caster = id(&g, 0, 3);
            let wall = id(&g, 3, 3);
            cast(
                &mut g,
                caster,
                if source == "ram" { "charge" } else { "pierce" },
                Some(Pos::new(1, 3)),
            );
            assert_eq!(g.piece(wall).unwrap().state.damage, 0);
            assert_eq!(g.at(Pos::new(5, 3)).is_some(), defense == strength);
            if source == "ram" {
                assert_eq!(
                    g.piece(caster).unwrap().state.damage,
                    if defense == strength { 1 } else { 0 }
                );
            }
        }
    }
}

#[test]
fn directional_selection_can_point_at_an_immune_piece_and_is_not_an_attack() {
    let mut g = setup(&[
        ("ballista", 0, 0, 3),
        ("super_pawn", 1, 1, 3),
        ("pawn", 1, 2, 3),
    ]);
    let ballista = id(&g, 0, 3);
    cast(&mut g, ballista, "pierce", Some(Pos::new(1, 3)));
    assert!(g.at(Pos::new(1, 3)).is_some());
    assert!(g.at(Pos::new(2, 3)).is_none());
    let reg = registry(|defs| {
        defs.iter_mut()
            .find(|p| p.id == "pawn")
            .unwrap()
            .tags
            .insert(Tag::Heroic);
    });
    let mut g = shot_setup(reg, "ballista");
    let ballista = id(&g, 0, 3);
    cast(&mut g, ballista, "pierce", Some(Pos::new(1, 3)));
    assert!(g.at(Pos::new(1, 3)).is_none());
}

#[test]
fn all_cardinal_rays_and_empty_edges_have_deterministic_landings() {
    for (origin, toward, end) in [
        ((3, 3), (3, 4), (3, 7)),
        ((3, 3), (4, 3), (7, 3)),
        ((3, 3), (3, 2), (3, 0)),
        ((3, 3), (2, 3), (0, 3)),
    ] {
        let mut g = setup(&[("ram", 0, origin.0, origin.1)]);
        let ram = id(&g, origin.0, origin.1);
        let action = cast(&mut g, ram, "charge", Some(Pos::new(toward.0, toward.1)));
        assert_eq!(g.piece(ram).unwrap().position, Pos::new(end.0, end.1));
        assert!(!action
            .events
            .iter()
            .any(|e| matches!(e, Event::DamageDealt { .. })));
        assert!(g.pieces().all(|p| g.bounds().contains(p.position)));
    }
    let mut g = setup(&[("ram", 0, 0, 0)]);
    let before = g.clone();
    for target in [
        Pos::new(-1, 0),
        Pos::new(1, 1),
        Pos::new(0, 2),
        Pos::new(0, 0),
    ] {
        assert!(g
            .apply(ActionIntent::Ability {
                player: 0,
                source: id(&g, 0, 0),
                ability: "charge".into(),
                target: Some(target)
            })
            .is_err());
        assert_eq!(g, before);
    }
}

#[test]
fn ram_never_overwrites_protected_or_tough_survivors_at_the_edge() {
    for name in ["super_pawn", "wall"] {
        let mut g = setup_with(
            defense_registry(0, 2, false, 2),
            &[("ram", 0, 0, 3), (name, 1, 7, 3)],
            vec![player(0, Direction::N), player(1, Direction::S)],
        );
        let ram = id(&g, 0, 3);
        let victim = id(&g, 7, 3);
        cast(&mut g, ram, "charge", Some(Pos::new(1, 3)));
        assert_eq!(g.piece(ram).unwrap().position, Pos::new(6, 3));
        assert_eq!(g.piece(victim).unwrap().position, Pos::new(7, 3));
    }
}

#[test]
fn shield_buff_changes_a_walls_ram_blocker_threshold() {
    let mut g = setup(&[
        ("shield_bearer", 0, 3, 3),
        ("wall", 0, 3, 4),
        ("ram", 1, 0, 4),
    ]);
    let shield = id(&g, 3, 3);
    let ram = id(&g, 0, 4);
    let wall = id(&g, 3, 4);
    cast(&mut g, shield, "fortify", None);
    assert_eq!(g.piece(wall).unwrap().stats.impenetrable, 4);
    cast(&mut g, ram, "charge", Some(Pos::new(1, 4)));
    assert!(g.piece(ram).is_none());
    assert!(g.piece(wall).is_none());
}

#[test]
fn traversal_death_hooks_and_royal_victory_share_normal_resolution() {
    let reg = registry(|defs| {
        let king = defs.iter_mut().find(|p| p.id == "king").unwrap();
        king.tags.remove(&Tag::Immune);
    });
    let mut g = setup_with(
        reg,
        &[
            ("ballista", 0, 0, 3),
            ("mandragora", 1, 1, 3),
            ("king", 1, 3, 3),
        ],
        vec![player(0, Direction::N), player(1, Direction::S)],
    );
    let caster = id(&g, 0, 3);
    let action = cast(&mut g, caster, "pierce", Some(Pos::new(1, 3)));
    assert_eq!(g.winner(), Some(0));
    assert_eq!(g.player(1).unwrap().mana, 21);
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e, Event::ManaGranted { amount: 1, .. })));
    assert!(!action
        .events
        .iter()
        .any(|e| matches!(e, Event::TurnStarted { .. })));
}

#[test]
fn preview_replay_and_serialization_preserve_stats_spawns_and_traversal() {
    let initial = Game::siege_demo(Registry::built_in().unwrap()).unwrap();
    let mut g = initial.clone();
    let builder = id(&g, 1, 1);
    let intent = ActionIntent::Ability {
        player: 0,
        source: builder,
        ability: "build".into(),
        target: Some(Pos::new(1, 2)),
    };
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g, initial);
    assert_eq!(g.apply(intent).unwrap(), preview);
    pass(&mut g);
    let shield = id(&g, 2, 0);
    cast(&mut g, shield, "fortify", None);
    pass(&mut g);
    let ram = id(&g, 6, 1);
    cast(&mut g, ram, "charge", Some(Pos::new(6, 2)));
    let serialized = ron::to_string(&g.snapshot()).unwrap();
    assert_eq!(
        ron::from_str::<Snapshot>(&serialized).unwrap(),
        g.snapshot()
    );
    let mut replay = initial;
    for action in g.log() {
        assert_eq!(replay.apply(action.intent.clone()).unwrap(), *action);
    }
    assert_eq!(g, replay);
}

#[test]
fn stat_changes_saturate_at_zero_and_u32_max() {
    for (base, amount, expected) in [(0, -1, 0), (u32::MAX, 1, u32::MAX), (2, -1, 1)] {
        let reg = registry(|defs| {
            defs.iter_mut()
                .find(|p| p.id == "wall")
                .unwrap()
                .base_stats
                .impenetrable = base;
            defs.iter_mut()
                .find(|p| p.id == "shield_bearer")
                .unwrap()
                .abilities[0]
                .effects = vec![EffectDef::ModifyStat {
                stat: Stat::Impenetrable,
                amount,
            }];
        });
        let mut g = setup_with(
            reg,
            &[("shield_bearer", 0, 3, 3), ("wall", 0, 3, 4)],
            vec![player(0, Direction::N), player(1, Direction::S)],
        );
        let shield = id(&g, 3, 3);
        cast(&mut g, shield, "fortify", None);
        assert_eq!(g.at(Pos::new(3, 4)).unwrap().stats.impenetrable, expected);
    }
}

#[test]
fn traversal_work_is_counted_and_limit_failure_rolls_back_partial_damage() {
    let reg = registry(|defs| {
        let ballista = defs.iter_mut().find(|p| p.id == "ballista").unwrap();
        if let EffectDef::Traverse(t) = &mut ballista.abilities[0].effects[0] {
            t.on_pass = vec![TraverseEffect::DamageTarget(1); 1024];
        }
    });
    let mut g = shot_setup(reg, "ballista");
    let before = g.clone();
    assert_eq!(
        g.apply(ActionIntent::Ability {
            player: 0,
            source: id(&g, 0, 3),
            ability: "pierce".into(),
            target: Some(Pos::new(1, 3))
        }),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g, before);
}

#[test]
fn invulnerability_and_disable_statuses_apply_to_siege_abilities() {
    let mut g = setup(&[
        ("ballista", 0, 0, 3),
        ("pawn", 1, 1, 3),
        ("paladin", 1, 1, 4),
        ("pawn", 1, 3, 3),
    ]);
    let paladin = id(&g, 1, 4);
    let ballista = id(&g, 0, 3);
    pass(&mut g);
    cast(&mut g, paladin, "ward", Some(Pos::new(1, 3)));
    cast(&mut g, ballista, "pierce", Some(Pos::new(1, 3)));
    assert!(g.at(Pos::new(1, 3)).is_some());
    assert!(g.at(Pos::new(3, 3)).is_none());
    for source in ["ballista", "ram", "builder", "shield_bearer"] {
        // Seed the existing card-gated Ice cast through ordinary gameplay.
        let reg = registry(|defs| {
            defs.iter_mut()
                .find(|p| p.id == "magician")
                .unwrap()
                .abilities
                .iter_mut()
                .find(|a| a.id == "ice")
                .unwrap()
                .conditions
                .clear();
        });
        let mut g = setup_with(
            reg,
            &[(source, 0, 3, 3), ("magician", 1, 3, 4), ("pawn", 0, 2, 3)],
            vec![player(0, Direction::N), player(1, Direction::S)],
        );
        let magician = id(&g, 3, 4);
        let caster = id(&g, 3, 3);
        pass(&mut g);
        cast(&mut g, magician, "ice", None);
        assert!(g.has_status(caster, StatusKind::Ice));
        assert!(g.legal_actions(caster).is_empty());
    }
}
