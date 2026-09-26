use crate::{
    commands::{ActionIntent, Event, ResolvedAction},
    content::{ActionKind, CompiledRule, Condition, Tag},
    geometry::Pos,
    state::{EntityId, Game, PieceInstance, PlayerId, StatusKind},
    targeting,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    GameOver,
    SourceMissing(EntityId),
    WrongPlayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    NotOwner,
    AlreadyActed,
    UnsupportedAction,
    OutOfBounds(Pos),
    SelfTarget,
    InvalidTarget,
    ProtectedTarget,
    InsufficientActions {
        required: u32,
        available: u32,
    },
    InsufficientMana {
        required: u32,
        available: u32,
    },
    ConditionFailed(Condition),
    CounterOverflow,
    ResolutionLimit,
    ScheduleLimit,
    Deactivated(StatusKind),
    UnknownAbility(String),
    CooldownActive {
        ability: String,
        remaining: u32,
    },
    AbilityAlreadyUsed(String),
    MissingBoardCard(String),
    PromotionPending,
    InvalidPromotion,
    InvalidCard,
    InvalidScheduledTarget,
    DuplicateBoardCard(String),
}
impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GameOver => write!(f, "the game has ended"),
            Self::SourceMissing(id) => write!(f, "piece {id} does not exist"),
            Self::WrongPlayer { expected, actual } => {
                write!(f, "player {expected} has the turn, not player {actual}")
            }
            Self::NotOwner => write!(f, "the source belongs to the other player"),
            Self::AlreadyActed => write!(f, "this piece has already acted this turn"),
            Self::UnsupportedAction => write!(f, "this piece has no rule for that action"),
            Self::OutOfBounds(p) => write!(f, "target {p:?} is outside the board"),
            Self::SelfTarget => write!(f, "Move, Take and Attack cannot target their source"),
            Self::InvalidTarget => write!(f, "target fails the action pattern or filters"),
            Self::ProtectedTarget => write!(f, "the target rejects this action"),
            Self::InsufficientActions {
                required,
                available,
            } => write!(f, "requires {required} actions; {available} remain"),
            Self::InsufficientMana {
                required,
                available,
            } => write!(f, "requires {required} mana; {available} available"),
            Self::ConditionFailed(c) => write!(f, "condition failed: {c:?}"),
            Self::CounterOverflow => write!(f, "match counter limit reached"),
            Self::ResolutionLimit => {
                write!(f, "effect resolution limit reached; action rolled back")
            }
            Self::ScheduleLimit => write!(f, "scheduled work limit reached; action rolled back"),
            Self::Deactivated(kind) => write!(f, "this piece cannot act while {kind:?} is active"),
            Self::UnknownAbility(id) => write!(f, "unknown ability: {id}"),
            Self::CooldownActive { ability, remaining } => write!(
                f,
                "{ability} is on cooldown for {remaining} more owner turn(s)"
            ),
            Self::AbilityAlreadyUsed(id) => write!(f, "{id} has already been used"),
            Self::MissingBoardCard(id) => write!(f, "requires on-board card {id}"),
            Self::PromotionPending => write!(f, "choose the pending pawn promotion first"),
            Self::InvalidPromotion => write!(f, "invalid promotion choice, player or token"),
            Self::InvalidCard => write!(
                f,
                "card is not in the required zone or belongs to another player"
            ),
            Self::InvalidScheduledTarget => {
                write!(f, "select an existing friendly scheduled event")
            }
            Self::DuplicateBoardCard(id) => write!(f, "{id} is already on board for this player"),
        }
    }
}
impl std::error::Error for RuleError {}

impl Game {
    pub(crate) fn check_player(&self, player: PlayerId) -> Result<(), RuleError> {
        if self.is_over() {
            return Err(RuleError::GameOver);
        }
        if self.turn().player != player {
            return Err(RuleError::WrongPlayer {
                expected: self.turn().player,
                actual: player,
            });
        }
        Ok(())
    }
    pub(crate) fn condition(&self, source: EntityId, c: &Condition) -> bool {
        let p = self.piece(source).expect("validated source");
        match c {
            Condition::NoPendingWork => !self
                .scheduled()
                .iter()
                .any(|job| job.cause.source.piece() == Some(source)),
            Condition::HasNotActed => !p.state.acted,
            Condition::ManaAtLeast(n) => self.player(p.owner).unwrap().mana >= *n,
            Condition::HasTag(t) => self.has_tag(source, *t),
            Condition::CardOnBoard(id) => self.has_board_card(p.owner, id),
            Condition::And(v) => v.iter().all(|c| self.condition(source, c)),
            Condition::Or(v) => v.iter().any(|c| self.condition(source, c)),
            Condition::Not(c) => !self.condition(source, c),
        }
    }
    fn piece_eligibility(
        &self,
        player: PlayerId,
        source: EntityId,
        kind: ActionKind,
    ) -> Result<&CompiledRule, RuleError> {
        let piece = self.piece(source).ok_or(RuleError::SourceMissing(source))?;
        self.check_player(player)?;
        if piece.owner != player {
            return Err(RuleError::NotOwner);
        }
        let rule = self
            .registry
            .get(&piece.archetype)
            .unwrap()
            .rules
            .iter()
            .find(|r| r.kind == kind)
            .ok_or(RuleError::UnsupportedAction)?;
        if piece.state.acted {
            return Err(RuleError::AlreadyActed);
        }
        self.check_active(source, kind)?;
        if self.turn().remaining_actions < rule.cost.actions {
            return Err(RuleError::InsufficientActions {
                required: rule.cost.actions,
                available: self.turn().remaining_actions,
            });
        }
        let mana = &self.player(player).unwrap().mana;
        if *mana < rule.cost.mana {
            return Err(RuleError::InsufficientMana {
                required: rule.cost.mana,
                available: mana.as_u32().unwrap(),
            });
        }
        for c in &rule.conditions {
            if !self.condition(source, c) {
                return Err(RuleError::ConditionFailed(c.clone()));
            }
        }
        Ok(rule)
    }
    fn validate_piece(
        &self,
        player: PlayerId,
        source: EntityId,
        kind: ActionKind,
        target: Pos,
    ) -> Result<&CompiledRule, RuleError> {
        self.validate_piece_targets(player, source, kind, target, None)
    }
    fn validate_piece_targets(
        &self,
        player: PlayerId,
        source: EntityId,
        kind: ActionKind,
        target: Pos,
        targets: Option<&[Pos]>,
    ) -> Result<&CompiledRule, RuleError> {
        let rule = self.piece_eligibility(player, source, kind)?;
        let piece = self.piece(source).unwrap();
        if !self.is_playable(target) {
            return Err(RuleError::OutOfBounds(target));
        }
        if target == piece.position {
            return Err(RuleError::SelfTarget);
        }
        if !targets.map_or_else(
            || targeting::resolve(self, source, player, &rule.target).contains(&target),
            |targets| targets.contains(&target),
        ) || self.random_direction(source).is_some_and(|d| {
            d != (
                (target.x - piece.position.x).signum(),
                (target.y - piece.position.y).signum(),
            )
        }) {
            return Err(RuleError::InvalidTarget);
        }
        if self
            .at(target)
            .is_some_and(|p| !self.can_receive(source, p.id, kind))
        {
            return Err(RuleError::ProtectedTarget);
        }
        Ok(rule)
    }
    pub fn validate(&self, intent: &ActionIntent) -> Result<(), RuleError> {
        if self.pending_promotion().is_some()
            && !matches!(intent, ActionIntent::ChoosePromotion { .. })
        {
            return Err(RuleError::PromotionPending);
        }
        match intent {
            ActionIntent::ScheduledAbility {
                player,
                source,
                ability,
                scheduled,
            } => {
                self.validate_scheduled_ability(*player, *source, ability, *scheduled)?;
            }
            ActionIntent::Transport {
                player,
                source,
                ability,
                target,
                destination,
            } => {
                self.validate_transport(*player, *source, ability, *target, *destination)?;
            }
            ActionIntent::PlayCard {
                player,
                card,
                target,
            } => {
                self.validate_card(*player, *card, target)?;
            }
            ActionIntent::Piece {
                player,
                source,
                kind,
                target,
            } => {
                self.validate_piece(*player, *source, *kind, *target)?;
            }
            ActionIntent::EndTurn { player } => self.check_player(*player)?,
            ActionIntent::Ability {
                player,
                source,
                ability,
                target,
            } => {
                let (definition, _) = self.validate_ability(*player, *source, ability, *target)?;
                if matches!(
                    definition.definition.selection,
                    crate::content::Selection::Transport
                        | crate::content::Selection::ScheduledSummon
                ) {
                    return Err(RuleError::InvalidTarget);
                }
            }
            ActionIntent::ChoosePromotion {
                player,
                token,
                archetype,
            } => {
                self.validate_promotion(*player, *token, archetype)?;
            }
        }
        Ok(())
    }
    /// Highlights and submitted actions use exactly the same validator.
    pub fn legal_actions(&self, source: EntityId) -> Vec<ActionIntent> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        let mut actions = Vec::new();
        if let Some(pending) = self.pending_promotion() {
            if pending.piece == source {
                return pending
                    .choices
                    .iter()
                    .map(|archetype| ActionIntent::ChoosePromotion {
                        player: pending.player,
                        token: pending.token,
                        archetype: archetype.clone(),
                    })
                    .collect();
            }
            return actions;
        }
        for rule in &self.registry.get(&piece.archetype).unwrap().rules {
            if self
                .piece_eligibility(self.turn().player, source, rule.kind)
                .is_err()
            {
                continue;
            }
            let targets = targeting::resolve(self, source, piece.owner, &rule.target);
            let direction = self.random_direction(source);
            for target in targets.iter().copied() {
                let intent = ActionIntent::Piece {
                    player: self.turn().player,
                    source,
                    kind: rule.kind,
                    target,
                };
                if self.generated_piece_target_is_legal(piece, rule.kind, target, direction) {
                    actions.push(intent);
                }
            }
        }
        actions.extend(self.legal_abilities(source));
        actions
    }
    /// Enumerate a single ordinary action class without computing ability choices.
    pub fn legal_piece_actions(&self, source: EntityId, kind: ActionKind) -> Vec<ActionIntent> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        if self.pending_promotion().is_some() || self.is_over() {
            return vec![];
        }
        let mut actions = Vec::new();
        for rule in self
            .registry
            .get(&piece.archetype)
            .unwrap()
            .rules
            .iter()
            .filter(|r| r.kind == kind)
        {
            if self
                .piece_eligibility(self.turn().player, source, rule.kind)
                .is_err()
            {
                continue;
            }
            let targets = targeting::resolve(self, source, piece.owner, &rule.target);
            let direction = self.random_direction(source);
            for target in targets.iter().copied() {
                let intent = ActionIntent::Piece {
                    player: self.turn().player,
                    source,
                    kind,
                    target,
                };
                if self.generated_piece_target_is_legal(piece, kind, target, direction) {
                    actions.push(intent);
                }
            }
        }
        actions
    }
    // Only called after eligibility and target resolution. Those checks are
    // invariant across a rule's targets; direction selection is also pure and
    // shared. Registry compilation rejects duplicate ordinary action kinds.
    fn generated_piece_target_is_legal(
        &self,
        piece: &PieceInstance,
        kind: ActionKind,
        target: Pos,
        direction: Option<(i32, i32)>,
    ) -> bool {
        target != piece.position
            && direction.is_none_or(|d| {
                d == (
                    (target.x - piece.position.x).signum(),
                    (target.y - piece.position.y).signum(),
                )
            })
            && self
                .at(target)
                .is_none_or(|p| self.can_receive(piece.id, p.id, kind))
    }
    /// Validate without writes, then commit a fully resolved snapshot atomically.
    /// All mutations and emitted events go through the effects layer.
    pub fn apply(&mut self, intent: ActionIntent) -> Result<ResolvedAction, RuleError> {
        self.validate(&intent)?;
        let id = self
            .action_count()
            .checked_add(1)
            .ok_or(RuleError::CounterOverflow)?;
        if self.turn().number == u64::MAX {
            return Err(RuleError::CounterOverflow);
        }
        let mut next = Self {
            simulation_actions: self.simulation_actions,
            registry: self.registry.clone(),
            state: self.state.clone_for_resolution(),
            occupancy: self.occupancy.clone(),
            resolver: Default::default(),
        };
        let mut events = Vec::new();
        if !matches!(
            intent,
            ActionIntent::ChoosePromotion { .. } | ActionIntent::EndTurn { .. }
        ) {
            next.tick_clock(crate::clock::ClockUnit::Movements, id, &mut events)?;
            next.request_promotions(id, &mut events);
            // Start-of-movement effects may change eligibility. Never execute a stale action.
            if let Err(reason) = next.validate(&intent) {
                events.push(Event::EffectSkipped {
                    cause: crate::cards::system_cause(next.turn().player, id),
                    reason: format!("action interrupted at movement start: {reason}"),
                });
                next.request_promotions(id, &mut events);
                next.finish_action(id, &mut events)?;
                let resolved = ResolvedAction { id, intent, events };
                self.commit_resolution(next, &resolved);
                return Ok(resolved);
            }
        }
        match &intent {
            ActionIntent::ScheduledAbility {
                player,
                source,
                ability,
                scheduled,
            } => {
                let (definition, targets) =
                    next.validate_scheduled_ability(*player, *source, ability, *scheduled)?;
                let definition = definition.clone();
                next.execute_ability(
                    id,
                    *source,
                    &definition,
                    (targets, None, Some(*scheduled)),
                    &mut events,
                )?;
            }
            ActionIntent::Transport {
                player,
                source,
                ability,
                target,
                destination,
            } => {
                let (definition, targets) =
                    next.validate_ability(*player, *source, ability, Some(*target))?;
                let definition = definition.clone();
                next.execute_ability(
                    id,
                    *source,
                    &definition,
                    (targets, Some(*destination), None),
                    &mut events,
                )?;
            }
            ActionIntent::PlayCard {
                player,
                card,
                target,
            } => {
                next.execute_card(id, *player, *card, target, &mut events)?;
            }
            ActionIntent::Piece {
                player,
                source,
                kind,
                target,
            } => {
                let rule = next
                    .validate_piece(*player, *source, *kind, *target)?
                    .clone();
                if let Some(direction) = next.random_direction(*source) {
                    let choices = next.random_directions(*source);
                    let chosen = choices[next.state.rng.below(choices.len() as u64)];
                    debug_assert_eq!(direction, chosen);
                    events.push(Event::RandomDirectionChosen {
                        source: *source,
                        direction,
                    });
                }
                events.push(Event::ActionStarted {
                    source: Some(*source),
                    kind: Some(*kind),
                });
                next.execute_rule(
                    crate::effects::EffectContext {
                        action_id: id,
                        player: *player,
                        source: *source,
                        kind: *kind,
                        target: *target,
                    },
                    &rule,
                    &mut events,
                )?;
            }
            ActionIntent::EndTurn { player: _ } => {
                events.push(Event::ActionStarted {
                    source: None,
                    kind: None,
                });
                next.state.pass_requested = true;
                next.finish_action(id, &mut events)?;
            }
            ActionIntent::Ability {
                player,
                source,
                ability,
                target,
            } => {
                let (definition, targets) =
                    next.validate_ability(*player, *source, ability, *target)?;
                let definition = definition.clone();
                next.execute_ability(id, *source, &definition, (targets, None, None), &mut events)?;
            }
            ActionIntent::ChoosePromotion {
                player: _,
                token: _,
                archetype,
            } => {
                next.promote(id, archetype, &mut events)?;
            }
        }
        let resolved = ResolvedAction { id, intent, events };
        self.commit_resolution(next, &resolved);
        Ok(resolved)
    }
    fn commit_resolution(&mut self, mut next: Self, resolved: &ResolvedAction) {
        debug_assert!(next.state.log.is_empty());
        if self.simulation_actions.is_some() {
            self.simulation_actions = Some(resolved.id);
        } else {
            next.state.log = std::mem::take(&mut self.state.log);
            next.state.log.push(resolved.clone());
        }
        self.state = next.state;
        self.occupancy = next.occupancy;
        #[cfg(debug_assertions)]
        self.check_occupancy();
    }
    /// Resolve on an isolated copy, including triggered effects and turn transfer.
    /// The returned trace is exactly what applying the same intent now would emit.
    pub fn preview(&self, intent: &ActionIntent) -> Result<ResolvedAction, RuleError> {
        self.simulation().apply(intent.clone())
    }
    pub(crate) fn check_active(&self, source: EntityId, kind: ActionKind) -> Result<(), RuleError> {
        for status in [StatusKind::Deactivate, StatusKind::Ice] {
            if self.has_status(source, status)
                && !matches!(
                    (status, kind),
                    (StatusKind::Ice, ActionKind::Attack)
                        | (StatusKind::Deactivate, ActionKind::Move)
                )
            {
                return Err(RuleError::Deactivated(status));
            }
        }
        Ok(())
    }
    pub(crate) fn can_receive(&self, source: EntityId, target: EntityId, kind: ActionKind) -> bool {
        // Immunity has no self, ally or global-effect exception.
        if matches!(kind, ActionKind::Ability | ActionKind::PlayCard)
            && self.has_tag(target, Tag::Immune)
        {
            return false;
        }
        if source == target {
            return true;
        }
        !self.has_status(target, StatusKind::Invulnerable)
            && !(kind == ActionKind::Attack && self.has_tag(target, Tag::Heroic))
            && !(matches!(kind, ActionKind::Ability | ActionKind::PlayCard)
                && self.has_tag(target, Tag::Immune))
    }
}
