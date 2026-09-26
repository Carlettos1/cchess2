use cchess_engine::geometry::*;
use std::collections::BTreeSet;

#[test]
fn union_patterns_deduplicate_and_validate_nested_members() {
    let origin = Pos::new(3, 3);
    assert_eq!(
        targets(
            Pattern::Any(vec![Pattern::FullStep, Pattern::FullStep]),
            origin,
            &[]
        ),
        targets(Pattern::FullStep, origin, &[])
    );
    assert!(Pattern::Any(vec![]).normalize().is_err());
    assert!(Pattern::Any(vec![Pattern::Leap(vec![])])
        .normalize()
        .is_err());
}

fn targets(pattern: Pattern, origin: Pos, blocked: &[Pos]) -> Vec<Pos> {
    pattern.normalize().unwrap().targets(
        Bounds {
            width: 8,
            height: 8,
        },
        origin,
        Direction::N,
        |p| blocked.contains(&p),
    )
}

#[test]
fn all_relative_directions_match_the_design_table() {
    use Relative::*;
    use SubDirection as S;
    let relative = [F, B, L, R, FR, BR, BL, FL];
    let table = [
        (
            Direction::N,
            [S::N, S::S, S::W, S::E, S::NE, S::SE, S::SW, S::NW],
        ),
        (
            Direction::E,
            [S::E, S::W, S::N, S::S, S::SE, S::SW, S::NW, S::NE],
        ),
        (
            Direction::S,
            [S::S, S::N, S::E, S::W, S::SW, S::NW, S::NE, S::SE],
        ),
        (
            Direction::W,
            [S::W, S::E, S::S, S::N, S::NW, S::NE, S::SE, S::SW],
        ),
    ];
    for (forward, expected) in table {
        for (relative, absolute) in relative.iter().zip(expected) {
            assert_eq!(relative.absolute(forward), absolute.vector());
        }
    }
}
#[test]
fn position_arithmetic_checks_overflow_and_bounds() {
    assert_eq!(Pos::new(i32::MAX, 0).offset(1, 0), None);
    assert_eq!(Pos::new(0, i32::MIN).offset(0, -1), None);
    assert_eq!(Pos::new(0, 0).offset(-1, 0), Some(Pos::new(-1, 0)));
    assert!(!Bounds {
        width: 8,
        height: 8
    }
    .contains(Pos::new(-1, 0)));
}
#[test]
fn rays_include_blocker_but_never_jump_it_even_before_minimum() {
    let origin = Pos::new(2, 2);
    let blocker = Pos::new(2, 4);
    assert_eq!(
        targets(
            Pattern::InfiniteRay(vec![SubDirection::N]),
            origin,
            &[blocker]
        ),
        vec![Pos::new(2, 3), blocker]
    );
    assert!(targets(
        Pattern::Ray {
            min: 3,
            max: Some(6),
            directions: vec![SubDirection::N]
        },
        origin,
        &[blocker]
    )
    .is_empty());
}
#[test]
fn leaps_ignore_intermediate_pieces_and_diagonal_distances_are_exact() {
    assert!(targets(
        Pattern::SymmetricalLeap(vec![(1, 2)]),
        Pos::new(0, 0),
        &[Pos::new(0, 1), Pos::new(1, 1)]
    )
    .contains(&Pos::new(1, 2)));
    assert_eq!(
        targets(
            Pattern::DiagonalLeap(vec![2]),
            Pos::new(0, 0),
            &[Pos::new(1, 1)]
        ),
        vec![Pos::new(2, 2)]
    );
}
#[test]
fn aliases_normalize_and_bad_ranges_are_rejected() {
    assert_eq!(
        Pattern::Step(vec![SubDirection::E]).normalize(),
        Pattern::Ray {
            min: 1,
            max: Some(1),
            directions: vec![SubDirection::E]
        }
        .normalize()
    );
    assert_eq!(
        Pattern::RelativeStep(vec![Relative::F]).normalize(),
        Pattern::RelativeRay {
            min: 1,
            max: Some(1),
            directions: vec![Relative::F]
        }
        .normalize()
    );
    assert!(Pattern::Ray {
        min: 2,
        max: Some(1),
        directions: vec![SubDirection::E]
    }
    .normalize()
    .is_err());
    assert!(Pattern::InfiniteRay(vec![]).normalize().is_err());
    assert!(Pattern::Leap(vec![]).normalize().is_err());
}
#[test]
fn self_range_and_end_of_board_contracts() {
    let origin = Pos::new(3, 3);
    assert_eq!(targets(Pattern::SelfOnly, origin, &[]), vec![origin]);
    assert_eq!(targets(Pattern::TaxicabRange(2), origin, &[]).len(), 13);
    assert_eq!(targets(Pattern::AreaSquare(2), origin, &[]).len(), 25);
    assert!(!targets(Pattern::FullStep, origin, &[]).contains(&origin));
    assert_eq!(
        targets(Pattern::EndOfBoard(Relative::F), origin, &[Pos::new(3, 4)]),
        vec![Pos::new(3, 7)]
    );
}
#[test]
fn symmetric_leaps_are_closed_under_rotations_and_reflections() {
    for dx in 0..=4 {
        for dy in 0..=4 {
            let pattern = Pattern::SymmetricalLeap(vec![(dx, dy), (dx, dy)])
                .normalize()
                .unwrap();
            let origin = Pos::new(8, 8);
            let result = pattern.targets(
                Bounds {
                    width: 17,
                    height: 17,
                },
                origin,
                Direction::N,
                |_| false,
            );
            for p in &result {
                let (x, y) = (p.x - origin.x, p.y - origin.y);
                for (x, y) in [(x, y), (-y, x), (-x, -y), (y, -x), (-x, y)] {
                    assert!(result.contains(&origin.offset(x, y).unwrap()));
                }
            }
            assert_eq!(result.len(), result.iter().collect::<BTreeSet<_>>().len());
        }
    }
}
#[test]
fn all_patterns_produce_sorted_unique_bounded_targets_on_edges_and_blockers() {
    let patterns = vec![
        Pattern::SelfOnly,
        Pattern::Ray {
            min: 0,
            max: Some(3),
            directions: vec![SubDirection::N, SubDirection::N],
        },
        Pattern::InfiniteRay(SubDirection::ALL.to_vec()),
        Pattern::Step(SubDirection::ALL.to_vec()),
        Pattern::FullStep,
        Pattern::Leap(vec![(-2, 1), (1, 2), (0, 0)]),
        Pattern::SymmetricalLeap(vec![(1, 2)]),
        Pattern::DiagonalLeap(vec![1, 3]),
        Pattern::RelativeRay {
            min: 1,
            max: Some(2),
            directions: vec![Relative::F],
        },
        Pattern::RelativeInfiniteRay(vec![Relative::F, Relative::FL]),
        Pattern::RelativeStep(vec![Relative::F]),
        Pattern::EndOfBoard(Relative::R),
        Pattern::TaxicabRange(2),
        Pattern::AreaSquare(2),
    ];
    for pattern in patterns {
        // Golden schema round trip for every deterministic input primitive.
        let text = ron::to_string(&pattern).unwrap();
        assert_eq!(ron::from_str::<Pattern>(&text).unwrap(), pattern);
        let pattern = pattern.normalize().unwrap();
        for bounds in [
            Bounds {
                width: 1,
                height: 1,
            },
            Bounds {
                width: 3,
                height: 5,
            },
            Bounds {
                width: 8,
                height: 8,
            },
        ] {
            for origin in bounds.positions() {
                for forward in [Direction::N, Direction::E, Direction::S, Direction::W] {
                    let result = pattern.targets(bounds, origin, forward, |p| (p.x + p.y) % 3 == 0);
                    assert!(result.iter().all(|&p| bounds.contains(p)));
                    assert!(result.windows(2).all(|p| p[0] < p[1]));
                }
            }
        }
    }
}

#[test]
fn bounded_range_enumeration_matches_full_board_reference_with_masks() {
    let bounds = Bounds {
        width: 64,
        height: 64,
    };
    let playable = |p: Pos| !(p.x < 4 && p.y < 4);
    for origin in [
        Pos::new(0, 1),
        Pos::new(4, 4),
        Pos::new(32, 31),
        Pos::new(63, 62),
    ] {
        for distance in [0, 1, 4, 64, u16::MAX] {
            for taxicab in [false, true] {
                let actual = RuntimePattern::Range { distance, taxicab }.targets_on_board(
                    bounds,
                    origin,
                    Direction::N,
                    |_| false,
                    playable,
                );
                let expected: Vec<_> = bounds
                    .positions()
                    .filter(|&p| {
                        let dx = (p.x - origin.x).abs();
                        let dy = (p.y - origin.y).abs();
                        playable(origin)
                            && playable(p)
                            && (if taxicab { dx + dy } else { dx.max(dy) }) <= i32::from(distance)
                    })
                    .collect();
                assert_eq!(actual, expected);
            }
        }
    }
}
