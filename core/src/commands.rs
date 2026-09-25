use crate::{
    content::{ActionKind, Stat, TriggerEvent},
    geometry::Pos,
    state::{ActionSource, CardId, EffectSource, EntityId, PlayerId, StatusKind},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ActionIntent {
    ScheduledAbility {
        player: PlayerId,
        source: EntityId,
        ability: String,
        scheduled: u64,
    },
    Transport {
        player: PlayerId,
        source: EntityId,
        ability: String,
        target: Pos,
        destination: Pos,
    },
    Piece {
        player: PlayerId,
        source: EntityId,
        kind: ActionKind,
        target: Pos,
    },
    EndTurn {
        player: PlayerId,
    },
    Ability {
        player: PlayerId,
        source: EntityId,
        ability: String,
        target: Option<Pos>,
    },
    PlayCard {
        player: PlayerId,
        card: CardId,
        target: crate::cards::CardChoice,
    },
    ChoosePromotion {
        player: PlayerId,
        token: u64,
        archetype: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    PieceRevived {
        piece: EntityId,
        previous: EntityId,
        death_index: usize,
        archetype: String,
        position: Pos,
        cause: EffectSource,
    },
    PlayerEliminated {
        player: PlayerId,
    },
    GameDrawn,
    TransportStarted {
        source: EntityId,
        target: EntityId,
        from: Pos,
        destination: Pos,
    },
    TransportCollision {
        source: EntityId,
        target: EntityId,
        blocker: EntityId,
        position: Pos,
    },
    RandomDirectionChosen {
        source: EntityId,
        direction: (i32, i32),
    },
    CardCreated {
        card: CardId,
        owner: PlayerId,
        definition: String,
        zone: crate::cards::CardZone,
    },
    ClockAdvanced {
        clock: crate::clock::ClockUnit,
        value: u64,
    },
    CardPlayed {
        card: CardId,
        player: PlayerId,
        definition: String,
    },
    CardResolved {
        card: CardId,
    },
    CardMoved {
        card: CardId,
        owner: PlayerId,
        from: crate::cards::CardZone,
        to: crate::cards::CardZone,
        cause: EffectSource,
    },
    DeckShuffled {
        player: PlayerId,
        order: Vec<CardId>,
        cause: EffectSource,
    },
    DrawSkipped {
        player: PlayerId,
        count: u32,
        cause: EffectSource,
    },
    ActionsPerTurnModified {
        player: PlayerId,
        before: u32,
        after: u32,
        cause: EffectSource,
    },
    ActionsModified {
        player: PlayerId,
        before: u32,
        after: u32,
        cause: EffectSource,
    },
    ActionReset {
        piece: EntityId,
        cause: EffectSource,
    },
    ScheduledChanged {
        id: u64,
        before: u64,
        after: u64,
        cause: EffectSource,
    },
    ScheduledCancelled {
        id: u64,
        cause: EffectSource,
    },
    CardTriggerFired {
        card: CardId,
        trigger: String,
        parent: EffectSource,
    },
    AbilityDiscountApplied {
        source: EntityId,
        card: CardId,
        amount: u32,
    },
    StatModified {
        piece: EntityId,
        stat: Stat,
        before: u32,
        after: u32,
        cause: EffectSource,
    },
    TraversalStarted {
        source: EntityId,
        from: Pos,
        direction: (i32, i32),
    },
    TraversalVisited {
        source: EntityId,
        position: Pos,
    },
    TraversalStopped {
        source: EntityId,
        blocker: EntityId,
        position: Pos,
    },
    TraversalEnded {
        source: EntityId,
        last_empty: Pos,
    },
    ActionStarted {
        source: Option<EntityId>,
        kind: Option<ActionKind>,
    },
    CostsPaid {
        player: PlayerId,
        actions: u32,
        mana: u32,
    },
    DamageDealt {
        source: ActionSource,
        target: EntityId,
        kind: ActionKind,
        amount: u32,
        total: u32,
        cause: EffectSource,
    },
    PieceRemoved {
        piece: EntityId,
        source: ActionSource,
    },
    PieceMoved {
        piece: EntityId,
        from: Pos,
        to: Pos,
    },
    ActionResolved {
        source: EntityId,
        kind: ActionKind,
    },
    TurnEnded {
        player: PlayerId,
    },
    TurnStarted {
        player: PlayerId,
        turn: u64,
    },
    GameWon {
        player: PlayerId,
    },
    AbilityResolved {
        source: EntityId,
        ability: String,
    },
    AbilityUsed {
        source: EntityId,
        ability: String,
        cooldown: u32,
        once: bool,
    },
    CooldownReady {
        piece: EntityId,
        ability: String,
    },
    StatusApplied {
        piece: EntityId,
        kind: StatusKind,
        duration: u32,
        refreshed: bool,
        cause: EffectSource,
    },
    StatusRemoved {
        piece: EntityId,
        kind: StatusKind,
        expired: bool,
    },
    PieceSpawned {
        piece: EntityId,
        archetype: String,
        owner: PlayerId,
        position: Pos,
    },
    PromotionRequired {
        token: u64,
        piece: EntityId,
        choices: Vec<String>,
    },
    PieceTransformed {
        piece: EntityId,
        from: String,
        to: String,
    },
    BoardCardConsumed {
        player: PlayerId,
        card: String,
    },
    ManaGained {
        player: PlayerId,
        amount: u32,
    },
    EffectsScheduled {
        id: u64,
        owner: PlayerId,
        due_tick: u64,
        clock: crate::clock::ClockUnit,
        cause: EffectSource,
    },
    ScheduledReady {
        id: u64,
    },
    ScheduledResolved {
        id: u64,
    },
    TriggerFired {
        piece: EntityId,
        trigger: String,
        event: TriggerEvent,
        parent: EffectSource,
    },
    EffectSkipped {
        cause: EffectSource,
        reason: String,
    },
    ManaGranted {
        player: PlayerId,
        amount: u32,
        cause: EffectSource,
    },
}

/// The enclosing record gives every event its action ID, source intent, and order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAction {
    pub id: u64,
    pub intent: ActionIntent,
    pub events: Vec<Event>,
}
