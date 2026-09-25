use cchess_engine::GameAi;
use cchess_engine::{ai::AiPolicy, content::Tag, state::Snapshot, *};
use std::collections::BTreeSet;

#[test]
fn full_two_player_setup_preserves_original_armies_and_terrain() {
    let g = Game::full_board(Registry::built_in().unwrap(), false).unwrap();
    assert_eq!(
        g.bounds(),
        Bounds {
            width: 16,
            height: 17
        }
    );
    assert_eq!(g.pieces().count(), 92);
    assert_eq!(
        g.pieces()
            .map(|p| p.archetype.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        20
    );
    for owner in 0..2 {
        assert_eq!(g.pieces().filter(|p| p.owner == owner).count(), 46);
        assert_eq!(
            g.pieces()
                .filter(|p| p.owner == owner && g.has_tag(p.id, Tag::Royal))
                .count(),
            1
        );
        assert_eq!(g.player(owner).unwrap().mana, 1);
        assert_eq!(g.player(owner).unwrap().cards.hand.len(), 3);
    }
    for piece in g.pieces().filter(|p| p.owner == 0) {
        let opposite = g
            .at(Pos::new(piece.position.x, 16 - piece.position.y))
            .unwrap();
        assert_eq!(opposite.owner, 1);
        assert_eq!(opposite.archetype, piece.archetype);
    }
    assert_eq!(g.at(Pos::new(8, 0)).unwrap().archetype, "king");
    assert_eq!(g.at(Pos::new(4, 0)).unwrap().archetype, "warlock");
    assert_eq!(g.at(Pos::new(3, 1)).unwrap().archetype, "builder");
    assert_eq!(g.at(Pos::new(3, 2)).unwrap().archetype, "shield_bearer");
    assert_eq!(
        g.bounds()
            .positions()
            .filter(|p| g.is_magic_tile(*p))
            .collect::<BTreeSet<_>>(),
        [
            Pos::new(0, 7),
            Pos::new(0, 9),
            Pos::new(15, 7),
            Pos::new(15, 9)
        ]
        .into_iter()
        .collect()
    );
    assert!(g.bounds().positions().all(|p| g.is_buildable_tile(p)));
}
#[test]
fn full_ffa_rotates_four_complete_armies_on_a_cross_without_overlap() {
    let g = Game::full_board(Registry::built_in().unwrap(), true).unwrap();
    assert_eq!(
        g.bounds(),
        Bounds {
            width: 24,
            height: 24
        }
    );
    assert_eq!(g.pieces().count(), 184);
    assert_eq!(
        g.pieces()
            .map(|p| p.position)
            .collect::<BTreeSet<_>>()
            .len(),
        184
    );
    assert_eq!(
        g.bounds().positions().filter(|p| g.is_playable(*p)).count(),
        512
    );
    assert_eq!(
        g.bounds()
            .positions()
            .filter(|p| g.is_magic_tile(*p))
            .count(),
        8
    );
    assert!(g.pieces().all(|p| g.is_playable(p.position)));
    for piece in g.pieces().filter(|p| p.owner == 0) {
        let other = g
            .at(Pos::new(piece.position.y, 23 - piece.position.x))
            .unwrap();
        assert_eq!(other.owner, 1);
        assert_eq!(other.archetype, piece.archetype);
    }
    for pos in [
        Pos::new(0, 0),
        Pos::new(23, 0),
        Pos::new(0, 23),
        Pos::new(23, 23),
    ] {
        assert!(!g.is_playable(pos));
        assert!(!g.is_buildable_tile(pos));
    }
}
#[test]
fn both_ai_policies_run_on_full_boards_with_deterministic_replay_and_terrain() {
    for four in [false, true] {
        let game = Game::full_board(Registry::built_in().unwrap(), four).unwrap();
        let seats = game
            .players()
            .iter()
            .map(|p| {
                (
                    p.id,
                    if p.id % 2 == 0 {
                        AiPolicy::Random
                    } else {
                        AiPolicy::First
                    },
                )
            })
            .collect();
        let mut game = game.with_ai(seats, 17).unwrap();
        let mut replay = game.clone();
        for _ in 0..24 {
            let Some(action) = game.choose_ai_action() else {
                break;
            };
            assert_eq!(
                game.play_ai_step().unwrap().unwrap(),
                replay.apply(action).unwrap()
            );
            assert!(game.pieces().all(|p| game.is_playable(p.position)));
        }
        assert_eq!(game.snapshot(), replay.snapshot());
        assert_eq!(
            ron::from_str::<Snapshot>(&ron::to_string(&game.snapshot()).unwrap()).unwrap(),
            game.snapshot()
        );
    }
}
