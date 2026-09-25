//! Bounded deterministic effect resolution, piece lifecycle triggers and owner-turn scheduling.
use crate::{
    commands::Event,
    content::{ActionKind, EffectDef, TriggerEvent},
    effects::ResolutionContext,
    geometry::Pos,
    rules::RuleError,
    state::{EffectSource, EntityId, Game, PieceInstance, PlayerId},
    targeting::{self, TargetSpec},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const MAX_RESOLUTION_ITEMS: usize = 1024;
pub const MAX_SCHEDULED_WORK: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundTarget {
    #[serde(default)]
    pub destination: Option<Pos>,
    pub entity: Option<EntityId>,
    pub position: Pos,
    #[serde(default)]
    pub scheduled: Option<u64>,
    #[serde(default)]
    pub card: Option<crate::state::CardId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    pub context: ResolutionContext,
    pub target: BoundTarget,
    pub source_archetype: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledWork {
    pub id: u64,
    pub owner: PlayerId,
    pub due_tick: u64,
    pub clock: crate::clock::ClockUnit,
    pub kind: ActionKind,
    pub cause: EffectSource,
    pub target: BoundTarget,
    pub source_archetype: Option<String>,
    pub effects: Vec<EffectDef>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkItem {
    effect: Option<EffectDef>,
    frame: Frame,
    complete_job: Option<u64>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Resolver {
    queue: VecDeque<WorkItem>,
    processed: usize,
}

impl Game {
    pub(crate) fn charge_resolution(&mut self) -> Result<(), RuleError> {
        self.resolver.processed += 1;
        if self.resolver.processed + self.resolver.queue.len() > MAX_RESOLUTION_ITEMS {
            return Err(RuleError::ResolutionLimit);
        }
        Ok(())
    }
    pub(crate) fn enqueue(
        &mut self,
        effects: &[EffectDef],
        frame: &Frame,
        front: bool,
    ) -> Result<(), RuleError> {
        if self.resolver.processed + self.resolver.queue.len() + effects.len()
            > MAX_RESOLUTION_ITEMS
        {
            return Err(RuleError::ResolutionLimit);
        }
        let items = effects.iter().map(|effect| WorkItem {
            effect: Some(effect.clone()),
            frame: frame.clone(),
            complete_job: None,
        });
        if front {
            for item in items.rev() {
                self.resolver.queue.push_front(item);
            }
        } else {
            self.resolver.queue.extend(items);
        }
        Ok(())
    }
    pub(crate) fn execute_effects(
        &mut self,
        context: &ResolutionContext,
        effects: &[EffectDef],
        targets: &[Pos],
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        self.execute_selected_effects(context, effects, targets, (None, None), events)
    }
    pub(crate) fn execute_selected_effects(
        &mut self,
        context: &ResolutionContext,
        effects: &[EffectDef],
        targets: &[Pos],
        (destination, scheduled): (Option<Pos>, Option<u64>),
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        for &position in targets {
            let frame = Frame {
                context: context.clone(),
                target: BoundTarget {
                    destination,
                    entity: self.at(position).map(|p| p.id),
                    position,
                    scheduled,
                    card: None,
                },
                source_archetype: self
                    .piece(context.cause.source.piece().unwrap_or(0))
                    .map(|p| p.archetype.clone()),
            };
            self.enqueue(effects, &frame, false)?;
        }
        self.drain_effect_queue(events)
    }
    pub(crate) fn drain_effect_queue(&mut self, events: &mut Vec<Event>) -> Result<(), RuleError> {
        while let Some(item) = self.resolver.queue.pop_front() {
            self.resolver.processed += 1;
            if self.resolver.processed > MAX_RESOLUTION_ITEMS {
                return Err(RuleError::ResolutionLimit);
            }
            let frame = item.frame;
            if self.is_eliminated(frame.context.cause.player) {
                self.skip_effect(&frame, "effect owner eliminated", events);
                continue;
            }
            if let Some(id) = item.complete_job {
                events.push(Event::ScheduledResolved { id });
                continue;
            }
            match item.effect.unwrap() {
                EffectDef::Sequence(effects) => self.enqueue(&effects, &frame, true)?,
                EffectDef::ForEach { target, effects } => {
                    let spec = TargetSpec {
                        origin: target.origin,
                        pattern: target.pattern.normalize().expect("validated pattern"),
                        filters: target.filters,
                        count: target.count,
                    };
                    let targets = targeting::resolve(
                        self,
                        frame.context.cause.source.piece().unwrap_or(0),
                        frame.context.cause.player,
                        &spec,
                    );
                    // Front insertion in reverse preserves canonical target/effect order.
                    for position in targets.into_iter().rev() {
                        let entity = self.at(position).map(|p| p.id);
                        if entity.is_some_and(|id| {
                            !self.can_receive(
                                frame.context.cause.source.piece().unwrap_or(0),
                                id,
                                frame.context.kind,
                            )
                        }) {
                            continue;
                        }
                        let child = Frame {
                            target: BoundTarget {
                                destination: None,
                                entity,
                                position,
                                scheduled: None,
                                card: None,
                            },
                            ..frame.clone()
                        };
                        self.enqueue(&effects, &child, true)?;
                    }
                }
                EffectDef::Schedule { after, effects } => {
                    self.schedule_effects(
                        &frame,
                        crate::clock::ClockUnit::OwnerTurns,
                        after,
                        effects,
                        events,
                    )?;
                }
                EffectDef::ScheduleOn {
                    clock,
                    after,
                    effects,
                } => {
                    self.schedule_effects(&frame, clock, after, effects, events)?;
                }
                EffectDef::DrawCard(count) => self.draw_cards(
                    frame.context.cause.player,
                    count,
                    &frame.context.cause,
                    events,
                )?,
                EffectDef::ShuffleDeck => {
                    self.shuffle_deck(frame.context.cause.player, &frame.context.cause, events)?
                }
                EffectDef::IncreaseActionsPerTurn(amount) => {
                    let owner = frame.context.cause.player;
                    let player = self
                        .state
                        .players
                        .iter_mut()
                        .find(|p| p.id == owner)
                        .expect("validated effect owner");
                    let before = player.actions_per_turn;
                    player.actions_per_turn = before.saturating_add(amount);
                    let after = player.actions_per_turn;
                    events.push(Event::ActionsPerTurnModified {
                        player: owner,
                        before,
                        after,
                        cause: frame.context.cause.clone(),
                    });
                    if owner == self.turn().player {
                        let remaining = self.turn().remaining_actions;
                        self.state.turn.remaining_actions =
                            remaining.saturating_add(after - before);
                        events.push(Event::ActionsModified {
                            player: owner,
                            before: remaining,
                            after: self.turn().remaining_actions,
                            cause: frame.context.cause,
                        });
                    }
                }
                EffectDef::ModifyActions(amount) => {
                    if frame.context.cause.player == self.turn().player {
                        let before = self.state.turn.remaining_actions;
                        self.state.turn.remaining_actions = before.saturating_add(amount);
                        events.push(Event::ActionsModified {
                            player: self.turn().player,
                            before,
                            after: self.turn().remaining_actions,
                            cause: frame.context.cause,
                        });
                    } else {
                        self.skip_effect(
                            &frame,
                            "action grants require the owner's active turn",
                            events,
                        );
                    }
                }
                EffectDef::ModifyScheduledEvent(amount) => {
                    self.modify_scheduled(&frame, amount, false, events)?
                }
                EffectDef::CancelScheduledEvent => {
                    self.modify_scheduled(&frame, 0, true, events)?
                }
                EffectDef::RemoveBoardCard => {
                    if let Some(id) = frame.target.card.filter(|id| {
                        self.card_instance(*id)
                            .is_some_and(|c| c.zone == crate::cards::CardZone::OnBoard)
                    }) {
                        self.move_card(
                            id,
                            crate::cards::CardZone::Discard,
                            &frame.context.cause,
                            events,
                        )?;
                    } else {
                        self.skip_effect(&frame, "board card already removed", events);
                    }
                }
                EffectDef::AddMana(amount) => {
                    let owner = frame.context.cause.player;
                    let player = self
                        .state
                        .players
                        .iter_mut()
                        .find(|p| p.id == owner)
                        .expect("validated owner");
                    player.mana += amount;
                    events.push(Event::ManaGranted {
                        player: owner,
                        amount,
                        cause: frame.context.cause,
                    });
                }
                EffectDef::TransformSource(archetype) => {
                    let source = frame.context.cause.source.piece().unwrap_or(0);
                    if self.piece(source).is_none_or(|p| {
                        frame
                            .source_archetype
                            .as_ref()
                            .is_some_and(|expected| expected != &p.archetype)
                    }) {
                        self.skip_effect(&frame, "source removed or transformed", events);
                        continue;
                    }
                    if !self.can_receive(source, source, frame.context.kind) {
                        self.skip_effect(&frame, "transformation source protected", events);
                        continue;
                    }
                    let piece = self.state.pieces.get_mut(&source).unwrap();
                    let from = std::mem::replace(&mut piece.archetype, archetype.clone());
                    piece.stats = self.registry.get(&archetype).unwrap().definition.base_stats;
                    piece.state = crate::state::PieceState {
                        acted: true,
                        ..Default::default()
                    };
                    events.push(Event::PieceTransformed {
                        piece: source,
                        from,
                        to: archetype,
                    });
                }
                effect => self.apply_primitive(&effect, &frame, events)?,
            }
            self.cleanse_immune(events);
        }
        Ok(())
    }
    pub(crate) fn skip_effect(&self, frame: &Frame, reason: &str, events: &mut Vec<Event>) {
        events.push(Event::EffectSkipped {
            cause: frame.context.cause.clone(),
            reason: reason.into(),
        });
    }
    pub(crate) fn enqueue_lifecycle(
        &mut self,
        piece: &PieceInstance,
        event: TriggerEvent,
        context: &ResolutionContext,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let triggers = self
            .registry
            .get(&piece.archetype)
            .unwrap()
            .definition
            .triggers
            .clone();
        for trigger in triggers.into_iter().filter(|t| t.event == event) {
            events.push(Event::TriggerFired {
                piece: piece.id,
                trigger: trigger.id.clone(),
                event,
                parent: context.cause.clone(),
            });
            let cause = EffectSource {
                source: crate::state::ActionSource::Piece(piece.id),
                player: piece.owner,
                action_id: context.action_id,
                ability: None,
                status: None,
                trigger: Some(trigger.id),
                scheduled: None,
            };
            let frame = Frame {
                context: ResolutionContext {
                    action_id: context.action_id,
                    cause,
                    kind: ActionKind::Ability,
                },
                target: BoundTarget {
                    destination: None,
                    entity: Some(piece.id),
                    position: piece.position,
                    scheduled: None,
                    card: None,
                },
                source_archetype: Some(piece.archetype.clone()),
            };
            self.enqueue(&trigger.effects, &frame, false)?;
        }
        Ok(())
    }
    pub(crate) fn initialize_triggers(&mut self) -> Result<(), RuleError> {
        let mut events = Vec::new();
        let pieces: Vec<_> = self.pieces().cloned().collect();
        for piece in pieces {
            let cause = EffectSource {
                source: crate::state::ActionSource::Piece(piece.id),
                player: piece.owner,
                action_id: 0,
                ability: None,
                status: None,
                trigger: None,
                scheduled: None,
            };
            self.enqueue_lifecycle(
                &piece,
                TriggerEvent::Spawned,
                &ResolutionContext {
                    action_id: 0,
                    cause,
                    kind: ActionKind::Ability,
                },
                &mut events,
            )?;
        }
        self.drain_effect_queue(&mut events)?;
        self.state.setup_events.extend(events);
        self.resolver = Default::default();
        Ok(())
    }
    pub(crate) fn schedule_effects(
        &mut self,
        frame: &Frame,
        clock: crate::clock::ClockUnit,
        after: u32,
        effects: Vec<EffectDef>,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if self.state.scheduled.len() >= MAX_SCHEDULED_WORK {
            return Err(RuleError::ScheduleLimit);
        }
        let owner = frame.context.cause.player;
        let due_tick = self
            .clock_value(owner, clock)
            .checked_add(u64::from(after))
            .ok_or(RuleError::CounterOverflow)?;
        let id = self.state.next_scheduled;
        self.state.next_scheduled = id.checked_add(1).ok_or(RuleError::CounterOverflow)?;
        let job = ScheduledWork {
            id,
            owner,
            due_tick,
            clock,
            kind: frame.context.kind,
            cause: frame.context.cause.clone(),
            target: frame.target.clone(),
            source_archetype: frame.source_archetype.clone(),
            effects,
        };
        events.push(Event::EffectsScheduled {
            id,
            owner,
            due_tick,
            clock,
            cause: job.cause.clone(),
        });
        self.state.scheduled.push(job);
        Ok(())
    }
    fn enqueue_job(
        &mut self,
        job: ScheduledWork,
        action_id: u64,
        front: bool,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let id = job.id;
        let mut cause = job.cause;
        cause.scheduled = Some(id);
        let frame = Frame {
            context: ResolutionContext {
                action_id,
                cause,
                kind: job.kind,
            },
            target: job.target,
            source_archetype: job.source_archetype,
        };
        // Reserve the completion marker as well as the effects in the same budget.
        if self.resolver.processed + self.resolver.queue.len() + job.effects.len() + 1
            > MAX_RESOLUTION_ITEMS
        {
            return Err(RuleError::ResolutionLimit);
        }
        events.push(Event::ScheduledReady { id });
        let completion = WorkItem {
            effect: None,
            frame: frame.clone(),
            complete_job: Some(id),
        };
        if front {
            self.resolver.queue.push_front(completion);
            self.enqueue(&job.effects, &frame, true)?;
        } else {
            self.enqueue(&job.effects, &frame, false)?;
            self.resolver.queue.push_back(completion);
        }
        Ok(())
    }
    pub(crate) fn resolve_due_on(
        &mut self,
        clock: crate::clock::ClockUnit,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let owner = self.turn().player;
        let now = self.clock_value(owner, clock);
        let mut due = Vec::new();
        self.state.scheduled.retain(|job| {
            if job.clock == clock
                && (clock != crate::clock::ClockUnit::OwnerTurns || job.owner == owner)
                && job.due_tick <= now
            {
                due.push(job.clone());
                false
            } else {
                true
            }
        });
        due.sort_by_key(|job| (job.due_tick, job.id));
        for job in due {
            if self.is_eliminated(job.owner) {
                events.push(Event::ScheduledCancelled {
                    id: job.id,
                    cause: crate::cards::system_cause(self.turn().player, action_id),
                });
                continue;
            }
            self.enqueue_job(job, action_id, false, events)?;
            self.drain_effect_queue(events)?;
        }
        Ok(())
    }
    fn modify_scheduled(
        &mut self,
        frame: &Frame,
        reduction: u32,
        cancel: bool,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let Some(index) = self.state.scheduled.iter().position(|j| {
            Some(j.id) == frame.target.scheduled && j.owner == frame.context.cause.player
        }) else {
            self.skip_effect(frame, "scheduled target no longer exists", events);
            return Ok(());
        };
        if cancel {
            let job = self.state.scheduled.remove(index);
            events.push(Event::ScheduledCancelled {
                id: job.id,
                cause: frame.context.cause.clone(),
            });
        } else {
            let now = self.clock_value(
                self.state.scheduled[index].owner,
                self.state.scheduled[index].clock,
            );
            let job = &mut self.state.scheduled[index];
            let before = job.due_tick;
            job.due_tick = job.due_tick.saturating_sub(u64::from(reduction)).max(now);
            events.push(Event::ScheduledChanged {
                id: job.id,
                before,
                after: job.due_tick,
                cause: frame.context.cause.clone(),
            });
            if job.due_tick <= now {
                let job = self.state.scheduled.remove(index);
                self.enqueue_job(job, frame.context.action_id, true, events)?;
            }
        }
        Ok(())
    }
}
