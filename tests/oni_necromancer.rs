mod support;
use cchess_engine::{
    cards::CardMatcher,
    content::{EffectDef, PieceArchetype, Tag, TriggerDef, TriggerEvent},
    geometry::SubDirection,
    state::Snapshot,
    *,
};
use support::*;

fn revival_ready(name: &str) -> Game {
    let mut g = game(
        &[("necromancer", 0, 3, 3), (name, 0, 0, 2), ("rook", 1, 0, 7)],
        &["mana"],
        &["mana"],
        0,
    );
    let rook = id(&g, 0, 7);
    let tough = g.at(Pos::new(0, 2)).unwrap().stats.tough;
    for _ in 0..tough {
        pass(&mut g);
        act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    }
    g
}
fn revive(g: &Game, pos: Pos) -> ActionIntent {
    ActionIntent::Ability {
        player: g.turn().player,
        source: id(g, 3, 3),
        ability: "revive".into(),
        target: Some(pos),
    }
}

#[test]
fn oni_movement_combines_orthogonal_five_ray_with_knight_leap() {
    let g = game(
        &[("oni", 0, 1, 1), ("wall", 0, 2, 1)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 1, 1);
    for (target, valid) in [
        (Pos::new(1, 6), true),
        (Pos::new(1, 7), false),
        (Pos::new(3, 2), true),
        (Pos::new(2, 2), false),
        (Pos::new(3, 1), false),
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
}

#[test]
fn necromancer_has_only_orthogonal_two_ray_move_and_no_take() {
    let g = game(
        &[("necromancer", 0, 3, 3), ("pawn", 1, 4, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    for (kind, target, valid) in [
        (ActionKind::Move, Pos::new(3, 5), true),
        (ActionKind::Move, Pos::new(3, 6), false),
        (ActionKind::Move, Pos::new(4, 4), false),
        (ActionKind::Take, Pos::new(4, 3), false),
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
fn oni_lands_beyond_friendly_or_hostile_heroic_in_all_directions() {
    for d in SubDirection::ALL {
        let (dx, dy) = d.vector();
        for owner in [0, 1] {
            let mut g = game(
                &[("oni", 0, 3, 3), ("golem", owner, 3 + 2 * dx, 3 + 2 * dy)],
                &["mana"],
                &["mana"],
                0,
            );
            let source = id(&g, 3, 3);
            cast(&mut g, source, "seek", Some(Pos::new(3 + dx, 3 + dy)));
            assert_eq!(
                g.piece(source).unwrap().position,
                Pos::new(3 + 3 * dx, 3 + 3 * dy)
            );
            assert_eq!(
                g.at(Pos::new(3 + 2 * dx, 3 + 2 * dy)).unwrap().state.damage,
                0
            );
        }
    }
}

#[test]
fn oni_stops_at_nonheroic_and_uses_fallback_when_heroic_far_side_is_blocked() {
    for (name, wall, expected) in [
        ("wall", false, Pos::new(4, 3)),
        ("king", true, Pos::new(4, 3)),
        ("king", false, Pos::new(6, 3)),
    ] {
        let mut pieces = vec![("oni", 0, 3, 3), (name, 1, 5, 3)];
        if wall {
            pieces.push(("wall", 0, 6, 3));
        }
        let mut g = game(&pieces, &["mana"], &["mana"], 0);
        let source = id(&g, 3, 3);
        cast(&mut g, source, "seek", Some(Pos::new(4, 3)));
        assert_eq!(g.piece(source).unwrap().position, expected);
    }
    let mut g = game(&[("oni", 0, 3, 3)], &["mana"], &["mana"], 0);
    let source = id(&g, 3, 3);
    cast(&mut g, source, "seek", Some(Pos::new(4, 3)));
    assert_eq!(g.piece(source).unwrap().position, Pos::new(7, 3));
}

#[test]
fn oni_no_op_is_rejected_and_preview_replays_identically() {
    let mut g = game(
        &[("oni", 0, 3, 3), ("wall", 1, 4, 3)],
        &["mana"],
        &["mana"],
        0,
    );
    let source = id(&g, 3, 3);
    let before = g.snapshot();
    assert!(g
        .apply(ActionIntent::Ability {
            player: 0,
            source,
            ability: "seek".into(),
            target: Some(Pos::new(4, 3))
        })
        .is_err());
    assert_eq!(g.snapshot(), before);
    let intent = ActionIntent::Ability {
        player: 0,
        source,
        ability: "seek".into(),
        target: Some(Pos::new(3, 4)),
    };
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    assert_eq!(g.apply(intent).unwrap(), preview);
}

#[test]
fn revival_creates_new_identity_consumes_corpse_and_serializes_links() {
    let mut g = revival_ready("warlock");
    let corpse = g.dead()[0].piece.id;
    let caster = id(&g, 3, 3);
    let before = g.snapshot();
    let intent = revive(&g, Pos::new(3, 4));
    let preview = g.preview(&intent).unwrap();
    assert_eq!(g.snapshot(), before);
    let mut replay = g.clone();
    assert_eq!(g.apply(intent.clone()).unwrap(), preview);
    assert_eq!(replay.apply(intent).unwrap(), preview);
    let raised = g.at(Pos::new(3, 4)).unwrap();
    let raised_id = raised.id;
    assert_ne!(raised_id, corpse);
    assert!(g.piece(corpse).is_none());
    assert_eq!(g.dead()[0].revived_as, Some(raised_id));
    let link = raised.revival.as_ref().unwrap();
    assert_eq!(link.previous_entity, corpse);
    assert_eq!(link.revived_by, caster);
    assert_eq!(link.death_index, 0);
    assert!(g.has_tag(raised_id, Tag::Undead));
    assert!(g.has_tag(raised_id, Tag::Demonic));
    assert!(!g.has_tag(raised_id, Tag::Immune));
    assert!(raised.state.acted);
    assert_eq!(raised.state.damage, 0);
    assert_eq!(g.snapshot(), replay.snapshot());
    assert_eq!(
        ron::from_str::<Snapshot>(&ron::to_string(&g.snapshot()).unwrap()).unwrap(),
        g.snapshot()
    );
    for _ in 0..7 {
        pass(&mut g);
    }
    let before = g.snapshot();
    assert!(g.apply(revive(&g, Pos::new(4, 3))).is_err());
    assert_eq!(g.snapshot(), before);
}

#[test]
fn revival_resets_mutable_stats_statuses_cooldowns_and_once_flags() {
    let reg = registry(|pieces, _| {
        let p = pieces.iter_mut().find(|p| p.id == "warlock").unwrap();
        p.tags.remove(&Tag::Immune);
        p.tags.insert(Tag::Biological);
        p.tags.insert(Tag::Heroic);
        p.abilities=vec![ron::from_str(r#"(id: "prepare", display_name: "Prepare", target: (origin: Source, pattern: SelfOnly, filters: [Friendly,Occupied], count: 1), once: true, cooldown: 5, effects: [ModifyStat(stat: Strength, amount: 5),AddStatus(kind: Ice, duration: 3)])"#).unwrap()];
    });
    let mut g = setup(
        reg,
        &[
            ("necromancer", 0, 3, 3),
            ("warlock", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    let warlock = id(&g, 0, 2);
    cast(&mut g, warlock, "prepare", Some(Pos::new(0, 2)));
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    let raised = g.at(Pos::new(3, 4)).unwrap();
    assert_eq!(raised.stats.strength, 0);
    assert!(raised.statuses.is_empty());
    assert!(raised.state.cooldowns.is_empty());
    assert!(raised.state.used_abilities.is_empty());
    for tag in [Tag::Biological, Tag::Heroic, Tag::Immune] {
        assert!(!g.has_tag(raised.id, tag));
        assert!(raised.suppressed_tags.contains(&tag));
    }
}

#[test]
fn latest_friendly_death_is_selected_before_older_corpses() {
    let mut g = game(
        &[
            ("necromancer", 0, 3, 3),
            ("pawn", 0, 0, 2),
            ("imp", 0, 1, 3),
            ("cannon", 1, 0, 7),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    let cannon = id(&g, 0, 7);
    pass(&mut g);
    act(&mut g, cannon, ActionKind::Attack, Pos::new(0, 2));
    pass(&mut g);
    act(&mut g, cannon, ActionKind::Attack, Pos::new(1, 3));
    g.apply(revive(&g, Pos::new(4, 3))).unwrap();
    assert_eq!(g.at(Pos::new(4, 3)).unwrap().archetype, "imp");
    assert!(g.dead()[0].revived_as.is_none());
    for _ in 0..7 {
        pass(&mut g);
    }
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    assert_eq!(g.at(Pos::new(3, 4)).unwrap().archetype, "pawn");
}

#[test]
fn empty_history_occupied_and_out_of_range_revivals_are_atomic() {
    let mut empty = game(&[("necromancer", 0, 3, 3)], &["mana"], &["mana"], 0);
    assert!(empty.apply(revive(&empty, Pos::new(3, 4))).is_err());
    let mut g = revival_ready("imp");
    let before = g.snapshot();
    for target in [Pos::new(3, 3), Pos::new(0, 2), Pos::new(7, 3)] {
        assert!(g.apply(revive(&g, target)).is_err());
        assert_eq!(g.snapshot(), before);
    }
}

#[test]
fn revived_egg_gets_new_hatch_job_old_job_cannot_retarget_it() {
    let mut g = revival_ready("spider_egg");
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    let raised = id(&g, 3, 4);
    pass(&mut g);
    assert_eq!(g.piece(raised).unwrap().archetype, "spider_egg");
    pass(&mut g);
    pass(&mut g);
    assert_eq!(g.piece(raised).unwrap().archetype, "spider");
    assert!(g.has_tag(raised, Tag::Undead));
    assert!(!g.has_tag(raised, Tag::Biological));
}

#[test]
fn undead_promotion_preserves_suppression_and_identity_link() {
    let mut g = game(
        &[
            ("necromancer", 0, 3, 5),
            ("pawn", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    let source = id(&g, 3, 5);
    cast(&mut g, source, "revive", Some(Pos::new(3, 7)));
    let pending = g.pending_promotion().unwrap().clone();
    g.apply(ActionIntent::ChoosePromotion {
        player: 0,
        token: pending.token,
        archetype: "queen".into(),
    })
    .unwrap();
    assert!(g.has_tag(pending.piece, Tag::Undead));
    assert!(!g.has_tag(pending.piece, Tag::Biological));
    assert!(g.piece(pending.piece).unwrap().revival.is_some());
}

#[test]
fn dead_card_matchers_use_effective_tags_of_undead_corpses() {
    let reg = registry(|_, cards| {
        cards
            .iter_mut()
            .find(|c| c.id == "blood_pact")
            .unwrap()
            .triggers[0]
            .event = CardMatcher::Removed(Tag::Biological);
    });
    let mut g = setup(
        reg,
        &[
            ("necromancer", 0, 3, 3),
            ("pawn", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["blood_pact"],
        &["mana"],
        1,
        0,
    );
    play(&mut g, "blood_pact", Default::default());
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    g.apply(revive(&g, Pos::new(2, 2))).unwrap();
    let mana = g.player(0).unwrap().mana.clone();
    act(&mut g, rook, ActionKind::Take, Pos::new(2, 2));
    assert_eq!(g.player(0).unwrap().mana, mana);
    assert!(g
        .effective_tags(&g.dead().last().unwrap().piece)
        .contains(&Tag::Undead));
}

#[test]
fn revival_failure_rolls_back_consumed_record_and_allocated_identity() {
    let reg = registry(|pieces, _| {
        let imp = pieces.iter_mut().find(|p| p.id == "imp").unwrap();
        imp.triggers.push(TriggerDef {
            id: "on_spawn".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::AddMana(1); 1000],
        });
        // Exactly 1024 setup effects fit; the additional revive effect exceeds the cap.
        imp.triggers.push(TriggerDef {
            id: "more".into(),
            event: TriggerEvent::Spawned,
            effects: vec![EffectDef::AddMana(1); 24],
        });
    });
    let mut g = setup(
        reg,
        &[
            ("necromancer", 0, 3, 3),
            ("imp", 0, 0, 2),
            ("rook", 1, 0, 7),
        ],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    let before = g.snapshot();
    assert_eq!(
        g.apply(revive(&g, Pos::new(3, 4))),
        Err(RuleError::ResolutionLimit)
    );
    assert_eq!(g.snapshot(), before);
}

#[test]
fn revive_and_seek_schemas_reject_wrong_selection_and_nesting() {
    for name in ["oni", "necromancer"] {
        let text = if name == "oni" {
            include_str!("../assets/oni.ron")
        } else {
            include_str!("../assets/necromancer.ron")
        };
        let mut p: PieceArchetype = ron::from_str(text).unwrap();
        p.abilities[0].effects = vec![EffectDef::Schedule {
            after: 1,
            effects: p.abilities[0].effects.clone(),
        }];
        let s = ron::to_string(&p).unwrap();
        assert!(Registry::load([("bad.ron", s.as_str())]).is_err());
    }
}

#[test]
fn undead_heroic_suppression_changes_onis_landing_rule() {
    let reg = registry(|pieces, _| {
        pieces
            .iter_mut()
            .find(|p| p.id == "golem")
            .unwrap()
            .base_stats
            .tough = 1
    });
    let mut g = setup(
        reg,
        &[
            ("necromancer", 0, 3, 3),
            ("golem", 0, 0, 2),
            ("rook", 1, 0, 7),
            ("oni", 1, 1, 4),
        ],
        &["mana"],
        &["mana"],
        0,
        0,
    );
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    let oni = id(&g, 1, 4);
    cast(&mut g, oni, "seek", Some(Pos::new(2, 4)));
    assert_eq!(g.piece(oni).unwrap().position, Pos::new(2, 4));
}

#[test]
fn revived_immune_piece_can_receive_hostile_abilities() {
    let mut g = game(
        &[
            ("necromancer", 0, 3, 3),
            ("warlock", 0, 0, 2),
            ("rook", 1, 0, 7),
            ("witch", 1, 4, 4),
        ],
        &["mana"],
        &["mana"],
        0,
    );
    pass(&mut g);
    let rook = id(&g, 0, 7);
    act(&mut g, rook, ActionKind::Take, Pos::new(0, 2));
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    let raised = id(&g, 3, 4);
    let witch = id(&g, 4, 4);
    let trace = cast(&mut g, witch, "burst", None);
    assert!(trace
        .events
        .iter()
        .any(|e| matches!(e,Event::DamageDealt{target,..} if *target==raised)));
    assert!(g.piece(raised).is_none());
}

#[test]
fn each_new_death_can_be_revived_once_with_a_new_link() {
    let mut g = revival_ready("imp");
    g.apply(revive(&g, Pos::new(2, 2))).unwrap();
    let first = id(&g, 2, 2);
    let rook = id(&g, 0, 2);
    act(&mut g, rook, ActionKind::Take, Pos::new(2, 2));
    for _ in 0..6 {
        pass(&mut g);
    }
    g.apply(revive(&g, Pos::new(3, 4))).unwrap();
    let second = g.at(Pos::new(3, 4)).unwrap();
    assert_ne!(first, second.id);
    assert_eq!(second.revival.as_ref().unwrap().previous_entity, first);
    assert_eq!(g.dead()[1].revived_as, Some(second.id));
}

#[test]
fn royal_tag_survives_undead_suppression_in_four_player_mode() {
    use cchess_engine::state::{Placement, Player};
    let mut g = Game::new(
        Registry::built_in().unwrap(),
        Bounds {
            width: 14,
            height: 14,
        },
        [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .enumerate()
            .map(|(id, d)| {
                let mut p = Player::new(id as u8, d);
                p.mana = 10.into();
                p
            })
            .collect(),
        vec![
            Placement::new("necromancer", 0, Pos::new(6, 6)),
            Placement::new("king", 0, Pos::new(3, 5)),
            Placement::new("king", 0, Pos::new(10, 10)),
            Placement::new("rook", 1, Pos::new(3, 10)),
            Placement::new("king", 2, Pos::new(10, 3)),
            Placement::new("king", 3, Pos::new(3, 3)),
        ],
    )
    .unwrap();
    pass(&mut g);
    let rook = id(&g, 3, 10);
    act(&mut g, rook, ActionKind::Take, Pos::new(3, 5));
    pass(&mut g);
    pass(&mut g);
    assert!(!g.is_eliminated(0));
    let source = id(&g, 6, 6);
    cast(&mut g, source, "revive", Some(Pos::new(6, 7)));
    let raised = id(&g, 6, 7);
    assert!(g.has_tag(raised, Tag::Royal));
    assert!(g.has_tag(raised, Tag::Undead));
    assert!(!g.has_tag(raised, Tag::Immune));
    assert_eq!(
        g.pieces()
            .filter(|p| p.owner == 0 && g.has_tag(p.id, Tag::Royal))
            .count(),
        2
    );
}
