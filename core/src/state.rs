use crate::{
    commands::ResolvedAction,
    content::{Registry, Stats, Tag},
    geometry::{Bounds, Direction, Pos},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type EntityId = u64;
pub type PlayerId = u8;
pub type CardId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionSource {
    Piece(EntityId),
    Card(CardId),
    System,
}
impl ActionSource {
    pub fn piece(self) -> Option<EntityId> {
        if let Self::Piece(id) = self {
            Some(id)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StatusKind {
    Deactivate,
    Fire,
    Ice,
    Invulnerable,
}

/// Retained even if the caster is subsequently removed or transformed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectSource {
    pub source: ActionSource,
    pub player: PlayerId,
    pub action_id: u64,
    pub ability: Option<String>,
    pub status: Option<StatusKind>,
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub scheduled: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInstance {
    pub kind: StatusKind,
    pub remaining: u32,
    pub applied_turn: u64,
    pub source: EffectSource,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cooldown {
    pub remaining: u32,
    pub applied_turn: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPromotion {
    pub token: u64,
    pub player: PlayerId,
    pub piece: EntityId,
    pub choices: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub forward: Direction,
    pub mana: crate::mana::Mana,
    pub actions_per_turn: u32,
    #[serde(default)]
    pub mana_per_turn: u32,
    #[serde(default = "default_mana_interval")]
    pub mana_interval: u32,
    #[serde(default)]
    /// Setup seeds and a derived compatibility view. Runtime queries use instances.
    pub board_cards: BTreeSet<String>,
    #[serde(default)]
    pub cards: crate::cards::CardZones,
}
fn default_mana_interval() -> u32 {
    2
}
impl Player {
    pub fn new(id: PlayerId, forward: Direction) -> Self {
        Self {
            id,
            forward,
            mana: 1.into(),
            actions_per_turn: 1,
            mana_per_turn: 1,
            mana_interval: 2,
            board_cards: BTreeSet::new(),
            cards: Default::default(),
        }
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceState {
    pub damage: u32,
    pub acted: bool,
    #[serde(default)]
    pub cooldowns: BTreeMap<String, Cooldown>,
    #[serde(default)]
    pub used_abilities: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevivalLink {
    pub previous_entity: EntityId,
    pub death_index: usize,
    pub revived_by: EntityId,
    pub action_id: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceInstance {
    #[serde(default)]
    pub added_tags: BTreeSet<Tag>,
    #[serde(default)]
    pub suppressed_tags: BTreeSet<Tag>,
    #[serde(default)]
    pub revival: Option<RevivalLink>,
    pub id: EntityId,
    pub archetype: String,
    pub owner: PlayerId,
    pub position: Pos,
    pub stats: Stats,
    pub state: PieceState,
    #[serde(default)]
    pub statuses: BTreeMap<StatusKind, StatusInstance>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnState {
    pub player: PlayerId,
    pub number: u64,
    pub remaining_actions: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnPhase {
    Action,
    /// End-of-turn timers/clocks resolved; a promotion may precede transfer.
    Transfer,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeathRecord {
    #[serde(default)]
    pub revived_as: Option<EntityId>,
    pub piece: PieceInstance,
    pub action_id: u64,
    pub source: ActionSource,
    pub cause: EffectSource,
}

/// Serializable match state has no copies of action definitions or UI state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub(crate) ai: BTreeMap<PlayerId, crate::ai::AiController>,
    pub(crate) bounds: Bounds,
    #[serde(default)]
    pub(crate) excluded_tiles: BTreeSet<Pos>,
    pub(crate) pieces: BTreeMap<EntityId, PieceInstance>,
    pub(crate) players: Vec<Player>,
    pub(crate) turn: TurnState,
    pub(crate) dead: Vec<DeathRecord>,
    pub(crate) winner: Option<PlayerId>,
    #[serde(default)]
    pub(crate) eliminated: BTreeSet<PlayerId>,
    #[serde(default)]
    pub(crate) drawn: bool,
    pub(crate) next_entity: EntityId,
    pub(crate) log: Vec<ResolvedAction>,
    #[serde(default)]
    pub(crate) pending_promotion: Option<PendingPromotion>,
    #[serde(default)]
    pub(crate) scheduled: Vec<crate::events::ScheduledWork>,
    pub(crate) next_scheduled: u64,
    pub(crate) owner_turns: BTreeMap<PlayerId, u64>,
    pub(crate) setup_events: Vec<crate::commands::Event>,
    pub(crate) cards: BTreeMap<CardId, crate::cards::CardInstance>,
    pub(crate) next_card: CardId,
    pub(crate) deck_rules: crate::cards::DeckRules,
    #[serde(default)]
    pub(crate) magic_tiles: BTreeSet<Pos>,
    #[serde(default)]
    pub(crate) unbuildable_tiles: BTreeSet<Pos>,
    pub(crate) rng: crate::cards::MatchRng,
    pub(crate) clock: crate::clock::GameClock,
    pub(crate) pending_movement: bool,
    pub(crate) pass_requested: bool,
    pub(crate) turn_phase: TurnPhase,
}
impl Snapshot {
    /// Transactional live-state copy. Committed history stays on the original game
    /// until a successful commit transfers it, without changing snapshot serialization.
    pub(crate) fn clone_for_resolution(&self) -> Self {
        Self {
            ai: self.ai.clone(),
            bounds: self.bounds,
            excluded_tiles: self.excluded_tiles.clone(),
            pieces: self.pieces.clone(),
            players: self.players.clone(),
            turn: self.turn.clone(),
            dead: self.dead.clone(),
            winner: self.winner,
            eliminated: self.eliminated.clone(),
            drawn: self.drawn,
            next_entity: self.next_entity,
            log: Vec::new(),
            pending_promotion: self.pending_promotion.clone(),
            scheduled: self.scheduled.clone(),
            next_scheduled: self.next_scheduled,
            owner_turns: self.owner_turns.clone(),
            setup_events: self.setup_events.clone(),
            cards: self.cards.clone(),
            next_card: self.next_card,
            deck_rules: self.deck_rules.clone(),
            magic_tiles: self.magic_tiles.clone(),
            unbuildable_tiles: self.unbuildable_tiles.clone(),
            rng: self.rng.clone(),
            clock: self.clock.clone(),
            pending_movement: self.pending_movement,
            pass_requested: self.pass_requested,
            turn_phase: self.turn_phase,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    // Simulation-only counter; live games derive action IDs from their complete log.
    pub(crate) simulation_actions: Option<u64>,
    pub(crate) registry: Arc<Registry>,
    pub(crate) state: Snapshot,
    pub(crate) resolver: crate::events::Resolver,
    // Derived index, never serialized; canonical iteration still uses the piece map.
    pub(crate) occupancy: Vec<Option<EntityId>>,
}
#[derive(Debug, Clone)]
pub struct Placement {
    pub archetype: String,
    pub owner: PlayerId,
    pub position: Pos,
}
impl Placement {
    pub fn new(archetype: &str, owner: PlayerId, position: Pos) -> Self {
        Self {
            archetype: archetype.into(),
            owner,
            position,
        }
    }
}

impl Game {
    pub fn new(
        registry: Registry,
        bounds: Bounds,
        players: Vec<Player>,
        placements: Vec<Placement>,
    ) -> Result<Self, String> {
        if bounds.width == 0 || bounds.height == 0 || bounds.width > 64 || bounds.height > 64 {
            return Err("board dimensions must be between 1 and 64".into());
        }
        if !matches!(players.len(), 2 | 4)
            || players.iter().map(|p| p.id).collect::<BTreeSet<_>>().len() != players.len()
            || players.iter().any(|p| p.actions_per_turn == 0)
        {
            return Err("provide two or four distinct players with positive action budgets".into());
        }
        let corner = if players.len() == 4 {
            match (bounds.width, bounds.height) {
                (14, 14) => 3,
                (24, 24) => 4,
                _ => return Err("four-player FFA requires a 14x14 or 24x24 cross board".into()),
            }
        } else {
            0
        };
        let excluded_tiles: BTreeSet<_> = bounds
            .positions()
            .filter(|p| {
                (p.x < corner || p.x >= i32::from(bounds.width) - corner)
                    && (p.y < corner || p.y >= i32::from(bounds.height) - corner)
            })
            .collect();
        let turn = TurnState {
            player: players[0].id,
            number: 1,
            remaining_actions: players[0].actions_per_turn,
        };
        let mut pieces = BTreeMap::new();
        let mut positions = BTreeSet::new();
        let mut next_entity = 1;
        for p in placements {
            let definition = registry
                .get(&p.archetype)
                .ok_or_else(|| format!("unknown archetype: {}", p.archetype))?;
            if !bounds.contains(p.position)
                || excluded_tiles.contains(&p.position)
                || !positions.insert(p.position)
            {
                return Err(format!("invalid or occupied placement: {:?}", p.position));
            }
            if !players.iter().any(|player| player.id == p.owner) {
                return Err(format!("unknown player: {}", p.owner));
            }
            pieces.insert(
                next_entity,
                PieceInstance {
                    added_tags: Default::default(),
                    suppressed_tags: Default::default(),
                    revival: None,
                    id: next_entity,
                    archetype: p.archetype,
                    owner: p.owner,
                    position: p.position,
                    stats: definition.definition.base_stats,
                    state: PieceState::default(),
                    statuses: BTreeMap::new(),
                },
            );
            next_entity += 1;
        }
        let owner_turns = players
            .iter()
            .map(|p| (p.id, u64::from(p.id == turn.player)))
            .collect();
        let mut game = Self {
            simulation_actions: None,
            registry: Arc::new(registry),
            occupancy: vec![None; usize::from(bounds.width) * usize::from(bounds.height)],
            resolver: Default::default(),
            state: Snapshot {
                ai: BTreeMap::new(),
                bounds,
                excluded_tiles,
                pieces,
                players,
                turn,
                dead: vec![],
                winner: None,
                eliminated: BTreeSet::new(),
                drawn: false,
                next_entity,
                log: vec![],
                pending_promotion: None,
                scheduled: vec![],
                next_scheduled: 1,
                owner_turns,
                setup_events: vec![],
                cards: BTreeMap::new(),
                next_card: 1,
                deck_rules: Default::default(),
                magic_tiles: Default::default(),
                unbuildable_tiles: Default::default(),
                rng: crate::cards::MatchRng::new(0),
                clock: Default::default(),
                pending_movement: false,
                pass_requested: false,
                turn_phase: TurnPhase::Action,
            },
        };
        game.rebuild_occupancy();
        game.initialize_cards()?;
        game.initialize_triggers().map_err(|e| e.to_string())?;
        // Setup positions can also contain pawns already on their final rank.
        game.request_promotions(0, &mut Vec::new());
        Ok(game)
    }
    pub fn classical(registry: Registry) -> Result<Self, String> {
        let mut placements = Vec::new();
        let back = [
            "rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook",
        ];
        for (owner, rank, pawns) in [(0, 0, 1), (1, 7, 6)] {
            for (x, id) in back.iter().enumerate() {
                placements.push(Placement::new(id, owner, Pos::new(x as i32, rank)));
                placements.push(Placement::new("pawn", owner, Pos::new(x as i32, pawns)));
            }
        }
        let has_cards = registry.cards().next().is_some();
        let game = Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            [Player::new(0, Direction::N), Player::new(1, Direction::S)]
                .into_iter()
                .map(|mut p| {
                    p.mana = 1.into();
                    p.mana_per_turn = 1;
                    p
                })
                .collect(),
            placements,
        )?;
        if has_cards {
            let deck: crate::cards::DeckDef =
                ron::from_str(include_str!("../../assets/decks/starter.ron"))
                    .map_err(|e| e.to_string())?;
            game.with_decks(vec![(0, deck.clone()), (1, deck)], 42, 3, 1, true)
        } else {
            Ok(game)
        }
    }
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    /// An explicit setup for exercising the implemented custom abilities. Cards
    /// are seeded as real on-board instances; the cards demo also supplies decks.
    pub fn abilities_demo(registry: Registry) -> Result<Self, String> {
        let mut placements = Vec::new();
        for (id, x, y) in [
            ("king", 4, 0),
            ("queen", 3, 0),
            ("bishop", 2, 0),
            ("knight", 1, 0),
            ("magician", 2, 2),
            ("succubus", 5, 2),
            ("paladin", 5, 0),
            ("super_pawn", 3, 2),
            ("pawn", 1, 6),
            ("pawn", 3, 3),
        ] {
            placements.push(Placement::new(id, 0, Pos::new(x, y)));
            placements.push(Placement::new(id, 1, Pos::new(7 - x, 7 - y)));
        }
        let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
            .into_iter()
            .map(|mut p| {
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p.board_cards = ["fire", "ice", "invulnerability", "attack_demonic", "revive"]
                    .into_iter()
                    .map(String::from)
                    .collect();
                p
            })
            .collect();
        Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            players,
            placements,
        )
    }
    pub fn bounds(&self) -> Bounds {
        self.state.bounds
    }
    pub fn events_demo(registry: Registry) -> Result<Self, String> {
        let mut placements = Vec::new();
        for (id, x, y) in [
            ("king", 0, 0),
            ("mandragora", 1, 1),
            ("tesla_tower", 3, 1),
            ("spider", 4, 1),
            ("witch", 6, 1),
            ("archer", 7, 1),
            ("spider_egg", 0, 2),
            ("rook", 2, 2),
            ("pawn", 0, 3),
        ] {
            placements.push(Placement::new(id, 0, Pos::new(x, y)));
            placements.push(Placement::new(id, 1, Pos::new(7 - x, 7 - y)));
        }
        let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
            .into_iter()
            .map(|mut p| {
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p
            })
            .collect();
        Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            players,
            placements,
        )
    }
    pub fn turn(&self) -> &TurnState {
        &self.state.turn
    }
    pub fn turn_phase(&self) -> TurnPhase {
        self.state.turn_phase
    }
    pub fn siege_demo(registry: Registry) -> Result<Self, String> {
        let mut placements = Vec::new();
        for (id, x, y) in [
            ("king", 0, 0),
            ("builder", 1, 1),
            ("shield_bearer", 2, 0),
            ("wall", 3, 0),
            ("ballista", 4, 1),
            ("ram", 6, 1),
            ("pawn", 4, 3),
            ("wall", 6, 3),
        ] {
            placements.push(Placement::new(id, 0, Pos::new(x, y)));
            placements.push(Placement::new(id, 1, Pos::new(7 - x, 7 - y)));
        }
        let players = [Player::new(0, Direction::N), Player::new(1, Direction::S)]
            .into_iter()
            .map(|mut p| {
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p
            })
            .collect();
        Self::new(
            registry,
            Bounds {
                width: 8,
                height: 8,
            },
            players,
            placements,
        )
    }
    pub fn winner(&self) -> Option<PlayerId> {
        self.state.winner
    }
    pub fn piece(&self, id: EntityId) -> Option<&PieceInstance> {
        self.state.pieces.get(&id)
    }
    pub fn pieces(&self) -> impl Iterator<Item = &PieceInstance> {
        self.state.pieces.values()
    }
    pub fn at(&self, pos: Pos) -> Option<&PieceInstance> {
        if !self.bounds().contains(pos) {
            return None;
        }
        self.occupancy[self.square_index(pos)].and_then(|id| self.piece(id))
    }
    fn square_index(&self, pos: Pos) -> usize {
        debug_assert!(self.bounds().contains(pos));
        pos.y as usize * usize::from(self.bounds().width) + pos.x as usize
    }
    pub(crate) fn rebuild_occupancy(&mut self) {
        self.occupancy =
            vec![None; usize::from(self.bounds().width) * usize::from(self.bounds().height)];
        for piece in self.state.pieces.values() {
            let index = self.square_index(piece.position);
            debug_assert!(self.occupancy[index].is_none());
            self.occupancy[index] = Some(piece.id);
        }
    }
    pub(crate) fn insert_piece(&mut self, piece: PieceInstance) {
        let index = self.square_index(piece.position);
        debug_assert!(self.occupancy[index].is_none());
        self.occupancy[index] = Some(piece.id);
        self.state.pieces.insert(piece.id, piece);
    }
    pub(crate) fn detach_piece(&mut self, id: EntityId) -> Option<PieceInstance> {
        let piece = self.state.pieces.remove(&id)?;
        let index = self.square_index(piece.position);
        debug_assert_eq!(self.occupancy[index], Some(id));
        self.occupancy[index] = None;
        Some(piece)
    }
    pub(crate) fn relocate_piece(&mut self, id: EntityId, to: Pos) -> Pos {
        let from = self.piece(id).unwrap().position;
        let old = self.square_index(from);
        let new = self.square_index(to);
        debug_assert_eq!(self.occupancy[old], Some(id));
        debug_assert!(self.occupancy[new].is_none());
        self.occupancy[old] = None;
        self.occupancy[new] = Some(id);
        self.state.pieces.get_mut(&id).unwrap().position = to;
        from
    }
    #[cfg(debug_assertions)]
    pub(crate) fn check_occupancy(&self) {
        debug_assert_eq!(
            self.occupancy.iter().flatten().count(),
            self.state.pieces.len()
        );
        for p in self.pieces() {
            debug_assert_eq!(self.occupancy[self.square_index(p.position)], Some(p.id));
        }
    }
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        self.state.players.iter().find(|p| p.id == id)
    }
    pub fn dead(&self) -> &[DeathRecord] {
        &self.state.dead
    }
    pub fn log(&self) -> &[ResolvedAction] {
        &self.state.log
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.clone()
    }
    pub fn pending_promotion(&self) -> Option<&PendingPromotion> {
        self.state.pending_promotion.as_ref()
    }
    pub fn has_status(&self, id: EntityId, kind: StatusKind) -> bool {
        self.piece(id)
            .is_some_and(|p| p.statuses.contains_key(&kind))
    }
    pub fn has_tag(&self, id: EntityId, tag: Tag) -> bool {
        self.piece(id)
            .is_some_and(|p| self.effective_tags(p).contains(&tag))
    }
    pub fn scheduled(&self) -> &[crate::events::ScheduledWork] {
        &self.state.scheduled
    }
    pub fn owner_turn(&self, owner: PlayerId) -> Option<u64> {
        self.state.owner_turns.get(&owner).copied()
    }
    pub fn setup_events(&self) -> &[crate::commands::Event] {
        &self.state.setup_events
    }
}
