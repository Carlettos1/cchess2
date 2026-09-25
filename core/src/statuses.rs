use crate::{
    commands::Event,
    content::ActionKind,
    effects::ResolutionContext,
    rules::RuleError,
    state::{EffectSource, EntityId, Game, PlayerId, StatusInstance, StatusKind},
};

impl Game {
    pub(crate) fn cleanse_immune(&mut self, events: &mut Vec<Event>) {
        let ids: Vec<_> = self
            .pieces()
            .filter(|p| self.has_tag(p.id, crate::content::Tag::Immune))
            .map(|p| p.id)
            .collect();
        for id in ids {
            let statuses = std::mem::take(&mut self.state.pieces.get_mut(&id).unwrap().statuses);
            for kind in statuses.keys() {
                events.push(Event::StatusRemoved {
                    piece: id,
                    kind: *kind,
                    expired: false,
                });
            }
        }
    }

    pub(crate) fn add_status(
        &mut self,
        target: EntityId,
        kind: StatusKind,
        duration: u32,
        mut source: EffectSource,
        events: &mut Vec<Event>,
    ) {
        if self.has_tag(target, crate::content::Tag::Immune) {
            return;
        }
        source.status = Some(kind);
        let turn = self.turn().number;
        let statuses = &mut self.state.pieces.get_mut(&target).unwrap().statuses;
        let refreshed = statuses.contains_key(&kind);
        // Positive effects keep the longer duration; negative effects keep the shorter.
        let incoming = StatusInstance {
            kind,
            remaining: duration,
            applied_turn: turn,
            source,
        };
        let status = statuses.entry(kind).or_insert_with(|| incoming.clone());
        if (kind == StatusKind::Invulnerable && duration > status.remaining)
            || (kind != StatusKind::Invulnerable && duration < status.remaining)
        {
            *status = incoming;
        }
        events.push(Event::StatusApplied {
            piece: target,
            kind,
            duration: status.remaining,
            refreshed,
            cause: status.source.clone(),
        });
    }

    pub(crate) fn tick_owner_timers(
        &mut self,
        action_id: u64,
        owner: PlayerId,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        self.cleanse_immune(events);
        let turn = self.turn().number;
        let mut expired = Vec::new();
        // Collect and remove all expirations first, in entity-ID/status-kind order.
        // Fire then resolves through the same damage/death layer as piece actions.
        for piece in self.state.pieces.values_mut().filter(|p| p.owner == owner) {
            piece.state.cooldowns.retain(|ability, cooldown| {
                if cooldown.applied_turn != turn {
                    cooldown.remaining = cooldown.remaining.saturating_sub(1);
                }
                if cooldown.remaining == 0 {
                    events.push(Event::CooldownReady {
                        piece: piece.id,
                        ability: ability.clone(),
                    });
                    false
                } else {
                    true
                }
            });
            piece.statuses.retain(|kind, status| {
                if status.applied_turn != turn {
                    status.remaining = status.remaining.saturating_sub(1);
                }
                if status.remaining == 0 {
                    events.push(Event::StatusRemoved {
                        piece: piece.id,
                        kind: *kind,
                        expired: true,
                    });
                    expired.push((piece.id, status.clone()));
                    false
                } else {
                    true
                }
            });
        }
        for (target, status) in expired {
            if status.kind == StatusKind::Fire {
                // Immunity cleanses pending Fire before expiration processing.
                let context = ResolutionContext {
                    action_id,
                    kind: if matches!(status.source.source, crate::state::ActionSource::Card(_)) {
                        ActionKind::PlayCard
                    } else {
                        ActionKind::Ability
                    },
                    cause: status.source,
                };
                self.damage_piece(&context, target, 1, events)?;
            }
        }
        self.drain_effect_queue(events)
    }
}
