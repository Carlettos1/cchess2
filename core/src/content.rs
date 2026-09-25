use crate::geometry::{Pattern, Relative, RuntimePattern};
use crate::state::StatusKind;
use crate::targeting::{TargetFilter, TargetOrigin, TargetSpec};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActionKind {
    Move,
    Take,
    Attack,
    Ability,
    PlayCard,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tag {
    Biological,
    Structure,
    Demonic,
    Undead,
    Immune,
    Heroic,
    Royal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stats {
    pub tough: u32,
    pub strength: u32,
    pub transportable: Option<u32>,
    pub impenetrable: u32,
}
impl Default for Stats {
    fn default() -> Self {
        Self {
            tough: 1,
            strength: 0,
            transportable: None,
            impenetrable: 0,
        }
    }
}

/// Mutable numeric combat stats. Health/transport changes have separate semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stat {
    Strength,
    Impenetrable,
}
impl Stats {
    pub fn get(&self, stat: Stat) -> u32 {
        match stat {
            Stat::Strength => self.strength,
            Stat::Impenetrable => self.impenetrable,
        }
    }
    pub(crate) fn modify(&mut self, stat: Stat, amount: i32) -> (u32, u32) {
        let value = match stat {
            Stat::Strength => &mut self.strength,
            Stat::Impenetrable => &mut self.impenetrable,
        };
        let before = *value;
        *value = value.saturating_add_signed(amount);
        (before, *value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparison {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}
impl Comparison {
    pub fn test(self, a: u32, b: u32) -> bool {
        match self {
            Self::Lt => a < b,
            Self::Le => a <= b,
            Self::Eq => a == b,
            Self::Ge => a >= b,
            Self::Gt => a > b,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatCompare {
    pub target: Stat,
    pub comparison: Comparison,
    pub source: Stat,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraverseEffect {
    DamageTarget(u32),
    DamageSource(u32),
    MoveSourceToLastEmpty,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraverseDef {
    pub stop_when: StatCompare,
    pub on_pass: Vec<TraverseEffect>,
    pub on_stop: Vec<TraverseEffect>,
    pub on_end: Vec<TraverseEffect>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActionCost {
    pub actions: u32,
    pub mana: u32,
}
impl Default for ActionCost {
    fn default() -> Self {
        Self {
            actions: 1,
            mana: 0,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    HasNotActed,
    NoPendingWork,
    ManaAtLeast(u32),
    HasTag(Tag),
    CardOnBoard(String),
    And(Vec<Condition>),
    Or(Vec<Condition>),
    Not(Box<Condition>),
}

/// Minimal generic effect vocabulary; the executor never dispatches on archetype.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectDef {
    MoveSource,
    RemoveSource,
    DamageTarget(u32),
    MoveSourceIfTargetRemoved,
    AddStatus {
        kind: StatusKind,
        duration: u32,
    },
    RemoveStatuses,
    SpawnRelative {
        archetype: String,
        directions: Vec<Relative>,
    },
    AddMana(u32),
    Sequence(Vec<EffectDef>),
    ForEach {
        target: TargetSpec<Pattern>,
        effects: Vec<EffectDef>,
    },
    Schedule {
        after: u32,
        effects: Vec<EffectDef>,
    },
    TransformSource(String),
    ModifyStat {
        stat: Stat,
        amount: i32,
    },
    /// Select a cardinal facing; all relative placements must be available.
    SpawnOriented {
        archetype: String,
        directions: Vec<Relative>,
    },
    /// Select an adjacent square as a ray direction, independent of its occupant.
    Traverse(TraverseDef),
    SeekTag(Tag),
    AdvancePeers {
        halfside: u16,
    },
    ReviveLatest {
        undead: bool,
    },
    Transport {
        max_range: u16,
        collision_damage: u32,
    },
    Push {
        distance: u16,
        collision_damage: u32,
    },
    SpawnTarget(String),
    DrawCard(u32),
    ShuffleDeck,
    ModifyActions(u32),
    /// Permanently increase the owner's allowance, including the active turn.
    IncreaseActionsPerTurn(u32),
    ResetAction,
    ModifyScheduledEvent(u32),
    CancelScheduledEvent,
    RemoveBoardCard,
    ScheduleOn {
        clock: crate::clock::ClockUnit,
        after: u32,
        effects: Vec<EffectDef>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerEvent {
    Spawned,
    Removed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerDef {
    pub id: String,
    pub event: TriggerEvent,
    pub effects: Vec<EffectDef>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    #[default]
    One,
    All,
    Direction,
    Transport,
    ScheduledSummon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityDef {
    pub id: String,
    pub display_name: String,
    pub target: TargetSpec<Pattern>,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub cost: ActionCost,
    #[serde(default)]
    pub cooldown: u32,
    #[serde(default)]
    pub once: bool,
    #[serde(default)]
    pub consume_cards: Vec<String>,
    pub effects: Vec<EffectDef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDef {
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledAbility {
    pub definition: AbilityDef,
    pub target: TargetSpec<RuntimePattern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRule {
    pub kind: ActionKind,
    pub target: TargetSpec<Pattern>,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub cost: ActionCost,
    pub effects: Vec<EffectDef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceArchetype {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub symbol: char,
    #[serde(default)]
    pub tags: BTreeSet<Tag>,
    #[serde(default)]
    pub base_stats: Stats,
    /// Select one seeded legal ray direction for Move and Take together.
    #[serde(default)]
    pub random_ray: bool,
    pub action_rules: Vec<ActionRule>,
    #[serde(default)]
    pub abilities: Vec<AbilityDef>,
    #[serde(default)]
    pub promotion: Option<PromotionDef>,
    #[serde(default)]
    pub triggers: Vec<TriggerDef>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledRule {
    pub kind: ActionKind,
    pub target: TargetSpec<RuntimePattern>,
    pub conditions: Vec<Condition>,
    pub cost: ActionCost,
    pub effects: Vec<EffectDef>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPiece {
    pub definition: PieceArchetype,
    pub rules: Vec<CompiledRule>,
    pub abilities: Vec<CompiledAbility>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pieces: BTreeMap<String, CompiledPiece>,
    pub(crate) cards: BTreeMap<String, crate::cards::CompiledCard>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub id: Option<String>,
    pub field: String,
    pub message: String,
}
impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {}: {}",
            self.file,
            self.id.as_deref().unwrap_or("?"),
            self.field,
            self.message
        )
    }
}
impl Registry {
    /// All files are inspected before returning diagnostics. No partial registry escapes.
    pub fn load<'a>(
        sources: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut pieces = BTreeMap::new();
        let mut files = BTreeMap::new();
        let mut errors = Vec::new();
        for (file, source) in sources {
            let definition: PieceArchetype = match ron::from_str(source) {
                Ok(d) => d,
                Err(e) => {
                    errors.push(Diagnostic {
                        file: file.into(),
                        id: None,
                        field: "parse".into(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            let mut report = |field: String, message: &str| {
                errors.push(Diagnostic {
                    file: file.into(),
                    id: Some(definition.id.clone()),
                    field,
                    message: message.into(),
                })
            };
            if definition.schema_version != 1 {
                report("schema_version".into(), "expected version 1");
            }
            if definition.id.is_empty()
                || !definition
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                report("id".into(), "use a nonempty lowercase ASCII identifier");
            }
            if pieces.contains_key(&definition.id) {
                report("id".into(), "duplicate archetype identifier");
            }
            if definition.display_name.trim().is_empty() {
                report("display_name".into(), "must not be empty");
            }
            if !definition.symbol.is_ascii_uppercase() {
                report("symbol".into(), "expected an uppercase ASCII letter");
            }
            if definition.base_stats.tough == 0 {
                report("base_stats.tough".into(), "must be at least one");
            }
            if definition.random_ray
                && (definition.action_rules.is_empty()
                    || definition.action_rules.iter().any(|r| {
                        !matches!(r.kind, ActionKind::Move | ActionKind::Take)
                            || !matches!(r.target.pattern, Pattern::Ray { .. })
                    }))
            {
                report(
                    "random_ray".into(),
                    "random selection requires ordinary Ray Move/Take rules",
                );
            }
            let mut kinds = BTreeSet::new();
            let mut rules = Vec::new();
            for (i, r) in definition.action_rules.iter().enumerate() {
                let path = format!("action_rules[{i}]");
                if !kinds.insert(r.kind) {
                    report(path.clone(), "duplicate action kind");
                }
                if r.target.count != 1 {
                    report(
                        format!("{path}.target.count"),
                        "core actions require exactly one target",
                    );
                }
                if r.target.origin != TargetOrigin::Source {
                    report(
                        format!("{path}.target.origin"),
                        "core piece actions must originate at Source",
                    );
                }
                if r.cost.actions == 0 {
                    report(
                        format!("{path}.cost.actions"),
                        "core actions cost at least one action",
                    );
                }
                let required = match r.kind {
                    ActionKind::Move => vec![TargetFilter::Empty],
                    _ => vec![TargetFilter::Enemy, TargetFilter::Occupied],
                };
                if !required.iter().all(|f| r.target.filters.contains(f)) {
                    report(
                        format!("{path}.target.filters"),
                        "missing required occupancy/ownership filters",
                    );
                }
                let filters = &r.target.filters;
                if (filters.contains(&TargetFilter::Empty)
                    && filters.iter().any(|f| {
                        !matches!(
                            f,
                            TargetFilter::Empty | TargetFilter::Magic | TargetFilter::Buildable
                        )
                    }))
                    || (filters.contains(&TargetFilter::Friendly)
                        && filters.contains(&TargetFilter::Enemy))
                {
                    report(
                        format!("{path}.target.filters"),
                        "contradictory target filters",
                    );
                }
                let valid = match (r.kind, r.effects.as_slice()) {
                    (ActionKind::Move, [EffectDef::MoveSource]) => true,
                    (
                        ActionKind::Take,
                        [EffectDef::DamageTarget(n), EffectDef::MoveSourceIfTargetRemoved],
                    ) => *n > 0,
                    (ActionKind::Attack, [EffectDef::DamageTarget(n)]) => *n > 0,
                    _ => false,
                };
                if !valid {
                    report(
                        format!("{path}.effects"),
                        "effects must preserve Move/Take/Attack semantics with positive damage",
                    );
                }
                match r.target.pattern.normalize() {
                    Ok(pattern) => rules.push(CompiledRule {
                        kind: r.kind,
                        target: TargetSpec {
                            origin: r.target.origin.clone(),
                            pattern,
                            filters: r.target.filters.clone(),
                            count: r.target.count,
                        },
                        conditions: r.conditions.clone(),
                        cost: r.cost,
                        effects: r.effects.clone(),
                    }),
                    Err(e) => report(format!("{path}.target.pattern"), &e),
                }
            }
            let mut abilities = Vec::new();
            let mut ability_ids = BTreeSet::new();
            for (i, ability) in definition.abilities.iter().enumerate() {
                let path = format!("abilities[{i}]");
                if !valid_id(&ability.id) || !ability_ids.insert(&ability.id) {
                    report(
                        format!("{path}.id"),
                        "ability IDs must be valid and unique within the archetype",
                    );
                }
                if ability.display_name.trim().is_empty() {
                    report(format!("{path}.display_name"), "must not be empty");
                }
                if ability.cost.actions == 0 {
                    report(
                        format!("{path}.cost.actions"),
                        "abilities require a positive action cost",
                    );
                }
                if ability.target.count != 1 {
                    report(
                        format!("{path}.target.count"),
                        "use one required target; selection All affects all eligible targets",
                    );
                }
                if ability.consume_cards.iter().any(|id| !valid_id(id))
                    || ability.consume_cards.iter().collect::<BTreeSet<_>>().len()
                        != ability.consume_cards.len()
                {
                    report(
                        format!("{path}.consume_cards"),
                        "card identifiers must be valid and unique",
                    );
                }
                let filters = &ability.target.filters;
                if ability.selection == Selection::Direction
                    && (ability.target.origin != TargetOrigin::Source
                        || !filters.is_empty()
                        || !matches!(
                            &ability.target.pattern,
                            Pattern::FullStep | Pattern::Step(_)
                        ))
                {
                    report(
                        format!("{path}.target"),
                        "direction selection requires an unfiltered Source Step or FullStep",
                    );
                }
                if (filters.contains(&TargetFilter::Empty)
                    && filters.iter().any(|f| {
                        !matches!(
                            f,
                            TargetFilter::Empty | TargetFilter::Magic | TargetFilter::Buildable
                        )
                    }))
                    || (filters.contains(&TargetFilter::Friendly)
                        && filters.contains(&TargetFilter::Enemy))
                {
                    report(
                        format!("{path}.target.filters"),
                        "contradictory target filters",
                    );
                }
                if let Err(error) = validate_effects(
                    &ability.effects,
                    &ability.target,
                    ability.selection,
                    false,
                    0,
                ) {
                    report(format!("{path}.effects"), &error);
                }
                match ability.target.pattern.normalize() {
                    Ok(pattern) => abilities.push(CompiledAbility {
                        definition: ability.clone(),
                        target: TargetSpec {
                            origin: ability.target.origin.clone(),
                            pattern,
                            filters: filters.clone(),
                            count: ability.target.count,
                        },
                    }),
                    Err(e) => report(format!("{path}.target.pattern"), &e),
                }
            }
            let mut trigger_ids = BTreeSet::new();
            for (i, trigger) in definition.triggers.iter().enumerate() {
                if !valid_id(&trigger.id) || !trigger_ids.insert(&trigger.id) {
                    report(
                        format!("triggers[{i}].id"),
                        "trigger IDs must be valid and unique",
                    );
                }
                let target = TargetSpec {
                    origin: TargetOrigin::Source,
                    pattern: Pattern::SelfOnly,
                    filters: vec![TargetFilter::Friendly, TargetFilter::Occupied],
                    count: 1,
                };
                if let Err(error) = validate_effects(
                    &trigger.effects,
                    &target,
                    Selection::One,
                    trigger.event == TriggerEvent::Removed,
                    0,
                ) {
                    report(format!("triggers[{i}].effects"), &error);
                }
            }
            if let Some(promotion) = &definition.promotion {
                if promotion.choices.is_empty()
                    || promotion.choices.iter().collect::<BTreeSet<_>>().len()
                        != promotion.choices.len()
                {
                    report(
                        "promotion.choices".into(),
                        "promotion needs unique, nonempty choices",
                    );
                }
                if definition.tags.contains(&Tag::Royal) {
                    report("promotion".into(), "Royal pieces cannot promote");
                }
            }
            files.insert(definition.id.clone(), file.to_string());
            pieces.insert(
                definition.id.clone(),
                CompiledPiece {
                    definition,
                    rules,
                    abilities,
                },
            );
        }
        for (id, piece) in &pieces {
            for (i, rule) in piece.definition.action_rules.iter().enumerate() {
                for (j, filter) in rule.target.filters.iter().enumerate() {
                    if let TargetFilter::Archetype(target) = filter {
                        if !pieces.contains_key(target) {
                            errors.push(Diagnostic {
                                file: files[id].clone(),
                                id: Some(id.clone()),
                                field: format!("action_rules[{i}].target.filters[{j}]"),
                                message: format!("unknown archetype: {target}"),
                            });
                        }
                    }
                }
            }
            let mut report = |field: String, message: String| {
                errors.push(Diagnostic {
                    file: files[id].clone(),
                    id: Some(id.clone()),
                    field,
                    message,
                })
            };
            for (i, ability) in piece.definition.abilities.iter().enumerate() {
                for (j, filter) in ability.target.filters.iter().enumerate() {
                    if let TargetFilter::Archetype(target) = filter {
                        if !pieces.contains_key(target) {
                            report(
                                format!("abilities[{i}].target.filters[{j}]"),
                                format!("unknown archetype: {target}"),
                            );
                        }
                    }
                }
                effect_references(
                    &ability.effects,
                    &pieces,
                    &piece.definition,
                    format!("abilities[{i}].effects"),
                    &mut report,
                );
            }
            for (i, trigger) in piece.definition.triggers.iter().enumerate() {
                effect_references(
                    &trigger.effects,
                    &pieces,
                    &piece.definition,
                    format!("triggers[{i}].effects"),
                    &mut report,
                );
            }
            if let Some(promotion) = &piece.definition.promotion {
                for (i, choice) in promotion.choices.iter().enumerate() {
                    match pieces.get(choice) {
                        None => report(
                            format!("promotion.choices[{i}]"),
                            format!("unknown archetype: {choice}"),
                        ),
                        Some(p)
                            if p.definition.tags.contains(&Tag::Royal)
                                || p.definition.promotion.is_some() =>
                        {
                            report(
                                format!("promotion.choices[{i}]"),
                                "cannot promote to a Royal or promoting archetype".into(),
                            )
                        }
                        _ => (),
                    }
                }
            }
        }
        if pieces.is_empty() && errors.is_empty() {
            errors.push(Diagnostic {
                file: "registry".into(),
                id: None,
                field: "pieces".into(),
                message: "at least one archetype is required".into(),
            });
        }
        if errors.is_empty() {
            Ok(Self {
                pieces,
                cards: BTreeMap::new(),
            })
        } else {
            Err(errors)
        }
    }
    pub fn get(&self, id: &str) -> Option<&CompiledPiece> {
        self.pieces.get(id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &CompiledPiece> {
        self.pieces.values()
    }
    pub fn classical() -> Result<Self, Vec<Diagnostic>> {
        Self::load(CLASSICAL_SOURCES.iter().copied())
    }
    pub fn built_in() -> Result<Self, Vec<Diagnostic>> {
        Self::load(
            CLASSICAL_SOURCES
                .iter()
                .chain(CUSTOM_SOURCES.iter())
                .copied(),
        )?
        .with_cards(crate::cards::CARD_SOURCES.iter().copied())
    }
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub const CLASSICAL_SOURCES: [(&str, &str); 6] = [
    ("pawn.ron", include_str!("../../assets/pawn.ron")),
    ("knight.ron", include_str!("../../assets/knight.ron")),
    ("bishop.ron", include_str!("../../assets/bishop.ron")),
    ("rook.ron", include_str!("../../assets/rook.ron")),
    ("queen.ron", include_str!("../../assets/queen.ron")),
    ("king.ron", include_str!("../../assets/king.ron")),
];
pub const CUSTOM_SOURCES: [(&str, &str); 30] = [
    (
        "super_pawn.ron",
        include_str!("../../assets/super_pawn.ron"),
    ),
    ("succubus.ron", include_str!("../../assets/succubus.ron")),
    ("magician.ron", include_str!("../../assets/magician.ron")),
    ("paladin.ron", include_str!("../../assets/paladin.ron")),
    (
        "mandragora.ron",
        include_str!("../../assets/mandragora.ron"),
    ),
    (
        "tesla_tower.ron",
        include_str!("../../assets/tesla_tower.ron"),
    ),
    ("spider.ron", include_str!("../../assets/spider.ron")),
    (
        "spider_egg.ron",
        include_str!("../../assets/spider_egg.ron"),
    ),
    ("witch.ron", include_str!("../../assets/witch.ron")),
    ("archer.ron", include_str!("../../assets/archer.ron")),
    (
        "shield_bearer.ron",
        include_str!("../../assets/shield_bearer.ron"),
    ),
    ("builder.ron", include_str!("../../assets/builder.ron")),
    ("wall.ron", include_str!("../../assets/wall.ron")),
    ("ballista.ron", include_str!("../../assets/ballista.ron")),
    ("ram.ron", include_str!("../../assets/ram.ron")),
    (
        "crazy_pawn.ron",
        include_str!("../../assets/crazy_pawn.ron"),
    ),
    ("cannon.ron", include_str!("../../assets/cannon.ron")),
    ("ship.ron", include_str!("../../assets/ship.ron")),
    ("warlock.ron", include_str!("../../assets/warlock.ron")),
    ("portal.ron", include_str!("../../assets/portal.ron")),
    ("catapult.ron", include_str!("../../assets/catapult.ron")),
    ("imp.ron", include_str!("../../assets/imp.ron")),
    ("basilisk.ron", include_str!("../../assets/basilisk.ron")),
    ("gargoyle.ron", include_str!("../../assets/gargoyle.ron")),
    ("mermaid.ron", include_str!("../../assets/mermaid.ron")),
    ("golem.ron", include_str!("../../assets/golem.ron")),
    ("ogre.ron", include_str!("../../assets/ogre.ron")),
    ("oni.ron", include_str!("../../assets/oni.ron")),
    ("dragon.ron", include_str!("../../assets/dragon.ron")),
    (
        "necromancer.ron",
        include_str!("../../assets/necromancer.ron"),
    ),
];

pub(crate) fn validate_effects(
    effects: &[EffectDef],
    target: &TargetSpec<Pattern>,
    selection: Selection,
    removed: bool,
    depth: usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err("effect nesting exceeds 16 levels".into());
    }
    if effects.is_empty() {
        return Err("effect lists cannot be empty".into());
    }
    if selection == Selection::Direction
        && (depth != 0
            || effects.len() != 1
            || !matches!(
                effects[0],
                EffectDef::SpawnOriented { .. }
                    | EffectDef::Traverse(_)
                    | EffectDef::SeekTag(_)
                    | EffectDef::AdvancePeers { .. }
            ))
    {
        return Err(
            "direction selection requires one immediate oriented spawn, traversal or tag search"
                .into(),
        );
    }
    if selection == Selection::Transport
        && (depth != 0 || effects.len() != 1 || !matches!(effects[0], EffectDef::Transport { .. }))
    {
        return Err("transport selection requires one immediate Transport effect".into());
    }
    if selection == Selection::ScheduledSummon
        && (depth != 0
            || effects.len() != 1
            || !matches!(effects[0], EffectDef::ModifyScheduledEvent(_)))
    {
        return Err("scheduled summon selection requires one immediate event reduction".into());
    }
    let occupied = target.filters.contains(&TargetFilter::Occupied);
    let self_target = selection == Selection::One
        && target.origin == TargetOrigin::Source
        && target.pattern == Pattern::SelfOnly
        && target.filters.contains(&TargetFilter::Friendly);
    for effect in effects {
        if removed
            && !matches!(
                effect,
                EffectDef::AddMana(_)
                    | EffectDef::DrawCard(_)
                    | EffectDef::ShuffleDeck
                    | EffectDef::Sequence(_)
                    | EffectDef::Schedule { .. }
                    | EffectDef::ScheduleOn { .. }
            )
        {
            return Err("removed-piece triggers support player effects and scheduling only".into());
        }
        let valid = match effect {
            EffectDef::AdvancePeers { halfside } => {
                selection == Selection::Direction
                    && depth == 0
                    && effects.len() == 1
                    && (1..=63).contains(halfside)
            }
            EffectDef::SeekTag(_) => {
                selection == Selection::Direction && depth == 0 && effects.len() == 1
            }
            EffectDef::ReviveLatest { .. } => {
                selection == Selection::One
                    && depth == 0
                    && effects.len() == 1
                    && target.filters.contains(&TargetFilter::Empty)
            }
            EffectDef::Push {
                distance,
                collision_damage,
            } => {
                selection == Selection::One
                    && depth == 0
                    && effects.len() == 1
                    && occupied
                    && target.origin == TargetOrigin::Source
                    && target.pattern == Pattern::FullStep
                    && (1..=64).contains(distance)
                    && *collision_damage > 0
            }
            EffectDef::Transport {
                max_range,
                collision_damage,
            } => {
                selection == Selection::Transport
                    && depth == 0
                    && occupied
                    && target.origin == TargetOrigin::Source
                    && target.pattern == Pattern::FullStep
                    && (1..=64).contains(max_range)
                    && *collision_damage > 0
            }
            EffectDef::MoveSource => {
                depth == 0
                    && effects.len() == 1
                    && selection == Selection::One
                    && target.filters.contains(&TargetFilter::Empty)
            }
            EffectDef::RemoveSource => self_target,
            EffectDef::DamageTarget(n) => *n > 0 && occupied,
            EffectDef::DrawCard(n) => *n > 0 && *n <= 256,
            EffectDef::ShuffleDeck => true,
            EffectDef::ModifyActions(n) | EffectDef::IncreaseActionsPerTurn(n) => *n > 0,
            EffectDef::ResetAction => occupied,
            EffectDef::SpawnTarget(id) => {
                valid_id(id) && target.filters.contains(&TargetFilter::Empty)
            }
            // Piece abilities bind scheduled summons; other non-square targets belong to cards.
            EffectDef::ModifyScheduledEvent(amount) => {
                *amount > 0
                    && depth == 0
                    && selection == Selection::ScheduledSummon
                    && occupied
                    && target.filters.contains(&TargetFilter::Friendly)
                    && target.origin == TargetOrigin::Source
                    && target.pattern == Pattern::FullStep
            }
            EffectDef::CancelScheduledEvent | EffectDef::RemoveBoardCard => false,
            EffectDef::ModifyStat { amount, .. } => *amount != 0 && occupied,
            EffectDef::SpawnOriented {
                archetype,
                directions,
            } => {
                let cardinal = matches!(&target.pattern, Pattern::Step(ds)
                    if ds.iter().all(|d| matches!(d, crate::geometry::SubDirection::N | crate::geometry::SubDirection::E | crate::geometry::SubDirection::S | crate::geometry::SubDirection::W)));
                selection == Selection::Direction
                    && depth == 0
                    && cardinal
                    && valid_id(archetype)
                    && !directions.is_empty()
                    && directions
                        .iter()
                        .enumerate()
                        .all(|(i, d)| !directions[..i].contains(d))
            }
            EffectDef::Traverse(def) => {
                let valid_effect = |e: &TraverseEffect| match e {
                    TraverseEffect::DamageTarget(n) | TraverseEffect::DamageSource(n) => *n > 0,
                    TraverseEffect::MoveSourceToLastEmpty => true,
                };
                selection == Selection::Direction
                    && depth == 0
                    && (!def.on_pass.is_empty()
                        || !def.on_stop.is_empty()
                        || !def.on_end.is_empty())
                    && def
                        .on_pass
                        .iter()
                        .chain(&def.on_stop)
                        .chain(&def.on_end)
                        .all(valid_effect)
                    && !def
                        .on_end
                        .iter()
                        .any(|e| matches!(e, TraverseEffect::DamageTarget(_)))
            }
            EffectDef::AddStatus { duration, .. } => *duration > 0 && occupied,
            EffectDef::RemoveStatuses => occupied,
            EffectDef::SpawnRelative {
                archetype,
                directions,
            } => self_target && valid_id(archetype) && !directions.is_empty(),
            EffectDef::TransformSource(archetype) => self_target && valid_id(archetype),
            EffectDef::AddMana(amount) => *amount > 0,
            EffectDef::Sequence(children) => {
                validate_effects(children, target, selection, removed, depth + 1)?;
                true
            }
            EffectDef::Schedule {
                after,
                effects: children,
            }
            | EffectDef::ScheduleOn {
                after,
                effects: children,
                ..
            } => {
                if *after == 0 {
                    return Err("scheduled delay must be at least one owner turn".into());
                }
                validate_effects(children, target, selection, removed, depth + 1)?;
                true
            }
            EffectDef::ForEach {
                target: spec,
                effects: children,
            } => {
                spec.pattern.normalize()?;
                if spec.count != 1 {
                    return Err("ForEach requires count 1 per resolved target".into());
                }
                if (spec.filters.contains(&TargetFilter::Empty)
                    && spec.filters.iter().any(|f| {
                        !matches!(
                            f,
                            TargetFilter::Empty | TargetFilter::Magic | TargetFilter::Buildable
                        )
                    }))
                    || (spec.filters.contains(&TargetFilter::Friendly)
                        && spec.filters.contains(&TargetFilter::Enemy))
                {
                    return Err("contradictory ForEach filters".into());
                }
                validate_effects(children, spec, Selection::All, false, depth + 1)?;
                true
            }
            EffectDef::MoveSourceIfTargetRemoved => false,
        };
        if !valid {
            return Err(
                "unsafe effect or incompatible target filters, selection or duration".into(),
            );
        }
    }
    Ok(())
}
fn effect_references(
    effects: &[EffectDef],
    pieces: &BTreeMap<String, CompiledPiece>,
    source: &PieceArchetype,
    path: String,
    report: &mut impl FnMut(String, String),
) {
    for (i, effect) in effects.iter().enumerate() {
        let field = format!("{path}[{i}]");
        match effect {
            EffectDef::SpawnRelative { archetype, .. }
            | EffectDef::SpawnOriented { archetype, .. }
            | EffectDef::SpawnTarget(archetype)
            | EffectDef::TransformSource(archetype) => match pieces.get(archetype) {
                None => report(
                    field,
                    format!("unknown spawn/transform archetype: {archetype}"),
                ),
                Some(p)
                    if p.definition.tags.contains(&Tag::Royal)
                        || (matches!(effect, EffectDef::TransformSource(_))
                            && source.tags.contains(&Tag::Royal)) =>
                {
                    report(
                        field,
                        "spawn/transform cannot create or replace Royal pieces".into(),
                    )
                }
                _ => (),
            },
            EffectDef::Sequence(children)
            | EffectDef::Schedule {
                effects: children, ..
            }
            | EffectDef::ScheduleOn {
                effects: children, ..
            } => effect_references(children, pieces, source, field, report),
            EffectDef::ForEach {
                target,
                effects: children,
            } => {
                for filter in &target.filters {
                    if let TargetFilter::Archetype(id) = filter {
                        if !pieces.contains_key(id) {
                            report(field.clone(), format!("unknown archetype: {id}"));
                        }
                    }
                }
                effect_references(children, pieces, source, field, report);
            }
            _ => (),
        }
    }
}
