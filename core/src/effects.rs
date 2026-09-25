use crate::{
    commands::Event,
    content::{ActionCost, ActionKind, CompiledRule, EffectDef, Tag, TriggerEvent},
    events::Frame,
    geometry::{Pos, Relative},
    rules::RuleError,
    state::{DeathRecord, EffectSource, EntityId, Game, PieceInstance, PieceState, PlayerId},
};
use std::collections::BTreeSet;

pub(crate) struct EffectContext {
    pub action_id: u64,
    pub player: PlayerId,
    pub source: EntityId,
    pub kind: ActionKind,
    pub target: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolutionContext {
    pub action_id: u64,
    pub cause: EffectSource,
    pub kind: ActionKind,
}

impl Game {
    pub(crate) fn execute_rule(
        &mut self,
        context: EffectContext,
        rule: &CompiledRule,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let EffectContext {
            action_id,
            player,
            source,
            kind,
            target,
        } = context;
        self.pay_cost(player, source, rule.cost, events);
        let context = ResolutionContext {
            action_id,
            cause: EffectSource {
                source: crate::state::ActionSource::Piece(source),
                player,
                action_id,
                ability: None,
                status: None,
                trigger: None,
                scheduled: None,
            },
            kind,
        };
        self.execute_effects(&context, &rule.effects, &[target], events)?;
        events.push(Event::ActionResolved { source, kind });
        if !self.is_over() {
            self.request_promotions(action_id, events);
        }
        self.finish_action(action_id, events)
    }

    pub(crate) fn pay_cost(
        &mut self,
        player: PlayerId,
        source: EntityId,
        cost: ActionCost,
        events: &mut Vec<Event>,
    ) {
        self.state.turn.remaining_actions -= cost.actions;
        self.state
            .players
            .iter_mut()
            .find(|p| p.id == player)
            .unwrap()
            .mana -= cost.mana;
        self.state.pieces.get_mut(&source).unwrap().state.acted = true;
        events.push(Event::CostsPaid {
            player,
            actions: cost.actions,
            mana: cost.mana,
        });
    }

    pub(crate) fn apply_primitive(
        &mut self,
        effect: &EffectDef,
        frame: &Frame,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let context = &frame.context;
        let source = context.cause.source.piece().unwrap_or(0);
        let selected = frame.target.entity.filter(|id| self.piece(*id).is_some());
        if matches!(
            effect,
            EffectDef::RemoveSource | EffectDef::MoveSource | EffectDef::MoveSourceIfTargetRemoved
        ) && !self.can_receive(source, source, context.kind)
        {
            self.skip_effect(frame, "source protected", events);
            return Ok(());
        }
        match effect {
            EffectDef::RemoveSource => self.remove_piece(context, source, events)?,
            EffectDef::MoveSource => {
                if self.piece(source).is_some() && self.at(frame.target.position).is_none() {
                    self.move_entity(source, frame.target.position, events);
                } else {
                    self.skip_effect(
                        frame,
                        "movement source missing or destination occupied",
                        events,
                    );
                }
            }
            EffectDef::MoveSourceIfTargetRemoved => {
                if frame.target.entity.is_some()
                    && selected.is_none()
                    && self.piece(source).is_some()
                    && self.at(frame.target.position).is_none()
                {
                    self.move_entity(source, frame.target.position, events);
                }
            }
            EffectDef::DamageTarget(_)
            | EffectDef::ModifyStat { .. }
            | EffectDef::ResetAction
            | EffectDef::AddStatus { .. }
            | EffectDef::RemoveStatuses => {
                let Some(id) = selected else {
                    self.skip_effect(frame, "target removed", events);
                    return Ok(());
                };
                if !self.can_receive(source, id, context.kind) {
                    self.skip_effect(frame, "target protected", events);
                    return Ok(());
                }
                match effect {
                    EffectDef::ResetAction => {
                        self.state.pieces.get_mut(&id).unwrap().state.acted = false;
                        events.push(Event::ActionReset {
                            piece: id,
                            cause: context.cause.clone(),
                        });
                    }
                    EffectDef::ModifyStat { stat, amount } => {
                        let (before, after) = self
                            .state
                            .pieces
                            .get_mut(&id)
                            .unwrap()
                            .stats
                            .modify(*stat, *amount);
                        events.push(Event::StatModified {
                            piece: id,
                            stat: *stat,
                            before,
                            after,
                            cause: context.cause.clone(),
                        });
                    }
                    EffectDef::DamageTarget(amount) => {
                        self.damage_piece(context, id, *amount, events)?
                    }
                    EffectDef::AddStatus { kind, duration } => {
                        self.add_status(id, *kind, *duration, context.cause.clone(), events)
                    }
                    EffectDef::RemoveStatuses => {
                        let statuses =
                            std::mem::take(&mut self.state.pieces.get_mut(&id).unwrap().statuses);
                        for kind in statuses.keys() {
                            events.push(Event::StatusRemoved {
                                piece: id,
                                kind: *kind,
                                expired: false,
                            });
                        }
                    }
                    _ => unreachable!(),
                }
            }
            EffectDef::SpawnTarget(archetype) => {
                if self.is_playable(frame.target.position)
                    && self.at(frame.target.position).is_none()
                {
                    self.spawn_at(context, archetype, vec![frame.target.position], events)?;
                } else {
                    self.skip_effect(frame, "summon destination occupied or off board", events);
                }
            }
            EffectDef::SpawnRelative {
                archetype,
                directions,
            } => {
                if self.piece(source).is_none() {
                    self.skip_effect(frame, "spawn source removed", events);
                    return Ok(());
                }
                let positions = self.spawn_positions(source, directions);
                if positions.is_empty() {
                    self.skip_effect(frame, "no empty spawn square", events);
                }
                self.spawn_at(context, archetype, positions, events)?;
            }
            EffectDef::SpawnOriented {
                archetype,
                directions,
            } => {
                let positions =
                    self.oriented_spawn_positions(source, frame.target.position, directions)?;
                self.spawn_at(context, archetype, positions, events)?;
            }
            EffectDef::Traverse(def) => self.traverse(frame, def, events)?,
            EffectDef::AdvancePeers { halfside } => self.advance_peers(frame, *halfside, events)?,
            EffectDef::SeekTag(tag) => self.seek_tag(frame, *tag, events)?,
            EffectDef::ReviveLatest { undead } => self.revive_latest(frame, *undead, events)?,
            EffectDef::Push {
                distance,
                collision_damage,
            } => self.push_piece(frame, *distance, *collision_damage, events)?,
            EffectDef::Transport {
                max_range,
                collision_damage,
            } => self.transport(frame, *max_range, *collision_damage, events)?,
            _ => unreachable!("control flow is handled by the queue"),
        }
        Ok(())
    }

    fn spawn_at(
        &mut self,
        context: &ResolutionContext,
        archetype: &str,
        positions: Vec<Pos>,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        for position in positions {
            let id = self.state.next_entity;
            self.state.next_entity = id.checked_add(1).ok_or(RuleError::CounterOverflow)?;
            let piece = PieceInstance {
                added_tags: Default::default(),
                suppressed_tags: Default::default(),
                revival: None,
                id,
                archetype: archetype.into(),
                owner: context.cause.player,
                position,
                stats: self.registry.get(archetype).unwrap().definition.base_stats,
                state: PieceState {
                    acted: true,
                    ..Default::default()
                },
                statuses: Default::default(),
            };
            self.insert_piece(piece.clone());
            events.push(Event::PieceSpawned {
                piece: id,
                archetype: archetype.into(),
                owner: piece.owner,
                position,
            });
            self.enqueue_lifecycle(&piece, TriggerEvent::Spawned, context, events)?;
        }
        Ok(())
    }

    pub(crate) fn spawn_positions(&self, source: EntityId, directions: &[Relative]) -> Vec<Pos> {
        let piece = self.piece(source).unwrap();
        let forward = self.player(piece.owner).unwrap().forward;
        directions
            .iter()
            .filter_map(|d| {
                let (x, y) = d.absolute(forward);
                piece.position.offset(x, y)
            })
            .filter(|p| self.is_playable(*p) && self.at(*p).is_none())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn damage_piece(
        &mut self,
        context: &ResolutionContext,
        id: EntityId,
        amount: u32,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if matches!(context.kind, ActionKind::Ability | ActionKind::PlayCard)
            && self.has_tag(id, Tag::Immune)
        {
            return Ok(());
        }
        let Some(piece) = self.state.pieces.get_mut(&id) else {
            return Ok(());
        };
        piece.state.damage = piece.state.damage.saturating_add(amount);
        let total = piece.state.damage;
        let removed = total >= piece.stats.tough;
        events.push(Event::DamageDealt {
            source: context.cause.source,
            target: id,
            kind: context.kind,
            amount,
            total,
            cause: context.cause.clone(),
        });
        if removed {
            self.remove_piece(context, id, events)?;
        }
        Ok(())
    }

    pub(crate) fn remove_piece(
        &mut self,
        context: &ResolutionContext,
        id: EntityId,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if self.piece(id).is_none() {
            return Ok(());
        }
        let royal = self.has_tag(id, Tag::Royal);
        let dead = self.detach_piece(id).unwrap();
        let owner = dead.owner;
        self.state.dead.push(DeathRecord {
            revived_as: None,
            piece: dead.clone(),
            action_id: context.action_id,
            source: context.cause.source,
            cause: context.cause.clone(),
        });
        events.push(Event::PieceRemoved {
            piece: id,
            source: context.cause.source,
        });
        self.enqueue_lifecycle(&dead, TriggerEvent::Removed, context, events)?;
        self.fire_removed_card_triggers(&dead, context, events)?;
        if royal
            && !self
                .pieces()
                .any(|p| p.owner == owner && self.has_tag(p.id, Tag::Royal))
        {
            if self.state.players.len() == 4 {
                self.eliminate_player(owner, context, events)?;
                return Ok(());
            }
            // Self damage or damage from a dead caster still awards the opponent.
            self.state.winner = self
                .state
                .players
                .iter()
                .find(|p| p.id != owner)
                .map(|p| p.id);
        }
        Ok(())
    }

    pub(crate) fn move_entity(&mut self, id: EntityId, to: Pos, events: &mut Vec<Event>) {
        debug_assert!(self.at(to).is_none());
        let from = self.relocate_piece(id, to);
        events.push(Event::PieceMoved {
            piece: id,
            from,
            to,
        });
    }

    pub(crate) fn finish_action(
        &mut self,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if self.pending_promotion().is_some() && !self.is_over() {
            return Ok(());
        }
        if self.emit_outcome(events) {
            self.state.pass_requested = false;
        } else if self.pending_promotion().is_none()
            && (self.state.pass_requested
                || self.is_eliminated(self.turn().player)
                || self.turn().remaining_actions == 0
                || self.state.turn_phase == crate::state::TurnPhase::Transfer)
        {
            self.state.pass_requested = false;
            self.advance_turn(action_id, events)?;
        }
        Ok(())
    }

    pub(crate) fn advance_turn(
        &mut self,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let old = self.turn().player;
        let index = self.state.players.iter().position(|p| p.id == old).unwrap();

        events.push(Event::TurnEnded { player: old });
        self.state.turn_phase = crate::state::TurnPhase::Action;
        let next = self.next_active_index(index);
        let player = &mut self.state.players[next];
        let mut gained = 0;
        let ordinal = self.state.owner_turns[&player.id]
            .checked_add(1)
            .ok_or(RuleError::CounterOverflow)?;
        if ordinal > 1 && (ordinal - 1) % u64::from(player.mana_interval.max(1)) == 0 {
            gained = player.mana_per_turn;
            player.mana += gained;
        }
        self.state.turn.player = player.id;
        self.state.turn.number = self
            .state
            .turn
            .number
            .checked_add(1)
            .ok_or(RuleError::CounterOverflow)?;
        self.state.turn.remaining_actions = player.actions_per_turn;
        for piece in self
            .state
            .pieces
            .values_mut()
            .filter(|p| p.owner == self.state.turn.player)
        {
            piece.state.acted = false;
        }
        let owner = self.turn().player;
        self.state.owner_turns.insert(owner, ordinal);
        events.push(Event::TurnStarted {
            player: owner,
            turn: self.turn().number,
        });
        // Record both boundaries before resolving effects that may eliminate a seat.
        self.advance_clock_counter(crate::clock::ClockUnit::Turns, events)?;
        let wrapped = next <= index;
        if wrapped {
            self.advance_clock_counter(crate::clock::ClockUnit::Rounds, events)?;
        }
        self.tick_owner_timers(action_id, owner, events)?;
        self.resolve_clock_tick(crate::clock::ClockUnit::Turns, action_id, events)?;
        if wrapped {
            self.resolve_clock_tick(crate::clock::ClockUnit::Rounds, action_id, events)?;
        }
        if self.emit_outcome(events) {
            return Ok(());
        }
        if self.is_eliminated(owner) {
            return self.advance_turn(action_id, events);
        }
        if gained > 0 {
            events.push(Event::ManaGained {
                player: self.turn().player,
                amount: gained,
            });
        }
        let cause = crate::cards::system_cause(owner, action_id);
        self.draw_cards(owner, self.state.deck_rules.draw_per_turn, &cause, events)?;
        self.resolve_due_on(crate::clock::ClockUnit::OwnerTurns, action_id, events)?;
        if self.emit_outcome(events) {
            return Ok(());
        }
        if self.is_eliminated(owner) {
            return self.advance_turn(action_id, events);
        }
        self.fire_clock_triggers(crate::clock::ClockUnit::OwnerTurns, action_id, events)?;
        if self.emit_outcome(events) {
            return Ok(());
        }
        if self.is_eliminated(owner) {
            return self.advance_turn(action_id, events);
        }
        self.request_promotions(action_id, events);
        Ok(())
    }
}
