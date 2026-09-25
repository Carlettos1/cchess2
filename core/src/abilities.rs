use crate::{
    commands::{ActionIntent, Event},
    content::{ActionKind, CompiledAbility, EffectDef, Selection},
    effects::ResolutionContext,
    geometry::Pos,
    rules::RuleError,
    state::{Cooldown, EffectSource, EntityId, Game, PendingPromotion, PlayerId},
    targeting,
};

impl Game {
    /// Validated selection squares. Direction selections indicate a ray/facing,
    /// not an affected occupant; use `preview` for the resolved effect trace.
    pub fn ability_targets(&self, intent: &ActionIntent) -> Result<Vec<Pos>, RuleError> {
        self.validate(intent)?;
        if let ActionIntent::ScheduledAbility {
            player,
            source,
            ability,
            scheduled,
        } = intent
        {
            return Ok(self
                .validate_scheduled_ability(*player, *source, ability, *scheduled)?
                .1);
        }
        if let ActionIntent::Transport {
            player,
            source,
            ability,
            target,
            destination,
        } = intent
        {
            self.validate_transport(*player, *source, ability, *target, *destination)?;
            return Ok(vec![*target]);
        }
        if let ActionIntent::Ability {
            player,
            source,
            ability,
            target,
        } = intent
        {
            Ok(self.validate_ability(*player, *source, ability, *target)?.1)
        } else {
            Err(RuleError::UnsupportedAction)
        }
    }
    pub(crate) fn validate_scheduled_ability(
        &self,
        player: PlayerId,
        source: EntityId,
        ability: &str,
        scheduled: u64,
    ) -> Result<(&CompiledAbility, Vec<Pos>), RuleError> {
        self.check_player(player)?;
        let job = self
            .scheduled()
            .iter()
            .find(|j| j.id == scheduled && j.owner == player)
            .ok_or(RuleError::InvalidScheduledTarget)?;
        if !matches!(job.effects.as_slice(), [EffectDef::SpawnTarget(_)]) {
            return Err(RuleError::InvalidScheduledTarget);
        }
        let origin = job
            .cause
            .source
            .piece()
            .and_then(|id| self.piece(id))
            .ok_or(RuleError::InvalidScheduledTarget)?;
        if origin.owner != player {
            return Err(RuleError::InvalidScheduledTarget);
        }
        let (definition, targets) =
            self.validate_ability(player, source, ability, Some(origin.position))?;
        if definition.definition.selection != Selection::ScheduledSummon {
            return Err(RuleError::UnsupportedAction);
        }
        Ok((definition, targets))
    }
    fn ability_eligibility(
        &self,
        player: PlayerId,
        source: EntityId,
        id: &str,
    ) -> Result<&CompiledAbility, RuleError> {
        let piece = self.piece(source).ok_or(RuleError::SourceMissing(source))?;
        self.check_player(player)?;
        if piece.owner != player {
            return Err(RuleError::NotOwner);
        }
        self.check_active(source, ActionKind::Ability)?;
        if piece.state.acted {
            return Err(RuleError::AlreadyActed);
        }
        let ability = self
            .registry
            .get(&piece.archetype)
            .unwrap()
            .abilities
            .iter()
            .find(|a| a.definition.id == id)
            .ok_or_else(|| RuleError::UnknownAbility(id.into()))?;
        let definition = &ability.definition;
        let cost = self.effective_ability_cost(source, definition.cost);
        if piece.state.used_abilities.contains(id) {
            return Err(RuleError::AbilityAlreadyUsed(id.into()));
        }
        if let Some(cd) = piece.state.cooldowns.get(id) {
            return Err(RuleError::CooldownActive {
                ability: id.into(),
                remaining: cd.remaining,
            });
        }
        if self.turn().remaining_actions < cost.actions {
            return Err(RuleError::InsufficientActions {
                required: cost.actions,
                available: self.turn().remaining_actions,
            });
        }
        let owner = self.player(player).unwrap();
        if owner.mana < cost.mana {
            return Err(RuleError::InsufficientMana {
                required: cost.mana,
                available: owner.mana.as_u32().unwrap(),
            });
        }
        for condition in &definition.conditions {
            if !self.condition(source, condition) {
                return Err(RuleError::ConditionFailed(condition.clone()));
            }
        }
        for card in &definition.consume_cards {
            if !self.has_board_card(player, card) {
                return Err(RuleError::MissingBoardCard(card.clone()));
            }
        }
        if definition.effects.iter().any(|e| {
            matches!(
                e,
                EffectDef::MoveSource
                    | EffectDef::RemoveSource
                    | EffectDef::TransformSource(_)
                    | EffectDef::SeekTag(_)
            )
        }) && !self.can_receive(source, source, ActionKind::Ability)
        {
            return Err(RuleError::ProtectedTarget);
        }
        Ok(ability)
    }
    pub(crate) fn validate_ability(
        &self,
        player: PlayerId,
        source: EntityId,
        id: &str,
        target: Option<Pos>,
    ) -> Result<(&CompiledAbility, Vec<Pos>), RuleError> {
        self.validate_ability_targets(player, source, id, target, None)
    }
    fn validate_ability_targets(
        &self,
        player: PlayerId,
        source: EntityId,
        id: &str,
        target: Option<Pos>,
        resolved: Option<&[Pos]>,
    ) -> Result<(&CompiledAbility, Vec<Pos>), RuleError> {
        let ability = self.ability_eligibility(player, source, id)?;
        let piece = self.piece(source).unwrap();
        let definition = &ability.definition;
        let computed;
        let targets = if let Some(targets) = resolved {
            targets
        } else {
            computed = targeting::resolve(self, source, player, &ability.target);
            &computed
        };
        let selected = match (definition.selection, target) {
            (
                Selection::One
                | Selection::Direction
                | Selection::Transport
                | Selection::ScheduledSummon,
                Some(p),
            ) => {
                if !self.is_playable(p) {
                    return Err(RuleError::OutOfBounds(p));
                }
                if !targets.contains(&p) {
                    return Err(RuleError::InvalidTarget);
                }
                if definition.selection != Selection::Direction
                    && self
                        .at(p)
                        .is_some_and(|p| !self.can_receive(source, p.id, ActionKind::Ability))
                {
                    return Err(RuleError::ProtectedTarget);
                }
                vec![p]
            }
            (Selection::All, None) => targets
                .iter()
                .copied()
                .filter(|p| {
                    self.at(*p)
                        .is_none_or(|p| self.can_receive(source, p.id, ActionKind::Ability))
                })
                .collect(),
            _ => return Err(RuleError::InvalidTarget),
        };
        let selected: Vec<_> = selected
            .into_iter()
            .filter(|p| {
                !matches!(definition.effects.as_slice(), [EffectDef::RemoveStatuses])
                    || self.at(*p).is_some_and(|p| !p.statuses.is_empty())
            })
            .collect();
        if definition.selection == Selection::Transport
            && selected.iter().any(|pos| {
                self.at(*pos).is_none_or(|p| {
                    p.stats
                        .transportable
                        .is_none_or(|weight| weight > piece.stats.strength)
                })
            })
        {
            return Err(RuleError::InvalidTarget);
        }
        if selected.is_empty() {
            return Err(RuleError::InvalidTarget);
        }
        if let [EffectDef::SeekTag(tag)] = definition.effects.as_slice() {
            if self.seek_path(source, selected[0], *tag)?.0 == piece.position {
                return Err(RuleError::InvalidTarget);
            }
        }
        if let [EffectDef::ReviveLatest { .. }] = definition.effects.as_slice() {
            if self.latest_revivable(player).is_none() {
                return Err(RuleError::InvalidTarget);
            }
            self.state
                .next_entity
                .checked_add(1)
                .ok_or(RuleError::CounterOverflow)?;
        }
        if let [EffectDef::Push { distance, .. }] = definition.effects.as_slice() {
            self.push_destination(source, selected[0], *distance)?;
        }
        if let [EffectDef::SpawnRelative { directions, .. }] = definition.effects.as_slice() {
            let count = self.spawn_positions(source, directions).len();
            if count == 0 {
                return Err(RuleError::InvalidTarget);
            }
            self.state
                .next_entity
                .checked_add(count as u64)
                .ok_or(RuleError::CounterOverflow)?;
        }
        if let [EffectDef::SpawnOriented { directions, .. }] = definition.effects.as_slice() {
            let count = self
                .oriented_spawn_positions(source, selected[0], directions)?
                .len();
            self.state
                .next_entity
                .checked_add(count as u64)
                .ok_or(RuleError::CounterOverflow)?;
        }
        Ok((ability, selected))
    }

    pub fn legal_abilities(&self, source: EntityId) -> Vec<ActionIntent> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        let mut result = Vec::new();
        if self.pending_promotion().is_some() {
            return result;
        }
        for ability in &self.registry.get(&piece.archetype).unwrap().abilities {
            if self
                .ability_eligibility(self.turn().player, source, &ability.definition.id)
                .is_err()
            {
                continue;
            }
            if ability.definition.selection == Selection::ScheduledSummon {
                for job in self.scheduled() {
                    let intent = ActionIntent::ScheduledAbility {
                        player: self.turn().player,
                        source,
                        ability: ability.definition.id.clone(),
                        scheduled: job.id,
                    };
                    if self.validate(&intent).is_ok() {
                        result.push(intent);
                    }
                }
                continue;
            }
            let resolved = targeting::resolve(self, source, piece.owner, &ability.target);
            let targets = match ability.definition.selection {
                Selection::One
                | Selection::Direction
                | Selection::Transport
                | Selection::ScheduledSummon => resolved.iter().copied().map(Some).collect(),
                Selection::All => vec![None],
            };
            for target in targets {
                if ability.definition.selection == Selection::Transport {
                    let [EffectDef::Transport { max_range, .. }] =
                        ability.definition.effects.as_slice()
                    else {
                        unreachable!("validated transport ability")
                    };
                    if self
                        .validate_ability_targets(
                            self.turn().player,
                            source,
                            &ability.definition.id,
                            target,
                            Some(&resolved),
                        )
                        .is_err()
                    {
                        continue;
                    }
                    let origin = target.unwrap();
                    let destinations = crate::geometry::SubDirection::ALL
                        .into_iter()
                        .flat_map(|d| {
                            let (dx, dy) = d.vector();
                            (1..=i32::from(*max_range))
                                .filter_map(move |n| origin.offset(dx * n, dy * n))
                        })
                        .filter(|p| self.is_playable(*p));
                    for destination in destinations {
                        let intent = ActionIntent::Transport {
                            player: self.turn().player,
                            source,
                            ability: ability.definition.id.clone(),
                            target: target.unwrap(),
                            destination,
                        };
                        if self
                            .transport_direction(origin, destination, *max_range)
                            .is_ok()
                        {
                            result.push(intent);
                        }
                    }
                    continue;
                }
                let intent = ActionIntent::Ability {
                    player: self.turn().player,
                    source,
                    ability: ability.definition.id.clone(),
                    target,
                };
                if self
                    .validate_ability_targets(
                        self.turn().player,
                        source,
                        &ability.definition.id,
                        target,
                        Some(&resolved),
                    )
                    .is_ok()
                {
                    result.push(intent);
                }
            }
        }
        result
    }

    pub(crate) fn execute_ability(
        &mut self,
        action_id: u64,
        source: EntityId,
        ability: &CompiledAbility,
        (targets, destination, scheduled): (Vec<Pos>, Option<Pos>, Option<u64>),
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let definition = &ability.definition;
        let player = self.piece(source).unwrap().owner;
        events.push(Event::ActionStarted {
            source: Some(source),
            kind: Some(ActionKind::Ability),
        });
        let cost = self.effective_ability_cost(source, definition.cost);
        for (card, amount) in self.ability_discounts(source, definition.cost.mana) {
            events.push(Event::AbilityDiscountApplied {
                source,
                card,
                amount,
            });
        }
        self.pay_cost(player, source, cost, events);
        let turn = self.turn().number;
        let state = &mut self.state.pieces.get_mut(&source).unwrap().state;
        if definition.once {
            state.used_abilities.insert(definition.id.clone());
        }
        if definition.cooldown > 0 {
            state.cooldowns.insert(
                definition.id.clone(),
                Cooldown {
                    remaining: definition.cooldown,
                    applied_turn: turn,
                },
            );
        }
        events.push(Event::AbilityUsed {
            source,
            ability: definition.id.clone(),
            cooldown: definition.cooldown,
            once: definition.once,
        });
        let context = ResolutionContext {
            action_id,
            kind: ActionKind::Ability,
            cause: EffectSource {
                source: crate::state::ActionSource::Piece(source),
                player,
                action_id,
                ability: Some(definition.id.clone()),
                status: None,
                trigger: None,
                scheduled: None,
            },
        };
        for card in &definition.consume_cards {
            self.consume_board_card(player, card, &context.cause, events)?;
            events.push(Event::BoardCardConsumed {
                player,
                card: card.clone(),
            });
        }
        self.execute_selected_effects(
            &context,
            &definition.effects,
            &targets,
            (destination, scheduled),
            events,
        )?;
        events.push(Event::AbilityResolved {
            source,
            ability: definition.id.clone(),
        });
        if !self.is_over() {
            self.request_promotions(action_id, events);
        }
        self.finish_action(action_id, events)
    }

    pub(crate) fn request_promotions(&mut self, token: u64, events: &mut Vec<Event>) {
        let pending = self
            .pieces()
            .filter(|p| p.owner == self.turn().player)
            .find_map(|piece| {
                let promotion = self
                    .registry
                    .get(&piece.archetype)?
                    .definition
                    .promotion
                    .as_ref()?;
                let (dx, dy) = self.player(piece.owner)?.forward.rotate((0, 1));
                let at_end = piece
                    .position
                    .offset(dx, dy)
                    .is_none_or(|p| !self.is_playable(p));
                at_end.then(|| PendingPromotion {
                    token,
                    player: piece.owner,
                    piece: piece.id,
                    choices: promotion.choices.clone(),
                })
            });
        if let Some(pending) = pending {
            events.push(Event::PromotionRequired {
                token,
                piece: pending.piece,
                choices: pending.choices.clone(),
            });
            self.state.pending_promotion = Some(pending);
        }
    }

    pub(crate) fn validate_promotion(
        &self,
        player: PlayerId,
        token: u64,
        archetype: &str,
    ) -> Result<(), RuleError> {
        self.check_player(player)?;
        let pending = self
            .pending_promotion()
            .ok_or(RuleError::InvalidPromotion)?;
        if pending.token != token
            || pending.player != player
            || !pending.choices.iter().any(|id| id == archetype)
        {
            return Err(RuleError::InvalidPromotion);
        }
        Ok(())
    }

    pub(crate) fn promote(
        &mut self,
        action_id: u64,
        archetype: &str,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let pending = self.state.pending_promotion.take().unwrap();
        let piece = self.state.pieces.get_mut(&pending.piece).unwrap();
        let from = std::mem::replace(&mut piece.archetype, archetype.into());
        piece.stats = self.registry.get(archetype).unwrap().definition.base_stats;
        // Promotion refreshes health and the new ability set, preserving identity,
        // position, ownership, statuses and acted state. It never emits a death.
        piece.state.damage = 0;
        piece.state.cooldowns.clear();
        piece.state.used_abilities.clear();
        events.push(Event::PieceTransformed {
            piece: piece.id,
            from,
            to: archetype.into(),
        });
        self.cleanse_immune(events);
        self.request_promotions(action_id, events);
        self.finish_action(action_id, events)
    }
}
