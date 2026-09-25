use cchess_engine::{
    content::{EffectDef, PieceArchetype, CLASSICAL_SOURCES},
    geometry::{Pattern, SubDirection},
    targeting::TargetFilter,
    Registry,
};

fn pawn() -> PieceArchetype {
    ron::from_str(CLASSICAL_SOURCES[0].1).unwrap()
}
#[test]
fn duplicate_ids_and_unknown_fields_are_rejected_with_file_diagnostics() {
    let text = CLASSICAL_SOURCES[0].1;
    let errors = Registry::load([
        ("first.ron", text),
        ("duplicate.ron", text),
        ("typo.ron", "(unknown_field: 1)"),
    ])
    .unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.file == "duplicate.ron" && e.field == "id" && e.message.contains("duplicate")));
    assert!(errors
        .iter()
        .any(|e| e.file == "typo.ron" && e.field == "parse"));
}
#[test]
fn validation_collects_multiple_errors_with_field_paths() {
    let mut p = pawn();
    p.schema_version = 2;
    p.base_stats.tough = 0;
    p.action_rules[0].target.count = 0;
    p.action_rules[0].cost.actions = 0;
    p.action_rules[0].target.pattern = Pattern::Ray {
        min: 5,
        max: Some(1),
        directions: vec![SubDirection::N],
    };
    let text = ron::to_string(&p).unwrap();
    let errors = Registry::load([("broken.ron", text.as_str())]).unwrap_err();
    for field in [
        "schema_version",
        "base_stats.tough",
        "action_rules[0].target.count",
        "action_rules[0].cost.actions",
        "action_rules[0].target.pattern",
    ] {
        assert!(
            errors.iter().any(|e| e.file == "broken.ron"
                && e.id.as_deref() == Some("pawn")
                && e.field == field),
            "missing diagnostic for {field}"
        );
    }
}
#[test]
fn content_cannot_bypass_capture_order_or_occupancy_rules() {
    for effects in [
        vec![EffectDef::MoveSource],
        vec![
            EffectDef::MoveSourceIfTargetRemoved,
            EffectDef::DamageTarget(1),
        ],
        vec![
            EffectDef::DamageTarget(0),
            EffectDef::MoveSourceIfTargetRemoved,
        ],
    ] {
        let mut p = pawn();
        p.action_rules[1].effects = effects;
        let text = ron::to_string(&p).unwrap();
        assert!(Registry::load([("pawn.ron", text.as_str())])
            .unwrap_err()
            .iter()
            .any(|e| e.field == "action_rules[1].effects"));
    }
    let mut p = pawn();
    p.action_rules[1].target.filters = vec![TargetFilter::Occupied];
    let text = ron::to_string(&p).unwrap();
    assert!(Registry::load([("pawn.ron", text.as_str())]).is_err());
}
#[test]
fn cross_references_and_contradictions_are_checked() {
    let mut p = pawn();
    p.action_rules[1]
        .target
        .filters
        .push(TargetFilter::Archetype("missing".into()));
    let text = ron::to_string(&p).unwrap();
    assert!(Registry::load([("pawn.ron", text.as_str())])
        .unwrap_err()
        .iter()
        .any(|e| e.message.contains("unknown archetype: missing")));
    let mut p = pawn();
    p.action_rules[0]
        .target
        .filters
        .push(TargetFilter::Occupied);
    let text = ron::to_string(&p).unwrap();
    assert!(Registry::load([("pawn.ron", text.as_str())])
        .unwrap_err()
        .iter()
        .any(|e| e.message.contains("contradictory")));
}
#[test]
fn schema_is_explicit_and_empty_catalogs_are_rejected() {
    let without_version = CLASSICAL_SOURCES[0].1.replace("schema_version: 1,", "");
    assert!(Registry::load([("pawn.ron", without_version.as_str())]).is_err());
    assert!(Registry::load([]).is_err());
}

fn all_definitions() -> Vec<PieceArchetype> {
    CLASSICAL_SOURCES
        .iter()
        .chain(cchess_engine::content::CUSTOM_SOURCES.iter())
        .map(|(_, text)| ron::from_str(text).unwrap())
        .collect()
}

#[test]
fn nested_effects_and_lifecycle_definitions_are_validated() {
    let mut definitions = all_definitions();
    let plant = definitions
        .iter_mut()
        .find(|p| p.id == "mandragora")
        .unwrap();
    plant.abilities[0].effects = vec![EffectDef::Sequence(vec![EffectDef::Schedule {
        after: 0,
        effects: vec![EffectDef::AddMana(1)],
    }])];
    plant.triggers.push(plant.triggers[0].clone());
    plant.triggers[0].effects = vec![EffectDef::DamageTarget(1)];
    let egg = definitions
        .iter_mut()
        .find(|p| p.id == "spider_egg")
        .unwrap();
    egg.triggers
        .iter_mut()
        .find(|t| t.event == cchess_engine::content::TriggerEvent::Spawned)
        .unwrap()
        .effects = vec![EffectDef::Schedule {
        after: 1,
        effects: vec![EffectDef::TransformSource("missing".into())],
    }];
    let errors = diagnostics(&definitions);
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("mandragora")
            && e.field.starts_with("abilities[0].effects")));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("mandragora") && e.field == "triggers[1].id"));
    assert!(errors.iter().any(
        |e| e.id.as_deref() == Some("mandragora") && e.field.starts_with("triggers[0].effects")
    ));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("spider_egg") && e.message.contains("missing")));
}

#[test]
fn directional_content_rejects_unsafe_input_and_placement_definitions() {
    use cchess_engine::content::Selection;
    let mut definitions = all_definitions();
    let builder = definitions.iter_mut().find(|p| p.id == "builder").unwrap();
    builder.abilities[0].target.filters = vec![TargetFilter::Occupied];
    builder.abilities[0].effects = vec![EffectDef::SpawnOriented {
        archetype: "missing".into(),
        directions: vec![cchess_engine::geometry::Relative::F; 2],
    }];
    let ballista = definitions.iter_mut().find(|p| p.id == "ballista").unwrap();
    ballista.abilities[0].selection = Selection::All;
    let errors = diagnostics(&definitions);
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("builder") && e.field == "abilities[0].target"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("builder") && e.field == "abilities[0].effects"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("builder") && e.message.contains("missing")));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("ballista") && e.field == "abilities[0].effects"));
}

#[test]
fn stat_changes_and_traversal_hooks_require_meaningful_safe_effects() {
    use cchess_engine::content::{Stat, TraverseEffect};
    let mut definitions = all_definitions();
    definitions
        .iter_mut()
        .find(|p| p.id == "shield_bearer")
        .unwrap()
        .abilities[0]
        .effects = vec![EffectDef::ModifyStat {
        stat: Stat::Impenetrable,
        amount: 0,
    }];
    if let EffectDef::Traverse(t) = &mut definitions
        .iter_mut()
        .find(|p| p.id == "ram")
        .unwrap()
        .abilities[0]
        .effects[0]
    {
        t.on_pass = vec![TraverseEffect::DamageTarget(0)];
        t.on_end = vec![TraverseEffect::DamageTarget(1)]; // no target at ray end
    }
    let errors = diagnostics(&definitions);
    for id in ["shield_bearer", "ram"] {
        assert!(errors
            .iter()
            .any(|e| e.id.as_deref() == Some(id) && e.field == "abilities[0].effects"));
    }
}
fn diagnostics(definitions: &[PieceArchetype]) -> Vec<cchess_engine::content::Diagnostic> {
    let texts: Vec<_> = definitions
        .iter()
        .map(|d| (d.id.clone(), ron::to_string(d).unwrap()))
        .collect();
    Registry::load(texts.iter().map(|(id, text)| (id.as_str(), text.as_str()))).unwrap_err()
}
#[test]
fn ability_ids_durations_and_target_compatibility_are_validated() {
    let mut definitions = all_definitions();
    let magician = definitions.iter_mut().find(|p| p.id == "magician").unwrap();
    magician.abilities[0].effects = vec![EffectDef::AddStatus {
        kind: cchess_engine::state::StatusKind::Fire,
        duration: 0,
    }];
    magician.abilities[1].id = "fire".into();
    let queen = definitions.iter_mut().find(|p| p.id == "queen").unwrap();
    queen.abilities[0].target.filters = vec![TargetFilter::Occupied];
    let errors = diagnostics(&definitions);
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("magician") && e.field == "abilities[0].effects"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("magician") && e.field == "abilities[1].id"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("queen") && e.field == "abilities[0].effects"));
}
#[test]
fn promotion_and_spawn_references_cannot_be_missing_or_promote_to_royalty() {
    let mut definitions = all_definitions();
    definitions
        .iter_mut()
        .find(|p| p.id == "pawn")
        .unwrap()
        .promotion
        .as_mut()
        .unwrap()
        .choices = vec!["missing".into(), "king".into(), "pawn".into()];
    definitions
        .iter_mut()
        .find(|p| p.id == "knight")
        .unwrap()
        .abilities[0]
        .effects = vec![EffectDef::SpawnRelative {
        archetype: "missing".into(),
        directions: vec![cchess_engine::geometry::Relative::L],
    }];
    let errors = diagnostics(&definitions);
    assert!(errors
        .iter()
        .any(|e| e.field == "promotion.choices[0]" && e.message.contains("unknown")));
    for i in [1, 2] {
        assert!(errors
            .iter()
            .any(|e| e.field == format!("promotion.choices[{i}]")));
    }
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("knight") && e.message.contains("spawn")));
}
#[test]
fn empty_duplicate_choices_and_unsafe_effect_sequences_are_rejected() {
    let mut definitions = all_definitions();
    definitions
        .iter_mut()
        .find(|p| p.id == "pawn")
        .unwrap()
        .promotion
        .as_mut()
        .unwrap()
        .choices
        .clear();
    let paladin = definitions.iter_mut().find(|p| p.id == "paladin").unwrap();
    paladin.abilities[0]
        .consume_cards
        .push("invulnerability".into());
    let queen = definitions.iter_mut().find(|p| p.id == "queen").unwrap();
    queen.abilities[0].effects.push(EffectDef::DamageTarget(1));
    let errors = diagnostics(&definitions);
    assert!(errors.iter().any(|e| e.field == "promotion.choices"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("paladin") && e.field == "abilities[0].consume_cards"));
    assert!(errors
        .iter()
        .any(|e| e.id.as_deref() == Some("queen") && e.field == "abilities[0].effects"));
}
