mod support;
use cchess_engine::{
    cards::{CardChoice, DeckDef},
    content::Tag,
    state::{Placement, Player, Snapshot},
    *,
};
use support::*;

#[test]
fn dragon_catalogue_tags_stats_and_full_movement_are_exact() {
    let g = game(&[("dragon", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    assert_eq!(g.piece(source).unwrap().stats.tough, 3);
    assert!(g.has_tag(source, Tag::Immune));
    assert!(g.has_tag(source, Tag::Heroic));
    assert!(!g.has_tag(source, Tag::Demonic));
    assert!(!g.has_tag(source, Tag::Biological));
    assert_eq!(g.piece(source).unwrap().stats.transportable, None);
    let actions = g.legal_actions(source);
    assert_eq!(actions.len(), 16);
    assert!(actions.iter().all(|a| matches!(
        a,
        ActionIntent::Piece {
            kind: ActionKind::Move,
            ..
        }
    )));
    assert!(g.registry().get("dragon").unwrap().abilities.is_empty());
}

#[test]
fn dragon_captures_by_knight_leap_and_adjacent_step_without_ray_blocking() {
    for target in [Pos::new(4, 4), Pos::new(4, 5)] {
        let mut g = game(
            &[
                ("dragon", 0, 3, 3),
                ("pawn", 1, target.x, target.y),
                ("wall", 0, 3, 4),
            ],
            &["mana"],
            &["mana"],
            0,
        );
        let source = id(&g, 3, 3);
        act(&mut g, source, ActionKind::Take, target);
        assert_eq!(g.piece(source).unwrap().position, target);
    }
}

#[test]
fn heroic_and_immune_reject_attack_and_shove_without_cost() {
    let mut g = game(
        &[("cannon", 0, 0, 0), ("ogre", 0, 2, 2), ("dragon", 1, 3, 2)],
        &["mana"],
        &["mana"],
        0,
    );
    let before = g.snapshot();
    for intent in [
        ActionIntent::Piece {
            player: 0,
            source: id(&g, 0, 0),
            kind: ActionKind::Attack,
            target: Pos::new(3, 2),
        },
        ActionIntent::Ability {
            player: 0,
            source: id(&g, 2, 2),
            ability: "shove".into(),
            target: Some(Pos::new(3, 2)),
        },
    ] {
        assert_eq!(g.apply(intent), Err(RuleError::ProtectedTarget));
        assert_eq!(g.snapshot(), before);
    }
}

#[test]
fn three_take_hits_remove_dragon_without_demonic_reward() {
    let mut g = game(
        &[("rook", 0, 0, 0), ("dragon", 1, 0, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 0, 0);
    for hit in 1..=3 {
        act(&mut g, source, ActionKind::Take, Pos::new(0, 3));
        if hit < 3 {
            assert_eq!(g.at(Pos::new(0, 3)).unwrap().state.damage, hit);
            assert_eq!(g.piece(source).unwrap().position, Pos::new(0, 0));
            pass(&mut g);
        }
    }
    assert_eq!(g.piece(source).unwrap().position, Pos::new(0, 3));
    assert_eq!(g.player(1).unwrap().mana, 20);
}

#[test]
fn summon_card_is_seeded_in_starter_deck_and_spawns_acted_with_full_health() {
    let starter: DeckDef = ron::from_str(include_str!("../assets/decks/starter.ron")).unwrap();
    assert_eq!(starter.cards.len(), 21);
    assert_eq!(
        starter
            .cards
            .iter()
            .filter(|id| id.as_str() == "summon_dragon")
            .count(),
        1
    );
    let mut g = game(&[("king", 0, 3, 3)], &["summon_dragon"], &["mana"], 1);
    let intent = ActionIntent::PlayCard {
        player: 0,
        card: hand(&g, "summon_dragon"),
        target: CardChoice::Square(Pos::new(4, 3)),
    };
    let before = g.snapshot();
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(intent.clone()).unwrap(), preview);
    assert_eq!(replay.apply(intent).unwrap(), preview);
    let dragon = g.at(Pos::new(4, 3)).unwrap();
    assert_eq!(dragon.archetype, "dragon");
    assert_eq!(dragon.stats.tough, 3);
    assert!(dragon.state.acted);
    assert_eq!(g.player(0).unwrap().mana, 14);
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn dragon_revives_as_undead_with_suppressed_protection() {
    let mut g = game(
        &[
            ("necromancer", 0, 3, 3),
            ("dragon", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let rook = id(&g, 0, 7);
    for _ in 0..3 {
        pass(&mut g);
        act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    }
    let source = id(&g, 3, 3);
    cast(&mut g, source, "revive", Some(Pos::new(3, 4)));
    let dragon = g.at(Pos::new(3, 4)).unwrap();
    assert_eq!(dragon.stats.tough, 3);
    assert_eq!(dragon.state.damage, 0);
    assert!(g.has_tag(dragon.id, Tag::Undead));
    assert!(!g.has_tag(dragon.id, Tag::Heroic));
    assert!(!g.has_tag(dragon.id, Tag::Immune));
    assert!(!g.has_tag(dragon.id, Tag::Demonic));
}

#[test]
fn dragon_knight_capture_works_against_each_opponent_in_four_player_mode() {
    for enemy in 1..4 {
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
                Placement::new("dragon", 0, Pos::new(6, 6)),
                Placement::new("pawn", enemy, Pos::new(7, 8)),
            ],
        )
        .unwrap();
        let source = id(&g, 6, 6);
        act(&mut g, source, ActionKind::Take, Pos::new(7, 8));
        assert_eq!(g.piece(source).unwrap().position, Pos::new(7, 8));
    }
}
