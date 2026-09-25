//! Card definitions, unique instances, ordered zones and deterministic deck state.
use crate::{
    clock::ClockUnit,
    commands::{ActionIntent, Event},
    content::{
        valid_id, validate_effects, ActionCost, ActionKind, Condition, Diagnostic, EffectDef,
        Registry, Selection, Tag,
    },
    effects::ResolutionContext,
    events::{BoundTarget, Frame},
    geometry::{Pattern, Pos, RuntimePattern},
    rules::RuleError,
    state::{ActionSource, CardId, EffectSource, Game, PlayerId},
    targeting::{self, TargetFilter, TargetOrigin, TargetSpec},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardKind {
    Summon,
    Utility,
    OnBoard,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardTarget {
    #[default]
    None,
    Square(TargetSpec<Pattern>),
    FriendlyScheduled,
    EnemyBoardCard,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CardChoice {
    #[default]
    None,
    Square(Pos),
    Scheduled(u64),
    Card(CardId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardZone {
    Deck,
    Hand,
    Discard,
    OnBoard,
    Consumed,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardZones {
    /// Index zero is the next card drawn.
    pub deck: Vec<CardId>,
    pub hand: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub on_board: Vec<CardId>,
    pub consumed: Vec<CardId>,
}
impl CardZones {
    pub fn get(&self, zone: CardZone) -> &[CardId] {
        match zone {
            CardZone::Deck => &self.deck,
            CardZone::Hand => &self.hand,
            CardZone::Discard => &self.discard,
            CardZone::OnBoard => &self.on_board,
            CardZone::Consumed => &self.consumed,
        }
    }
    fn get_mut(&mut self, zone: CardZone) -> &mut Vec<CardId> {
        match zone {
            CardZone::Deck => &mut self.deck,
            CardZone::Hand => &mut self.hand,
            CardZone::Discard => &mut self.discard,
            CardZone::OnBoard => &mut self.on_board,
            CardZone::Consumed => &mut self.consumed,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    pub id: CardId,
    pub definition: String,
    pub owner: PlayerId,
    pub zone: CardZone,
    pub trigger_uses: BTreeMap<String, (u64, u64)>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardMatcher {
    Clock(ClockUnit),
    Removed(Tag),
    FriendlyRemoved(Tag),
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerLimit {
    #[default]
    OncePerEvent,
    OncePerAction,
    OncePerTurn,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardTrigger {
    pub id: String,
    pub event: CardMatcher,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub limit: TriggerLimit,
    pub effects: Vec<EffectDef>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityDiscount {
    pub source_tag: Tag,
    pub mana: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardDef {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub kind: CardKind,
    pub cost: ActionCost,
    #[serde(default)]
    pub target: CardTarget,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub effects: Vec<EffectDef>,
    #[serde(default)]
    pub triggers: Vec<CardTrigger>,
    #[serde(default)]
    pub ability_discounts: Vec<AbilityDiscount>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCard {
    pub definition: CardDef,
    pub target: Option<TargetSpec<RuntimePattern>>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckDef {
    pub schema_version: u32,
    pub id: String,
    pub cards: Vec<String>,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckRules {
    pub draw_per_turn: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRng {
    state: u64,
}
impl MatchRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    /// Return a deterministic random index below a positive bound.
    pub fn below(&mut self, bound: u64) -> usize {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let n = self.next();
            if n >= threshold {
                return (n % bound) as usize;
            }
        }
    }
}

pub const CARD_SOURCES: [(&str, &str); 20] = [
    (
        "lasting_momentum.ron",
        include_str!("../../assets/cards/lasting_momentum.ron"),
    ),
    (
        "attack_demonic.ron",
        include_str!("../../assets/cards/attack_demonic.ron"),
    ),
    ("revive.ron", include_str!("../../assets/cards/revive.ron")),
    (
        "summon_dragon.ron",
        include_str!("../../assets/cards/summon_dragon.ron"),
    ),
    (
        "summon_knight.ron",
        include_str!("../../assets/cards/summon_knight.ron"),
    ),
    (
        "summon_wall.ron",
        include_str!("../../assets/cards/summon_wall.ron"),
    ),
    (
        "forced_march.ron",
        include_str!("../../assets/cards/forced_march.ron"),
    ),
    (
        "second_wind.ron",
        include_str!("../../assets/cards/second_wind.ron"),
    ),
    (
        "event_haste.ron",
        include_str!("../../assets/cards/event_haste.ron"),
    ),
    (
        "cancel_event.ron",
        include_str!("../../assets/cards/cancel_event.ron"),
    ),
    ("fire.ron", include_str!("../../assets/cards/fire.ron")),
    ("ice.ron", include_str!("../../assets/cards/ice.ron")),
    (
        "invulnerability.ron",
        include_str!("../../assets/cards/invulnerability.ron"),
    ),
    (
        "blood_pact.ron",
        include_str!("../../assets/cards/blood_pact.ron"),
    ),
    (
        "royal_decree.ron",
        include_str!("../../assets/cards/royal_decree.ron"),
    ),
    (
        "more_mana.ron",
        include_str!("../../assets/cards/more_mana.ron"),
    ),
    (
        "supply_cache.ron",
        include_str!("../../assets/cards/supply_cache.ron"),
    ),
    (
        "insight.ron",
        include_str!("../../assets/cards/insight.ron"),
    ),
    ("mana.ron", include_str!("../../assets/cards/mana.ron")),
    ("dispel.ron", include_str!("../../assets/cards/dispel.ron")),
];

impl Registry {
    pub fn card(&self, id: &str) -> Option<&CompiledCard> {
        self.cards.get(id)
    }
    pub fn cards(&self) -> impl Iterator<Item = &CompiledCard> {
        self.cards.values()
    }
    pub fn with_cards<'a>(
        mut self,
        sources: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut errors = Vec::new();
        let mut files = BTreeMap::new();
        for (file, text) in sources {
            let def: CardDef = match ron::from_str(text) {
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
            let mut report = |field: &str, message: String| {
                errors.push(Diagnostic {
                    file: file.into(),
                    id: Some(def.id.clone()),
                    field: field.into(),
                    message,
                })
            };
            files.insert(def.id.clone(), file.to_string());
            if def.schema_version != 1 {
                report("schema_version", "expected version 1".into());
            }
            if !valid_id(&def.id) || self.cards.contains_key(&def.id) {
                report("id", "invalid or duplicate card ID".into());
            }
            if def.display_name.trim().is_empty() {
                report("display_name", "must not be empty".into());
            }
            if def.conditions.iter().any(|c| !player_condition(c)) {
                report(
                    "conditions",
                    "card conditions must refer to player mana or board cards".into(),
                );
            }
            if let CardTarget::Square(spec) = &def.target {
                if spec.origin == TargetOrigin::Source || spec.count != 1 {
                    report(
                        "target",
                        "cards require a non-piece origin and one selected square".into(),
                    );
                }
                if spec.filters.contains(&TargetFilter::Empty)
                    && spec.filters.iter().any(|f| *f != TargetFilter::Empty)
                    || spec.filters.contains(&TargetFilter::Friendly)
                        && spec.filters.contains(&TargetFilter::Enemy)
                {
                    report("target.filters", "contradictory filters".into());
                }
            }
            if def.kind == CardKind::OnBoard {
                if def.target != CardTarget::None || !def.effects.is_empty() {
                    report(
                        "effects",
                        "on-board cards enter their zone; use triggers for their effects".into(),
                    );
                }
            } else {
                if def.effects.is_empty() {
                    report("effects", "immediate cards require effects".into());
                }
                if !def.triggers.is_empty() || !def.ability_discounts.is_empty() {
                    report(
                        "kind",
                        "only on-board cards supply triggers/discounts".into(),
                    );
                }
                if let Err(e) = self.validate_card_effects(&def.effects, &def.target, 0) {
                    report("effects", e);
                }
            }
            if def.kind == CardKind::Summon
                && !matches!(def.effects.as_slice(), [EffectDef::SpawnTarget(_)])
            {
                report(
                    "effects",
                    "summon cards require one SpawnTarget effect".into(),
                );
            }
            let mut ids = BTreeSet::new();
            for trigger in &def.triggers {
                if !valid_id(&trigger.id) || !ids.insert(&trigger.id) {
                    report("triggers.id", "invalid or duplicate trigger ID".into());
                }
                if let Err(e) = self.validate_card_effects(&trigger.effects, &CardTarget::None, 0) {
                    report("triggers.effects", e);
                }
            }
            if def.ability_discounts.iter().any(|d| d.mana == 0) {
                report("ability_discounts", "discounts must be positive".into());
            }
            let target = if let CardTarget::Square(spec) = &def.target {
                match spec.pattern.normalize() {
                    Ok(pattern) => Some(TargetSpec {
                        origin: spec.origin.clone(),
                        pattern,
                        filters: spec.filters.clone(),
                        count: spec.count,
                    }),
                    Err(e) => {
                        report("target.pattern", e);
                        None
                    }
                }
            } else {
                None
            };
            self.cards.insert(
                def.id.clone(),
                CompiledCard {
                    definition: def,
                    target,
                },
            );
        }
        // Resolve unlock conditions only after all card files have been read.
        for card in self.cards.values() {
            let mut references = Vec::new();
            condition_card_ids(&card.definition.conditions, &mut references);
            for id in references {
                if self
                    .card(id)
                    .is_none_or(|c| c.definition.kind != CardKind::OnBoard)
                {
                    errors.push(Diagnostic {
                        file: files
                            .get(&card.definition.id)
                            .cloned()
                            .unwrap_or_else(|| "cards".into()),
                        id: Some(card.definition.id.clone()),
                        field: "conditions".into(),
                        message: format!("unknown or non-persistent board card: {id}"),
                    });
                }
            }
        }
        if errors.is_empty() {
            Ok(self)
        } else {
            Err(errors)
        }
    }
    fn validate_card_effects(
        &self,
        effects: &[EffectDef],
        target: &CardTarget,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 16 || effects.is_empty() {
            return Err("empty effects or nesting exceeds 16".into());
        }
        let player_spec = TargetSpec {
            origin: TargetOrigin::Board,
            pattern: Pattern::SelfOnly,
            filters: vec![],
            count: 1,
        };
        let spec = match target {
            CardTarget::Square(s) => s,
            _ => &player_spec,
        };
        for e in effects {
            match e {
                EffectDef::Sequence(children) => {
                    self.validate_card_effects(children, target, depth + 1)?
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
                        return Err("delay must be positive".into());
                    }
                    self.validate_card_effects(children, target, depth + 1)?;
                }
                EffectDef::ForEach {
                    target: inner,
                    effects: children,
                } => {
                    if inner.origin == TargetOrigin::Source {
                        return Err("card selectors cannot originate at Source".into());
                    }
                    self.validate_card_effects(
                        children,
                        &CardTarget::Square(inner.clone()),
                        depth + 1,
                    )?;
                    validate_effects(std::slice::from_ref(e), spec, Selection::One, false, depth)?;
                }
                EffectDef::ModifyScheduledEvent(n)
                    if *n > 0 && *target == CardTarget::FriendlyScheduled => {}
                EffectDef::CancelScheduledEvent if *target == CardTarget::FriendlyScheduled => (),
                EffectDef::RemoveBoardCard if *target == CardTarget::EnemyBoardCard => (),
                EffectDef::SpawnTarget(id) => {
                    let p = self
                        .get(id)
                        .ok_or_else(|| format!("unknown summon archetype: {id}"))?;
                    if p.definition.tags.contains(&Tag::Royal)
                        || !spec.filters.contains(&TargetFilter::Empty)
                    {
                        return Err(
                            "summons need empty squares and cannot create Royal pieces".into()
                        );
                    }
                }
                EffectDef::AddMana(_)
                | EffectDef::DrawCard(_)
                | EffectDef::ShuffleDeck
                | EffectDef::ModifyActions(_)
                | EffectDef::IncreaseActionsPerTurn(_)
                | EffectDef::ResetAction
                | EffectDef::AddStatus { .. }
                | EffectDef::RemoveStatuses
                | EffectDef::DamageTarget(_)
                | EffectDef::ModifyStat { .. } => {
                    validate_effects(std::slice::from_ref(e), spec, Selection::One, false, depth)?;
                }
                _ => return Err("effect is incompatible with this card's target/source".into()),
            }
        }
        for filter in &spec.filters {
            if let TargetFilter::Archetype(id) = filter {
                if self.get(id).is_none() {
                    return Err(format!("unknown archetype: {id}"));
                }
            }
        }
        Ok(())
    }
    pub fn validate_deck(&self, deck: &DeckDef) -> Result<(), String> {
        if deck.schema_version != 1
            || !valid_id(&deck.id)
            || deck.cards.is_empty()
            || deck.cards.len() > 256
        {
            return Err("deck needs version 1, an ID and 1–256 cards".into());
        }
        for id in &deck.cards {
            if self.card(id).is_none() {
                return Err(format!("unknown card: {id}"));
            }
        }
        Ok(())
    }
}
fn player_condition(c: &Condition) -> bool {
    match c {
        Condition::ManaAtLeast(_) | Condition::CardOnBoard(_) => true,
        Condition::And(cs) | Condition::Or(cs) => cs.iter().all(player_condition),
        Condition::Not(c) => player_condition(c),
        _ => false,
    }
}
fn condition_card_ids<'a>(conditions: &'a [Condition], result: &mut Vec<&'a str>) {
    for c in conditions {
        match c {
            Condition::CardOnBoard(id) => result.push(id),
            Condition::And(cs) | Condition::Or(cs) => condition_card_ids(cs, result),
            Condition::Not(c) => condition_card_ids(std::slice::from_ref(c.as_ref()), result),
            _ => (),
        }
    }
}

impl Game {
    pub fn cards_demo(registry: Registry) -> Result<Self, String> {
        use crate::{
            geometry::{Bounds, Direction},
            state::{Placement, Player},
        };
        let mut placements = Vec::new();
        for (name, x, y) in [
            ("king", 4, 0),
            ("magician", 2, 2),
            ("paladin", 5, 0),
            ("mandragora", 1, 1),
            ("spider", 6, 1),
            ("spider_egg", 0, 2),
            ("pawn", 3, 3),
        ] {
            placements.push(Placement::new(name, 0, Pos::new(x, y)));
            placements.push(Placement::new(name, 1, Pos::new(7 - x, 7 - y)));
        }
        let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
            .into_iter()
            .map(|mut p| {
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p
            })
            .collect();
        let deck = DeckDef {
            schema_version: 1,
            id: "demo".into(),
            cards: [
                "forced_march",
                "summon_knight",
                "fire",
                "ice",
                "supply_cache",
                "event_haste",
                "blood_pact",
                "royal_decree",
                "more_mana",
                "second_wind",
                "insight",
                "invulnerability",
                "attack_demonic",
                "revive",
                "dispel",
                "cancel_event",
                "summon_wall",
                "mana",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        };
        Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            players,
            placements,
        )?
        .with_decks(vec![(0, deck.clone()), (1, deck)], 42, 8, 1, false)
    }
    pub fn card_instance(&self, id: CardId) -> Option<&CardInstance> {
        self.state.cards.get(&id)
    }
    pub fn card_instances(&self) -> impl Iterator<Item = &CardInstance> {
        self.state.cards.values()
    }
    pub fn has_board_card(&self, owner: PlayerId, definition: &str) -> bool {
        self.state
            .cards
            .values()
            .any(|c| c.owner == owner && c.zone == CardZone::OnBoard && c.definition == definition)
    }
    fn create_card(
        &mut self,
        owner: PlayerId,
        definition: &str,
        zone: CardZone,
    ) -> Result<CardId, String> {
        if self.registry.card(definition).is_none() {
            return Err(format!("unknown card: {definition}"));
        }
        let id = self.state.next_card;
        self.state.next_card = id.checked_add(1).ok_or("card ID limit reached")?;
        self.state.cards.insert(
            id,
            CardInstance {
                id,
                owner,
                definition: definition.into(),
                zone,
                trigger_uses: BTreeMap::new(),
            },
        );
        let player = self
            .state
            .players
            .iter_mut()
            .find(|p| p.id == owner)
            .ok_or("unknown deck owner")?;
        player.cards.get_mut(zone).push(id);
        if zone == CardZone::OnBoard {
            player.board_cards.insert(definition.into());
        }
        self.state.setup_events.push(Event::CardCreated {
            card: id,
            owner,
            definition: definition.into(),
            zone,
        });
        Ok(id)
    }
    pub(crate) fn initialize_cards(&mut self) -> Result<(), String> {
        let seeds: Vec<_> = self
            .state
            .players
            .iter()
            .map(|p| (p.id, p.board_cards.clone()))
            .collect();
        if self
            .state
            .players
            .iter()
            .any(|p| p.cards != CardZones::default())
        {
            return Err("set up decks with with_decks; raw zone IDs are not accepted".into());
        }
        for (owner, cards) in seeds {
            for name in cards {
                if self
                    .registry
                    .card(&name)
                    .is_none_or(|c| c.definition.kind != CardKind::OnBoard)
                {
                    return Err(format!("unknown or non-persistent starting card: {name}"));
                }
                self.create_card(owner, &name, CardZone::OnBoard)?;
            }
        }
        Ok(())
    }
    /// Install ordered personal decks before the first action; optional shuffle is seeded.
    pub fn with_decks(
        mut self,
        decks: Vec<(PlayerId, DeckDef)>,
        seed: u64,
        opening_hand: u32,
        draw_per_turn: u32,
        shuffle: bool,
    ) -> Result<Self, String> {
        if !self.log().is_empty()
            || decks.len() != self.state.players.len()
            || opening_hand > 256
            || draw_per_turn > 256
        {
            return Err(
                "configure every player's deck before play; draw counts must be <=256".into(),
            );
        }
        let mut owners = BTreeSet::new();
        for (owner, deck) in &decks {
            let p = self.player(*owner).ok_or("unknown deck owner")?;
            if !owners.insert(*owner)
                || !p.cards.deck.is_empty()
                || !p.cards.hand.is_empty()
                || !p.cards.discard.is_empty()
                || !p.cards.consumed.is_empty()
            {
                return Err("duplicate deck owner or decks already initialized".into());
            }
            self.registry.validate_deck(deck)?;
        }
        self.state.rng = MatchRng::new(seed);
        self.state.deck_rules = DeckRules { draw_per_turn };
        // Player order, not caller argument order, determines RNG consumption.
        let mut decks: BTreeMap<_, _> = decks.into_iter().collect();
        let owners: Vec<_> = self.state.players.iter().map(|p| p.id).collect();
        for owner in owners {
            for name in decks.remove(&owner).unwrap().cards {
                self.create_card(owner, &name, CardZone::Deck)?;
            }
            let cause = system_cause(owner, 0);
            let mut events = Vec::new();
            self.resolver = Default::default();
            if shuffle {
                self.shuffle_deck(owner, &cause, &mut events)
                    .map_err(|e| e.to_string())?;
            }
            self.draw_cards(owner, opening_hand, &cause, &mut events)
                .map_err(|e| e.to_string())?;
            self.state.setup_events.extend(events);
        }
        self.resolver = Default::default();
        Ok(self)
    }
    pub(crate) fn move_card(
        &mut self,
        id: CardId,
        to: CardZone,
        cause: &EffectSource,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let card = self
            .state
            .cards
            .get_mut(&id)
            .ok_or(RuleError::InvalidCard)?;
        let from = card.zone;
        let owner = card.owner;
        let definition = card.definition.clone();
        if from == to {
            return Ok(());
        }
        card.zone = to;
        let player = self
            .state
            .players
            .iter_mut()
            .find(|p| p.id == owner)
            .unwrap();
        player
            .cards
            .get_mut(from)
            .retain(|existing| *existing != id);
        player.cards.get_mut(to).push(id);
        if from == CardZone::OnBoard {
            player.board_cards.remove(&definition);
        }
        if to == CardZone::OnBoard {
            player.board_cards.insert(definition);
        }
        events.push(Event::CardMoved {
            card: id,
            owner,
            from,
            to,
            cause: cause.clone(),
        });
        Ok(())
    }
    pub(crate) fn draw_cards(
        &mut self,
        owner: PlayerId,
        count: u32,
        cause: &EffectSource,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        for index in 0..count {
            self.charge_resolution()?;
            let Some(id) = self.player(owner).unwrap().cards.deck.first().copied() else {
                events.push(Event::DrawSkipped {
                    player: owner,
                    count: count - index,
                    cause: cause.clone(),
                });
                break;
            };
            self.move_card(id, CardZone::Hand, cause, events)?;
        }
        Ok(())
    }
    pub(crate) fn shuffle_deck(
        &mut self,
        owner: PlayerId,
        cause: &EffectSource,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let mut deck = self.player(owner).unwrap().cards.deck.clone();
        for i in (1..deck.len()).rev() {
            self.charge_resolution()?;
            let j = self.state.rng.below((i + 1) as u64);
            deck.swap(i, j);
        }
        self.state
            .players
            .iter_mut()
            .find(|p| p.id == owner)
            .unwrap()
            .cards
            .deck = deck.clone();
        events.push(Event::DeckShuffled {
            player: owner,
            order: deck,
            cause: cause.clone(),
        });
        Ok(())
    }
    pub(crate) fn consume_board_card(
        &mut self,
        owner: PlayerId,
        definition: &str,
        cause: &EffectSource,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let id = self
            .state
            .cards
            .values()
            .find(|c| c.owner == owner && c.zone == CardZone::OnBoard && c.definition == definition)
            .map(|c| c.id)
            .ok_or_else(|| RuleError::MissingBoardCard(definition.into()))?;
        self.move_card(id, CardZone::Consumed, cause, events)
    }
    fn card_condition(&self, owner: PlayerId, c: &Condition) -> bool {
        match c {
            Condition::ManaAtLeast(n) => self.player(owner).is_some_and(|p| p.mana >= *n),
            Condition::CardOnBoard(id) => self.has_board_card(owner, id),
            Condition::And(cs) => cs.iter().all(|c| self.card_condition(owner, c)),
            Condition::Or(cs) => cs.iter().any(|c| self.card_condition(owner, c)),
            Condition::Not(c) => !self.card_condition(owner, c),
            _ => false,
        }
    }
    fn card_eligibility(&self, owner: PlayerId, id: CardId) -> Result<&CompiledCard, RuleError> {
        self.check_player(owner)?;
        let instance = self
            .card_instance(id)
            .filter(|c| c.owner == owner && c.zone == CardZone::Hand)
            .ok_or(RuleError::InvalidCard)?;
        let card = self.registry.card(&instance.definition).unwrap();
        let def = &card.definition;
        if self.turn().remaining_actions < def.cost.actions {
            return Err(RuleError::InsufficientActions {
                required: def.cost.actions,
                available: self.turn().remaining_actions,
            });
        }
        if self.player(owner).unwrap().mana < def.cost.mana {
            return Err(RuleError::InsufficientMana {
                required: def.cost.mana,
                available: self.player(owner).unwrap().mana.as_u32().unwrap(),
            });
        }
        for c in &def.conditions {
            if !self.card_condition(owner, c) {
                return Err(RuleError::ConditionFailed(c.clone()));
            }
        }
        if def.kind == CardKind::OnBoard && self.has_board_card(owner, &def.id) {
            return Err(RuleError::DuplicateBoardCard(def.id.clone()));
        }
        Ok(card)
    }
    pub(crate) fn validate_card(
        &self,
        owner: PlayerId,
        id: CardId,
        choice: &CardChoice,
    ) -> Result<(&CompiledCard, Frame), RuleError> {
        self.validate_card_targets(owner, id, choice, None)
    }
    fn validate_card_targets(
        &self,
        owner: PlayerId,
        id: CardId,
        choice: &CardChoice,
        targets: Option<&[Pos]>,
    ) -> Result<(&CompiledCard, Frame), RuleError> {
        let card = self.card_eligibility(owner, id)?;
        let def = &card.definition;
        let mut target = BoundTarget {
            destination: None,
            entity: None,
            position: Pos::new(0, 0),
            scheduled: None,
            card: None,
        };
        match (&def.target, choice) {
            (CardTarget::None, CardChoice::None) => (),
            (CardTarget::Square(_), CardChoice::Square(pos)) => {
                if !self.is_playable(*pos) {
                    return Err(RuleError::OutOfBounds(*pos));
                }
                if !targets.map_or_else(
                    || {
                        targeting::resolve(self, 0, owner, card.target.as_ref().unwrap())
                            .contains(pos)
                    },
                    |targets| targets.contains(pos),
                ) {
                    return Err(RuleError::InvalidTarget);
                }
                target.position = *pos;
                target.entity = self.at(*pos).map(|p| p.id);
                if target
                    .entity
                    .is_some_and(|id| !self.can_receive(0, id, ActionKind::PlayCard))
                {
                    return Err(RuleError::ProtectedTarget);
                }
            }
            (CardTarget::FriendlyScheduled, CardChoice::Scheduled(id)) => {
                if !self
                    .scheduled()
                    .iter()
                    .any(|j| j.id == *id && j.owner == owner)
                {
                    return Err(RuleError::InvalidScheduledTarget);
                }
                target.scheduled = Some(*id);
            }
            (CardTarget::EnemyBoardCard, CardChoice::Card(id)) => {
                if !self
                    .card_instance(*id)
                    .is_some_and(|c| c.owner != owner && c.zone == CardZone::OnBoard)
                {
                    return Err(RuleError::InvalidCard);
                }
                target.card = Some(*id);
            }
            _ => return Err(RuleError::InvalidTarget),
        }
        let cause = EffectSource {
            source: ActionSource::Card(id),
            player: owner,
            action_id: 0,
            ability: None,
            status: None,
            trigger: None,
            scheduled: None,
        };
        Ok((
            card,
            Frame {
                context: ResolutionContext {
                    action_id: 0,
                    cause,
                    kind: ActionKind::PlayCard,
                },
                target,
                source_archetype: None,
            },
        ))
    }
    pub fn legal_card_plays(&self, id: CardId) -> Vec<ActionIntent> {
        let Some(instance) = self.card_instance(id) else {
            return vec![];
        };
        if self.pending_promotion().is_some() {
            return vec![];
        }
        let Ok(card) = self.card_eligibility(self.turn().player, id) else {
            return vec![];
        };
        let resolved = card
            .target
            .as_ref()
            .map(|spec| targeting::resolve(self, 0, instance.owner, spec));
        let choices = match &card.definition.target {
            CardTarget::None => vec![CardChoice::None],
            CardTarget::Square(_) => resolved
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .map(CardChoice::Square)
                .collect(),
            CardTarget::FriendlyScheduled => self
                .scheduled()
                .iter()
                .filter(|j| j.owner == instance.owner)
                .map(|j| CardChoice::Scheduled(j.id))
                .collect(),
            CardTarget::EnemyBoardCard => self
                .card_instances()
                .filter(|c| c.owner != instance.owner && c.zone == CardZone::OnBoard)
                .map(|c| CardChoice::Card(c.id))
                .collect(),
        };
        choices
            .into_iter()
            .map(|target| ActionIntent::PlayCard {
                player: self.turn().player,
                card: id,
                target,
            })
            .filter(|intent| match intent {
                ActionIntent::PlayCard {
                    player,
                    card,
                    target,
                } => self
                    .validate_card_targets(*player, *card, target, resolved.as_deref())
                    .is_ok(),
                _ => unreachable!(),
            })
            .collect()
    }
    pub(crate) fn execute_card(
        &mut self,
        action_id: u64,
        owner: PlayerId,
        id: CardId,
        choice: &CardChoice,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let (card, mut frame) = self.validate_card(owner, id, choice)?;
        let def = card.definition.clone();
        frame.context.action_id = action_id;
        frame.context.cause.action_id = action_id;
        events.push(Event::ActionStarted {
            source: None,
            kind: Some(ActionKind::PlayCard),
        });
        self.state.turn.remaining_actions -= def.cost.actions;
        self.state
            .players
            .iter_mut()
            .find(|p| p.id == owner)
            .unwrap()
            .mana -= def.cost.mana;
        events.push(Event::CostsPaid {
            player: owner,
            actions: def.cost.actions,
            mana: def.cost.mana,
        });
        events.push(Event::CardPlayed {
            card: id,
            player: owner,
            definition: def.id,
        });
        self.move_card(
            id,
            if def.kind == CardKind::OnBoard {
                CardZone::OnBoard
            } else {
                CardZone::Discard
            },
            &frame.context.cause,
            events,
        )?;
        if !def.effects.is_empty() {
            self.enqueue(&def.effects, &frame, false)?;
            self.drain_effect_queue(events)?;
        }
        events.push(Event::CardResolved { card: id });
        if !self.is_over() {
            self.request_promotions(action_id, events);
        }
        self.finish_action(action_id, events)
    }
    pub fn effective_ability_cost(&self, source: u64, base: ActionCost) -> ActionCost {
        let discount = self
            .ability_discounts(source, base.mana)
            .iter()
            .map(|(_, n)| *n)
            .sum::<u32>();
        ActionCost {
            mana: base.mana.saturating_sub(discount),
            ..base
        }
    }
    pub(crate) fn ability_discounts(&self, source: u64, base: u32) -> Vec<(CardId, u32)> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        if self.has_tag(source, crate::content::Tag::Immune) {
            return vec![];
        }
        let mut remaining = base;
        let mut result = Vec::new();
        for card in self
            .card_instances()
            .filter(|c| c.owner == piece.owner && c.zone == CardZone::OnBoard)
        {
            for d in &self
                .registry
                .card(&card.definition)
                .unwrap()
                .definition
                .ability_discounts
            {
                if self.has_tag(source, d.source_tag) {
                    let n = d.mana.min(remaining);
                    remaining -= n;
                    if n > 0 {
                        result.push((card.id, n));
                    }
                }
            }
        }
        result
    }
    pub(crate) fn fire_removed_card_triggers(
        &mut self,
        piece: &crate::state::PieceInstance,
        context: &ResolutionContext,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let tags = self.effective_tags(piece);
        self.fire_card_triggers(
            context,
            |owner, matcher| match matcher {
                CardMatcher::Removed(tag) => tags.contains(tag),
                CardMatcher::FriendlyRemoved(tag) => owner == piece.owner && tags.contains(tag),
                _ => false,
            },
            events,
        )
    }
    pub(crate) fn fire_clock_triggers(
        &mut self,
        clock: ClockUnit,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let active = self.turn().player;
        let context = ResolutionContext {
            action_id,
            cause: system_cause(active, action_id),
            kind: ActionKind::PlayCard,
        };
        self.fire_card_triggers(&context,|owner,matcher|matches!(matcher,CardMatcher::Clock(unit)if *unit==clock&&(clock!=ClockUnit::OwnerTurns||owner==active)),events)?;
        self.drain_effect_queue(events)
    }
    fn fire_card_triggers(
        &mut self,
        parent: &ResolutionContext,
        matches: impl Fn(PlayerId, &CardMatcher) -> bool,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let mut triggers = Vec::new();
        for card in self
            .card_instances()
            .filter(|c| c.zone == CardZone::OnBoard)
        {
            for (index, trigger) in self
                .registry
                .card(&card.definition)
                .unwrap()
                .definition
                .triggers
                .iter()
                .enumerate()
            {
                if matches(card.owner, &trigger.event) {
                    triggers.push((
                        trigger.priority,
                        card.id,
                        index,
                        card.owner,
                        trigger.clone(),
                    ));
                }
            }
        }
        triggers.sort_by_key(|(priority, id, index, _, _)| (*priority, *id, *index));
        for (_, id, _, owner, trigger) in triggers {
            let now = (parent.action_id, self.turn().number);
            let card = self.state.cards.get_mut(&id).unwrap();
            if card
                .trigger_uses
                .get(&trigger.id)
                .is_some_and(|last| match trigger.limit {
                    TriggerLimit::OncePerEvent => false,
                    TriggerLimit::OncePerAction => last.0 == now.0,
                    TriggerLimit::OncePerTurn => last.1 == now.1,
                })
            {
                continue;
            }
            card.trigger_uses.insert(trigger.id.clone(), now);
            events.push(Event::CardTriggerFired {
                card: id,
                trigger: trigger.id.clone(),
                parent: parent.cause.clone(),
            });
            let cause = EffectSource {
                source: ActionSource::Card(id),
                player: owner,
                action_id: parent.action_id,
                ability: None,
                status: None,
                trigger: Some(trigger.id),
                scheduled: None,
            };
            let frame = Frame {
                context: ResolutionContext {
                    action_id: parent.action_id,
                    cause,
                    kind: ActionKind::PlayCard,
                },
                target: BoundTarget {
                    destination: None,
                    entity: None,
                    position: Pos::new(0, 0),
                    scheduled: None,
                    card: None,
                },
                source_archetype: None,
            };
            self.enqueue(&trigger.effects, &frame, false)?;
        }
        Ok(())
    }
}
pub(crate) fn system_cause(player: PlayerId, action_id: u64) -> EffectSource {
    EffectSource {
        source: ActionSource::System,
        player,
        action_id,
        ability: None,
        status: None,
        trigger: None,
        scheduled: None,
    }
}
