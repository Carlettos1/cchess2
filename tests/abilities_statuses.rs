use cchess_engine::{
    content::{
        AbilityDef, ActionCost, ActionRule, Condition, EffectDef, PieceArchetype, Selection, Tag,
        CLASSICAL_SOURCES, CUSTOM_SOURCES,
    },
    geometry::Pattern,
    state::{Placement, Player, Snapshot, StatusKind},
    targeting::{TargetFilter, TargetOrigin, TargetSpec},
    *,
};

fn spell(id: &str, effect: EffectDef) -> AbilityDef {
    AbilityDef {
        id: id.into(),
        display_name: id.into(),
        target: TargetSpec {
            origin: TargetOrigin::Source,
            pattern: Pattern::AreaSquare(7),
            filters: vec![TargetFilter::Occupied],
            count: 1,
        },
        selection: Selection::One,
        conditions: vec![],
        cost: ActionCost::default(),
        cooldown: 0,
        once: false,
        consume_cards: vec![],
        effects: vec![effect],
    }
}
fn registry(edit: impl FnOnce(&mut Vec<PieceArchetype>)) -> Registry {
    let mut defs: Vec<PieceArchetype> = CLASSICAL_SOURCES
        .iter()
        .chain(CUSTOM_SOURCES.iter())
        .map(|(_, s)| ron::from_str(s).unwrap())
        .collect();
    let rook = defs.iter_mut().find(|d| d.id == "rook").unwrap();
    for (id, kind, duration) in [
        ("fire", StatusKind::Fire, 1),
        ("fire_long", StatusKind::Fire, 3),
        ("ice", StatusKind::Ice, 2),
        ("deactivate", StatusKind::Deactivate, 2),
        ("ward", StatusKind::Invulnerable, 2),
    ] {
        rook.abilities
            .push(spell(id, EffectDef::AddStatus { kind, duration }));
    }
    rook.abilities
        .push(spell("cleanse", EffectDef::RemoveStatuses));
    rook.abilities
        .push(spell("damage", EffectDef::DamageTarget(1)));
    rook.action_rules.push(ActionRule {
        kind: ActionKind::Attack,
        target: TargetSpec {
            origin: TargetOrigin::Source,
            pattern: Pattern::AreaSquare(7),
            filters: vec![TargetFilter::Enemy, TargetFilter::Occupied],
            count: 1,
        },
        conditions: vec![],
        cost: ActionCost::default(),
        effects: vec![EffectDef::DamageTarget(1)],
    });
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
fn players() -> Vec<Player> {
    [Player::new(0, Direction::N), Player::new(1, Direction::S)]
        .into_iter()
        .map(|mut p| {
            p.mana = 30.into();
            p.mana_per_turn = 0;
            p.board_cards = ["fire", "ice", "invulnerability"]
                .into_iter()
                .map(String::from)
                .collect();
            p
        })
        .collect()
}
fn setup_with(registry: Registry, pieces: &[(&str, u8, i32, i32)], players: Vec<Player>) -> Game {
    Game::new(
        registry,
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
    setup_with(registry(|_| {}), pieces, players())
}
fn id(game: &Game, x: i32, y: i32) -> u64 {
    game.at(Pos::new(x, y)).unwrap().id
}
fn ability(game: &Game, source: u64, name: &str, target: Option<Pos>) -> ActionIntent {
    ActionIntent::Ability {
        player: game.turn().player,
        source,
        ability: name.into(),
        target,
    }
}
fn cast(game: &mut Game, source: u64, name: &str, target: Option<Pos>) -> ResolvedAction {
    game.apply(ability(game, source, name, target)).unwrap()
}
fn pass(game: &mut Game) {
    game.apply(ActionIntent::EndTurn {
        player: game.turn().player,
    })
    .unwrap();
}
fn move_to(game: &mut Game, source: u64, kind: ActionKind, target: Pos) -> ResolvedAction {
    game.apply(ActionIntent::Piece {
        player: game.turn().player,
        source,
        kind,
        target,
    })
    .unwrap()
}
fn reject(game: &mut Game, intent: ActionIntent, error: RuleError) {
    let before = game.clone();
    assert_eq!(game.apply(intent), Err(error));
    assert_eq!(*game, before);
}
fn choose(game: &mut Game, archetype: &str) {
    let pending = game.pending_promotion().unwrap();
    game.apply(ActionIntent::ChoosePromotion {
        player: pending.player,
        token: pending.token,
        archetype: archetype.into(),
    })
    .unwrap();
}

#[test]
fn promotion_is_a_serializable_choice_preserving_identity_and_turn_cost() {
    let mut game = setup(&[("pawn", 0, 2, 6)]);
    let pawn = id(&game, 2, 6);
    move_to(&mut game, pawn, ActionKind::Move, Pos::new(2, 7));
    assert_eq!(game.turn().player, 0);
    assert_eq!(game.turn().remaining_actions, 0);
    let token = game.pending_promotion().unwrap().token;
    assert_eq!(game.legal_actions(pawn).len(), 4);
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&game.snapshot()).unwrap()).unwrap(),
        game.snapshot()
    );
    reject(
        &mut game,
        ActionIntent::EndTurn { player: 0 },
        RuleError::PromotionPending,
    );
    reject(
        &mut game,
        ActionIntent::ChoosePromotion {
            player: 0,
            token: token + 1,
            archetype: "queen".into(),
        },
        RuleError::InvalidPromotion,
    );
    reject(
        &mut game,
        ActionIntent::ChoosePromotion {
            player: 0,
            token,
            archetype: "king".into(),
        },
        RuleError::InvalidPromotion,
    );
    let mana = game.player(0).unwrap().mana.clone();
    choose(&mut game, "knight");
    assert_eq!(game.piece(pawn).unwrap().archetype, "knight");
    assert_eq!(game.piece(pawn).unwrap().position, Pos::new(2, 7));
    assert!(game.piece(pawn).unwrap().state.acted);
    assert!(game.dead().is_empty());
    assert_eq!(game.turn().player, 1);
    assert_eq!(game.player(0).unwrap().mana, mana);
    assert!(game.pending_promotion().is_none());
    assert!(!game
        .log()
        .last()
        .unwrap()
        .events
        .iter()
        .any(|e| matches!(e, Event::CostsPaid { .. })));
}
#[test]
fn promotion_uses_each_owners_forward_orientation() {
    for (forward, from, to) in [
        (Direction::N, (2, 6), (2, 7)),
        (Direction::E, (6, 2), (7, 2)),
        (Direction::S, (2, 1), (2, 0)),
        (Direction::W, (1, 2), (0, 2)),
    ] {
        let mut players = players();
        players[0].forward = forward;
        let mut game = setup_with(registry(|_| {}), &[("pawn", 0, from.0, from.1)], players);
        let pawn = id(&game, from.0, from.1);
        move_to(&mut game, pawn, ActionKind::Move, Pos::new(to.0, to.1));
        choose(&mut game, "rook");
        assert_eq!(game.piece(pawn).unwrap().archetype, "rook");
    }
}
#[test]
fn surviving_take_does_not_promote_and_winning_take_needs_no_choice() {
    let reg = registry(|d| {
        d.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .base_stats
            .tough = 2
    });
    let mut game = setup_with(reg, &[("pawn", 0, 2, 6), ("rook", 1, 3, 7)], players());
    let pawn = id(&game, 2, 6);
    move_to(&mut game, pawn, ActionKind::Take, Pos::new(3, 7));
    assert!(game.pending_promotion().is_none());
    pass(&mut game);
    move_to(&mut game, pawn, ActionKind::Take, Pos::new(3, 7));
    assert!(game.pending_promotion().is_some());
    choose(&mut game, "queen");
    let mut game = setup(&[("pawn", 0, 2, 6), ("king", 1, 3, 7)]);
    let pawn = id(&game, 2, 6);
    move_to(&mut game, pawn, ActionKind::Take, Pos::new(3, 7));
    assert_eq!(game.winner(), Some(0));
    assert!(game.pending_promotion().is_none());
}
#[test]
fn movement_abilities_pay_costs_track_cooldowns_and_enforce_one_use() {
    let reg = registry(|d| {
        d.iter_mut()
            .find(|p| p.id == "king")
            .unwrap()
            .tags
            .remove(&Tag::Immune);
    });
    let mut game = setup_with(reg, &[("bishop", 0, 2, 2), ("king", 0, 7, 0)], players());
    let bishop = id(&game, 2, 2);
    let king = id(&game, 7, 0);
    cast(&mut game, bishop, "sidestep", Some(Pos::new(2, 3)));
    assert_eq!(
        game.piece(bishop).unwrap().state.cooldowns["sidestep"].remaining,
        2
    );
    pass(&mut game);
    assert_eq!(
        game.piece(bishop).unwrap().state.cooldowns["sidestep"].remaining,
        1
    );
    pass(&mut game);
    pass(&mut game);
    assert!(!game
        .piece(bishop)
        .unwrap()
        .state
        .cooldowns
        .contains_key("sidestep"));
    cast(&mut game, king, "teleport", Some(Pos::new(4, 4)));
    assert_eq!(game.piece(king).unwrap().position, Pos::new(4, 4));
    pass(&mut game);
    let intent = ability(&game, king, "teleport", Some(Pos::new(7, 0)));
    reject(
        &mut game,
        intent,
        RuleError::AbilityAlreadyUsed("teleport".into()),
    );
}

#[test]
fn failed_abilities_do_not_pay_or_mark_used() {
    let mut p = players();
    p[0].mana = 1.into();
    let mut game = setup_with(registry(|_| {}), &[("king", 0, 0, 0)], p);
    let king = id(&game, 0, 0);
    let intent = ability(&game, king, "teleport", Some(Pos::new(2, 2)));
    reject(
        &mut game,
        intent,
        RuleError::InsufficientMana {
            required: 2,
            available: 1,
        },
    );
    let mut game = setup(&[("king", 0, 0, 0)]);
    let king = id(&game, 0, 0);
    let intent = ability(&game, king, "teleport", Some(Pos::new(7, 7)));
    reject(&mut game, intent, RuleError::ProtectedTarget);
    let intent = ability(&game, king, "teleport", None);
    reject(&mut game, intent, RuleError::ProtectedTarget);
}
#[test]
fn knight_spawns_unique_pawns_in_empty_relative_squares_without_immediate_actions() {
    let mut p = players();
    p[0].forward = Direction::E;
    p[0].actions_per_turn = 2;
    let mut game = setup_with(
        registry(|_| {}),
        &[("knight", 0, 3, 3), ("rook", 1, 3, 4)],
        p,
    );
    let knight = id(&game, 3, 3);
    cast(&mut game, knight, "muster", Some(Pos::new(3, 3)));
    let pawn = game.at(Pos::new(3, 2)).unwrap();
    assert_eq!(pawn.archetype, "pawn");
    assert!(pawn.state.acted);
    assert!(game.legal_actions(pawn.id).is_empty());
    assert_eq!(game.at(Pos::new(3, 4)).unwrap().owner, 1);
    assert_eq!(game.player(0).unwrap().mana, 29);
    assert_eq!(
        game.pieces()
            .map(|p| p.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
}
#[test]
fn spawning_on_final_rank_queues_each_promotion_before_turn_transfer() {
    let mut game = setup(&[("knight", 0, 3, 7)]);
    let knight = id(&game, 3, 7);
    cast(&mut game, knight, "muster", Some(Pos::new(3, 7)));
    let first = game.pending_promotion().unwrap().piece;
    let token = game.pending_promotion().unwrap().token;
    choose(&mut game, "queen");
    assert_eq!(game.turn().player, 0);
    assert_ne!(game.pending_promotion().unwrap().piece, first);
    assert_ne!(game.pending_promotion().unwrap().token, token);
    choose(&mut game, "rook");
    assert_eq!(game.turn().player, 1);
    assert!(game.pending_promotion().is_none());
}
#[test]
fn ice_and_deactivate_allow_only_attack_and_move_respectively() {
    for (name, status, allowed) in [
        ("ice", StatusKind::Ice, ActionKind::Attack),
        ("deactivate", StatusKind::Deactivate, ActionKind::Move),
    ] {
        let mut game = setup(&[("rook", 0, 0, 0), ("rook", 1, 3, 3), ("pawn", 0, 3, 5)]);
        let caster = id(&game, 0, 0);
        let victim = id(&game, 3, 3);
        cast(&mut game, caster, name, Some(Pos::new(3, 3)));
        assert_eq!(game.piece(victim).unwrap().statuses[&status].remaining, 1);
        for kind in [ActionKind::Move, ActionKind::Take, ActionKind::Attack] {
            let target = if kind == ActionKind::Move {
                Pos::new(3, 4)
            } else {
                Pos::new(3, 5)
            };
            let intent = ActionIntent::Piece {
                player: 1,
                source: victim,
                kind,
                target,
            };
            assert_eq!(game.validate(&intent).is_ok(), kind == allowed);
        }
        let intent = ability(&game, victim, "damage", Some(Pos::new(0, 0)));
        reject(&mut game, intent, RuleError::Deactivated(status));
        pass(&mut game);
        pass(&mut game);
        assert!(!game.has_status(victim, status));
    }
}

#[test]
fn fire_expiration_damages_once_and_uses_normal_toughness_removal() {
    for tough in [1, 2] {
        let reg = registry(|d| {
            d.iter_mut()
                .find(|d| d.id == "pawn")
                .unwrap()
                .base_stats
                .tough = tough
        });
        let mut game = setup_with(reg, &[("rook", 0, 0, 0), ("pawn", 1, 3, 5)], players());
        let caster = id(&game, 0, 0);
        let victim = id(&game, 3, 5);
        let applied = cast(&mut game, caster, "fire", Some(Pos::new(3, 5)));
        assert!(!game.has_status(victim, StatusKind::Fire));
        if tough == 1 {
            assert!(game.piece(victim).is_none());
            assert_eq!(game.dead()[0].piece.position, Pos::new(3, 5));
            assert_eq!(game.dead()[0].cause.action_id, applied.id);
        } else {
            assert_eq!(game.piece(victim).unwrap().state.damage, 1);
            assert!(!game.has_status(victim, StatusKind::Fire));
        }
        for _ in 0..4 {
            pass(&mut game);
        }
        assert_eq!(
            game.log()
                .iter()
                .flat_map(|a| &a.events)
                .filter(|e| matches!(e,Event::DamageDealt{target,..} if *target==victim))
                .count(),
            1
        );
    }
}
#[test]
fn cleanse_removes_all_statuses_and_cancels_fire_without_damage() {
    let mut p = players();
    p[0].actions_per_turn = 2;
    let mut game = setup_with(
        registry(|_| {}),
        &[
            ("rook", 0, 0, 0),
            ("rook", 0, 1, 0),
            ("pawn", 1, 3, 5),
            ("super_pawn", 1, 3, 6),
        ],
        p,
    );
    let first = id(&game, 0, 0);
    let second = id(&game, 1, 0);
    let victim = id(&game, 3, 5);
    let cleanser = id(&game, 3, 6);
    cast(&mut game, first, "fire_long", Some(Pos::new(3, 5)));
    cast(&mut game, second, "ice", Some(Pos::new(3, 5)));
    let clean = cast(&mut game, cleanser, "cleanse", Some(Pos::new(3, 5)));
    assert!(game.piece(victim).unwrap().statuses.is_empty());
    assert_eq!(game.piece(victim).unwrap().state.damage, 0);
    assert_eq!(
        clean
            .events
            .iter()
            .filter(|e| matches!(e, Event::StatusRemoved { expired: false, .. }))
            .count(),
        2
    );
    for _ in 0..6 {
        pass(&mut game);
    }
    assert!(game.piece(victim).is_some());
    assert_eq!(game.piece(victim).unwrap().state.damage, 0);
}
#[test]
fn negative_stacking_keeps_lower_duration_and_positive_keeps_higher() {
    let mut p = players();
    p[0].actions_per_turn = 5;
    let mut game = setup_with(
        registry(|_| {}),
        &[
            ("rook", 0, 0, 0),
            ("rook", 0, 1, 0),
            ("rook", 0, 2, 0),
            ("pawn", 1, 3, 5),
        ],
        p,
    );
    let a = id(&game, 0, 0);
    let b = id(&game, 1, 0);
    let c = id(&game, 2, 0);
    let victim = id(&game, 3, 5);
    cast(&mut game, a, "fire_long", Some(Pos::new(3, 5)));
    let shorter = cast(&mut game, b, "fire", Some(Pos::new(3, 5)));
    cast(&mut game, c, "fire_long", Some(Pos::new(3, 5)));
    let status = &game.piece(victim).unwrap().statuses[&StatusKind::Fire];
    assert_eq!(status.remaining, 1);
    assert_eq!(status.source.action_id, shorter.id);
    assert_eq!(game.piece(victim).unwrap().statuses.len(), 1);
}

#[test]
fn friendly_status_does_not_lose_a_turn_immediately_on_application() {
    let mut game = setup(&[("rook", 0, 0, 0), ("pawn", 0, 3, 3)]);
    let caster = id(&game, 0, 0);
    let target = id(&game, 3, 3);
    cast(&mut game, caster, "fire", Some(Pos::new(3, 3)));
    assert!(game.has_status(target, StatusKind::Fire));
    assert!(game.piece(target).is_some());
    pass(&mut game);
    assert!(game.piece(target).is_none());
}
#[test]
fn invulnerable_blocks_incoming_take_attack_and_beneficial_or_hostile_abilities() {
    let mut game = setup(&[("rook", 0, 0, 0), ("rook", 1, 0, 7), ("pawn", 0, 0, 3)]);
    let caster = id(&game, 0, 0);
    let enemy = id(&game, 0, 7);
    cast(&mut game, caster, "ward", Some(Pos::new(0, 3)));
    for kind in [ActionKind::Take, ActionKind::Attack] {
        let intent = ActionIntent::Piece {
            player: 1,
            source: enemy,
            kind,
            target: Pos::new(0, 3),
        };
        reject(&mut game, intent, RuleError::ProtectedTarget);
    }
    for spell in ["fire", "cleanse"] {
        let intent = ability(&game, enemy, spell, Some(Pos::new(0, 3)));
        reject(&mut game, intent, RuleError::ProtectedTarget);
    }
    pass(&mut game);
    let intent = ability(&game, caster, "cleanse", Some(Pos::new(0, 3)));
    reject(&mut game, intent, RuleError::ProtectedTarget);
}
#[test]
fn immunity_blocks_positive_negative_area_and_self_effects() {
    let mut game = setup(&[("rook", 0, 0, 0), ("king", 1, 3, 3)]);
    let caster = id(&game, 0, 0);
    for spell in ["fire", "ward", "damage"] {
        let intent = ability(&game, caster, spell, Some(Pos::new(3, 3)));
        reject(&mut game, intent, RuleError::ProtectedTarget);
    }
    let mut game = setup(&[("paladin", 0, 2, 2)]);
    let paladin = id(&game, 2, 2);
    let intent = ability(&game, paladin, "ward", Some(Pos::new(2, 2)));
    reject(&mut game, intent, RuleError::ProtectedTarget);
    assert!(!game.has_status(paladin, StatusKind::Invulnerable));
    move_to(&mut game, paladin, ActionKind::Move, Pos::new(3, 3));
    let mut game = setup(&[("rook", 0, 0, 0), ("queen", 1, 3, 3)]);
    let caster = id(&game, 0, 0);
    cast(&mut game, caster, "damage", Some(Pos::new(3, 3)));
    assert!(game.at(Pos::new(3, 3)).is_none());
}
#[test]
fn magician_area_effects_hit_only_eligible_enemies_and_card_unlocks_are_live() {
    let mut game = setup(&[
        ("magician", 0, 2, 2),
        ("pawn", 0, 2, 3),
        ("pawn", 1, 3, 3),
        ("pawn", 1, 4, 3),
        ("rook", 1, 7, 7),
        ("king", 1, 5, 5),
    ]);
    let magician = id(&game, 2, 2);
    let protected = id(&game, 3, 3);
    let victim = id(&game, 4, 3);
    let enemy = id(&game, 7, 7);
    pass(&mut game);
    cast(&mut game, enemy, "ward", Some(Pos::new(3, 3)));
    let intent = ability(&game, magician, "fire", None);
    assert_eq!(game.ability_targets(&intent).unwrap(), vec![Pos::new(4, 3)]);
    game.apply(intent).unwrap();
    assert!(game.has_status(victim, StatusKind::Fire));
    assert!(!game.has_status(protected, StatusKind::Fire));
    assert!(!game.has_status(id(&game, 2, 3), StatusKind::Fire));
    assert!(!game.has_status(id(&game, 5, 5), StatusKind::Fire));
    let reg = registry(|d| {
        let rook = d.iter_mut().find(|d| d.id == "rook").unwrap();
        let mut consume = spell(
            "consume_fire",
            EffectDef::AddStatus {
                kind: StatusKind::Invulnerable,
                duration: 1,
            },
        );
        consume.consume_cards = vec!["fire".into()];
        rook.abilities.push(consume);
    });
    let mut game = setup_with(
        reg,
        &[("magician", 0, 2, 2), ("rook", 0, 0, 0), ("pawn", 1, 4, 3)],
        players(),
    );
    let caster = id(&game, 0, 0);
    let magician = id(&game, 2, 2);
    cast(&mut game, caster, "consume_fire", Some(Pos::new(0, 0)));
    pass(&mut game);
    let intent = ability(&game, magician, "fire", None);
    reject(
        &mut game,
        intent,
        RuleError::ConditionFailed(Condition::CardOnBoard("fire".into())),
    );
}
#[test]
fn empty_area_casts_fail_atomically_and_paladin_consumes_its_card_once() {
    let mut game = setup(&[("magician", 0, 2, 2), ("king", 1, 3, 3)]);
    let magician = id(&game, 2, 2);
    let intent = ability(&game, magician, "fire", None);
    reject(&mut game, intent, RuleError::InvalidTarget);
    let mut game = setup(&[("paladin", 0, 2, 2), ("pawn", 0, 3, 3)]);
    let paladin = id(&game, 2, 2);
    let pawn = id(&game, 3, 3);
    cast(&mut game, paladin, "ward", Some(Pos::new(3, 3)));
    assert!(!game
        .player(0)
        .unwrap()
        .board_cards
        .contains("invulnerability"));
    assert!(game.has_status(pawn, StatusKind::Invulnerable));
    pass(&mut game);
    let intent = ability(&game, paladin, "ward", Some(Pos::new(2, 2)));
    reject(
        &mut game,
        intent,
        RuleError::ConditionFailed(Condition::CardOnBoard("invulnerability".into())),
    );
}
#[test]
fn fire_retains_attribution_after_caster_death() {
    let reg = registry(|d| {
        let rook = d.iter_mut().find(|d| d.id == "rook").unwrap();
        rook.abilities
            .iter_mut()
            .find(|a| a.id == "fire")
            .unwrap()
            .effects = vec![EffectDef::AddStatus {
            kind: StatusKind::Fire,
            duration: 2,
        }];
    });
    let mut game = setup_with(
        reg,
        &[("rook", 0, 0, 0), ("rook", 1, 0, 7), ("pawn", 1, 1, 6)],
        players(),
    );
    let caster = id(&game, 0, 0);
    let enemy = id(&game, 0, 7);
    let victim = id(&game, 1, 6);
    let action = cast(&mut game, caster, "fire", Some(Pos::new(1, 6)));
    move_to(&mut game, enemy, ActionKind::Take, Pos::new(0, 0));
    assert!(game.piece(caster).is_none());
    pass(&mut game);
    pass(&mut game);
    let death = game.dead().iter().find(|d| d.piece.id == victim).unwrap();
    assert_eq!(
        death.cause.source,
        cchess_engine::state::ActionSource::Piece(caster)
    );
    assert_eq!(death.cause.player, 0);
    assert_eq!(death.cause.action_id, action.id);
    assert_eq!(death.cause.status, Some(StatusKind::Fire));
}
#[test]
fn fire_royal_death_ends_the_game_before_starting_another_turn() {
    let reg = registry(|d| {
        d.iter_mut()
            .find(|d| d.id == "king")
            .unwrap()
            .tags
            .remove(&Tag::Immune);
    });
    let mut game = setup_with(reg, &[("rook", 0, 0, 0), ("king", 1, 3, 3)], players());
    let caster = id(&game, 0, 0);
    let action = cast(&mut game, caster, "fire", Some(Pos::new(3, 3)));
    assert_eq!(game.winner(), Some(0));
    assert_eq!(action.events.last(), Some(&Event::GameWon { player: 0 }));
    assert!(action
        .events
        .iter()
        .any(|e| matches!(e, Event::TurnStarted { .. })));
}
#[test]
fn preexisting_fire_survives_promotion_and_new_invulnerability() {
    let mut game = setup(&[("pawn", 0, 2, 6), ("rook", 1, 7, 7)]);
    let pawn = id(&game, 2, 6);
    let caster = id(&game, 7, 7);
    pass(&mut game);
    cast(&mut game, caster, "fire_long", Some(Pos::new(2, 6)));
    move_to(&mut game, pawn, ActionKind::Move, Pos::new(2, 7));
    assert!(game.has_status(pawn, StatusKind::Fire));
    choose(&mut game, "queen");
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(game.piece(pawn).is_none());
    assert_eq!(game.dead()[0].piece.archetype, "queen");
    let mut game = setup(&[("rook", 0, 0, 0), ("paladin", 1, 2, 2), ("pawn", 1, 3, 3)]);
    let caster = id(&game, 0, 0);
    let paladin = id(&game, 2, 2);
    let pawn = id(&game, 3, 3);
    cast(&mut game, caster, "fire_long", Some(Pos::new(3, 3)));
    cast(&mut game, paladin, "ward", Some(Pos::new(3, 3)));
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(game.piece(pawn).is_none());
}
#[test]
fn builtin_custom_scenario_replays_statuses_choices_and_abilities_deterministically() {
    let initial = Game::abilities_demo(Registry::built_in().unwrap()).unwrap();
    let mut game = initial.clone();
    let mut seed = 9182_u64;
    for _ in 0..120 {
        if game.winner().is_some() {
            break;
        }
        let actions: Vec<_> = game
            .pieces()
            .flat_map(|p| game.legal_actions(p.id))
            .collect();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let action = if actions.is_empty() {
            ActionIntent::EndTurn {
                player: game.turn().player,
            }
        } else {
            actions[seed as usize % actions.len()].clone()
        };
        game.apply(action).unwrap();
        assert_eq!(
            game.pieces()
                .map(|p| p.position)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            game.pieces().count()
        );
        assert!(game
            .pieces()
            .all(|p| p.statuses.values().all(|s| s.remaining > 0)
                && p.state.cooldowns.values().all(|c| c.remaining > 0)));
    }
    let mut replay = initial;
    for entry in game.log() {
        assert_eq!(&replay.apply(entry.intent.clone()).unwrap(), entry);
    }
    assert_eq!(game, replay);
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&game.snapshot()).unwrap()).unwrap(),
        game.snapshot()
    );
}

#[test]
fn invulnerability_expires_and_separate_disabling_statuses_do_not_cancel_each_other() {
    let mut game = setup(&[("rook", 0, 0, 0), ("rook", 1, 0, 7), ("pawn", 0, 0, 3)]);
    let caster = id(&game, 0, 0);
    let enemy = id(&game, 0, 7);
    let pawn = id(&game, 0, 3);
    cast(&mut game, caster, "ward", Some(Pos::new(0, 3)));
    for _ in 0..4 {
        pass(&mut game);
    }
    assert!(!game.has_status(pawn, StatusKind::Invulnerable));
    move_to(&mut game, enemy, ActionKind::Take, Pos::new(0, 3));
    assert!(game.piece(pawn).is_none());
    let reg = registry(|d| {
        d.iter_mut()
            .find(|d| d.id == "rook")
            .unwrap()
            .abilities
            .iter_mut()
            .find(|a| a.id == "ice")
            .unwrap()
            .effects = vec![EffectDef::AddStatus {
            kind: StatusKind::Ice,
            duration: 1,
        }]
    });
    let mut p = players();
    p[0].actions_per_turn = 2;
    let mut game = setup_with(
        reg,
        &[("rook", 0, 0, 0), ("rook", 0, 1, 0), ("bishop", 1, 3, 3)],
        p,
    );
    let first = id(&game, 0, 0);
    let second = id(&game, 1, 0);
    let victim = id(&game, 3, 3);
    cast(&mut game, first, "deactivate", Some(Pos::new(3, 3)));
    cast(&mut game, second, "ice", Some(Pos::new(3, 3)));
    assert!(!game.has_status(victim, StatusKind::Ice));
    assert!(game.has_status(victim, StatusKind::Deactivate));
    assert!(!game.legal_actions(victim).is_empty());
    pass(&mut game);
    pass(&mut game);
    assert!(!game.legal_actions(victim).is_empty());
}

#[test]
fn succubus_uses_biological_filter_and_taxicab_range() {
    let mut game = setup(&[
        ("succubus", 0, 2, 2),
        ("pawn", 1, 3, 3),
        ("rook", 1, 2, 3),
        ("pawn", 1, 6, 2),
    ]);
    let caster = id(&game, 2, 2);
    let victim = id(&game, 3, 3);
    for target in [Pos::new(2, 3), Pos::new(6, 2)] {
        let intent = ability(&game, caster, "deactivate", Some(target));
        reject(&mut game, intent, RuleError::InvalidTarget);
    }
    cast(&mut game, caster, "deactivate", Some(Pos::new(3, 3)));
    assert_eq!(
        game.piece(victim).unwrap().statuses[&StatusKind::Deactivate].remaining,
        2
    );
}

#[test]
fn positive_status_stacking_keeps_maximum_duration() {
    let reg = registry(|defs| {
        let rook = defs.iter_mut().find(|p| p.id == "rook").unwrap();
        rook.abilities.push(spell(
            "stack",
            EffectDef::Sequence(
                vec![2, 5, 3]
                    .into_iter()
                    .map(|duration| EffectDef::AddStatus {
                        kind: StatusKind::Invulnerable,
                        duration,
                    })
                    .collect(),
            ),
        ));
    });
    let mut game = setup_with(reg, &[("rook", 0, 0, 0)], players());
    let rook = id(&game, 0, 0);
    cast(&mut game, rook, "stack", Some(Pos::new(0, 0)));
    assert_eq!(
        game.piece(rook).unwrap().statuses[&StatusKind::Invulnerable].remaining,
        5
    );
}

#[test]
fn gaining_immunity_cleanses_pending_fire_without_damage() {
    let reg = registry(|defs| {
        defs.iter_mut()
            .find(|p| p.id == "pawn")
            .unwrap()
            .promotion
            .as_mut()
            .unwrap()
            .choices = vec!["warlock".into()]
    });
    let mut game = setup_with(reg, &[("pawn", 0, 2, 6), ("rook", 1, 7, 7)], players());
    let pawn = id(&game, 2, 6);
    let rook = id(&game, 7, 7);
    pass(&mut game);
    cast(&mut game, rook, "fire_long", Some(Pos::new(2, 6)));
    move_to(&mut game, pawn, ActionKind::Move, Pos::new(2, 7));
    assert!(game.has_status(pawn, StatusKind::Fire));
    choose(&mut game, "warlock");
    assert!(game.has_tag(pawn, Tag::Immune));
    assert!(game.piece(pawn).unwrap().statuses.is_empty());
    for _ in 0..8 {
        pass(&mut game);
    }
    assert_eq!(game.piece(pawn).unwrap().state.damage, 0);
}
