mod support;
use cchess_engine::{cards::CardChoice, content::Tag, state::Snapshot, *};
use support::*;

#[test]
fn throw_freezes_square_includes_caster_enemies_and_diagonal_boundary() {
    let mut g = game(
        &[
            ("rook", 0, 3, 3),
            ("rook", 0, 5, 5),
            ("rook", 1, 1, 1),
            ("rook", 0, 0, 2),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let caster = id(&g, 3, 3);
    let boundary = id(&g, 5, 5);
    let enemy = id(&g, 1, 1);
    let outside = id(&g, 0, 2);
    let intent = ActionIntent::Ability {
        player: 0,
        source: caster,
        ability: "throw".into(),
        target: Some(Pos::new(4, 3)),
    };
    let mut replay = g.clone();
    g.apply(intent.clone()).unwrap();
    replay.apply(intent).unwrap();
    assert_eq!(g.piece(caster).unwrap().position, Pos::new(7, 3));
    assert_eq!(g.piece(boundary).unwrap().position, Pos::new(7, 5));
    assert_eq!(g.piece(enemy).unwrap().position, Pos::new(7, 1));
    assert_eq!(g.piece(outside).unwrap().position, Pos::new(0, 2));
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
}

#[test]
fn throw_front_to_back_captures_and_follows_vacated_squares() {
    let mut g = game(
        &[("rook", 0, 2, 3), ("rook", 0, 4, 3), ("pawn", 1, 6, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let back = id(&g, 2, 3);
    let front = id(&g, 4, 3);
    cast(&mut g, back, "throw", Some(Pos::new(3, 3)));
    assert_eq!(g.piece(front).unwrap().position, Pos::new(6, 3));
    assert_eq!(g.piece(back).unwrap().position, Pos::new(5, 3));
    assert_eq!(g.dead().len(), 1);
}

#[test]
fn surviving_blocker_takes_damage_and_rook_moves_before_it() {
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "wall")
            .unwrap()
            .base_stats
            .tough = 3
    });
    let mut g = setup(
        reg,
        &[("rook", 0, 1, 1), ("wall", 1, 5, 1)],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let rook = id(&g, 1, 1);
    let wall = id(&g, 5, 1);
    cast(&mut g, rook, "throw", Some(Pos::new(2, 1)));
    assert_eq!(g.piece(rook).unwrap().position, Pos::new(4, 1));
    assert_eq!(g.piece(wall).unwrap().state.damage, 1);
}

#[test]
fn smite_requires_consumes_card_and_rejects_non_demonic() {
    let mut g = game(
        &[("paladin", 0, 0, 0), ("imp", 1, 7, 7), ("pawn", 1, 6, 7)],
        &["attack_demonic"],
        &["mana"],
        1,
    );
    let source = id(&g, 0, 0);
    let mut intent = ActionIntent::Ability {
        player: 0,
        source,
        ability: "smite".into(),
        target: Some(Pos::new(7, 7)),
    };
    assert!(g.validate(&intent).is_err());
    play(&mut g, "attack_demonic", CardChoice::None);
    if let ActionIntent::Ability { target, .. } = &mut intent {
        *target = Some(Pos::new(6, 7));
    }
    let before = g.snapshot();
    assert!(g.apply(intent.clone()).is_err());
    assert_eq!(before, g.snapshot());
    if let ActionIntent::Ability { target, .. } = &mut intent {
        *target = Some(Pos::new(7, 7));
    }
    g.apply(intent).unwrap();
    assert!(g.at(Pos::new(7, 7)).is_none());
    assert!(!g.player(0).unwrap().board_cards.contains("attack_demonic"));
}

#[test]
fn paladin_revives_normal_tags_and_claims_latest_friendly_death() {
    let mut g = game(
        &[
            ("paladin", 0, 3, 3),
            ("warlock", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["revive"],
        &["mana"],
        1,
    );
    play(&mut g, "revive", CardChoice::None);
    let source = id(&g, 3, 3);
    let intent = ActionIntent::Ability {
        player: 0,
        source,
        ability: "revive".into(),
        target: Some(Pos::new(7, 0)),
    };
    let before = g.snapshot();
    assert!(g.apply(intent.clone()).is_err());
    assert_eq!(g.snapshot(), before);
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    g.apply(intent).unwrap();
    let revived = id(&g, 7, 0);
    assert!(g.has_tag(revived, Tag::Immune));
    assert!(!g.has_tag(revived, Tag::Undead));
    assert_eq!(g.dead()[0].revived_as, Some(revived));
    assert!(!g.player(0).unwrap().board_cards.contains("revive"));
}

#[test]
fn throw_respects_invulnerable_blocker_and_all_cardinal_edges() {
    let mut g = game(
        &[("paladin", 0, 0, 0), ("pawn", 0, 5, 3), ("rook", 1, 1, 3)],
        &["invulnerability"],
        &["mana"],
        1,
    );
    play(&mut g, "invulnerability", CardChoice::None);
    let paladin = id(&g, 0, 0);
    cast(&mut g, paladin, "ward", Some(Pos::new(5, 3)));
    let rook = id(&g, 1, 3);
    cast(&mut g, rook, "throw", Some(Pos::new(2, 3)));
    assert_eq!(g.piece(rook).unwrap().position, Pos::new(4, 3));
    assert_eq!(g.at(Pos::new(5, 3)).unwrap().state.damage, 0);
    for (toward, expected) in [
        ((3, 4), (3, 7)),
        ((4, 3), (7, 3)),
        ((3, 2), (3, 0)),
        ((2, 3), (0, 3)),
    ] {
        let mut g = game(&[("rook", 0, 3, 3)], &["mana"], &["mana"], 0);
        let rook = id(&g, 3, 3);
        cast(&mut g, rook, "throw", Some(Pos::new(toward.0, toward.1)));
        assert_eq!(
            g.piece(rook).unwrap().position,
            Pos::new(expected.0, expected.1)
        );
    }
}
