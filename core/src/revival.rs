//! Revival consumes a death record and creates a new identity with explicit tags.
use crate::{
    commands::Event,
    content::{Tag, TriggerEvent},
    events::Frame,
    state::{Game, PieceInstance, PieceState, PlayerId, RevivalLink},
    RuleError,
};
use std::collections::BTreeSet;

impl Game {
    /// Works for living pieces and death records alike; all runtime tag users share it.
    pub fn effective_tags(&self, piece: &PieceInstance) -> BTreeSet<Tag> {
        self.registry
            .get(&piece.archetype)
            .unwrap()
            .definition
            .tags
            .union(&piece.added_tags)
            .copied()
            .filter(|tag| !piece.suppressed_tags.contains(tag))
            .collect()
    }

    pub(crate) fn latest_revivable(&self, owner: PlayerId) -> Option<usize> {
        self.state
            .dead
            .iter()
            .rposition(|d| d.piece.owner == owner && d.revived_as.is_none())
    }

    pub(crate) fn revive_latest(
        &mut self,
        frame: &Frame,
        undead: bool,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let owner = frame.context.cause.player;
        let index = self
            .latest_revivable(owner)
            .ok_or(RuleError::InvalidTarget)?;
        if self.at(frame.target.position).is_some() || !self.is_playable(frame.target.position) {
            return Err(RuleError::InvalidTarget);
        }
        let corpse = &self.state.dead[index].piece;
        let id = self.state.next_entity;
        let piece = PieceInstance {
            id,
            archetype: corpse.archetype.clone(),
            owner,
            position: frame.target.position,
            stats: self
                .registry
                .get(&corpse.archetype)
                .unwrap()
                .definition
                .base_stats,
            state: PieceState {
                acted: true,
                ..Default::default()
            },
            statuses: Default::default(),
            added_tags: if undead {
                [Tag::Undead].into_iter().collect()
            } else {
                Default::default()
            },
            suppressed_tags: if undead {
                [Tag::Biological, Tag::Heroic, Tag::Immune]
                    .into_iter()
                    .collect()
            } else {
                Default::default()
            },
            revival: Some(RevivalLink {
                previous_entity: corpse.id,
                death_index: index,
                revived_by: frame
                    .context
                    .cause
                    .source
                    .piece()
                    .ok_or(RuleError::InvalidTarget)?,
                action_id: frame.context.action_id,
            }),
        };
        self.state.next_entity = id.checked_add(1).ok_or(RuleError::CounterOverflow)?;
        self.state.dead[index].revived_as = Some(id);
        self.insert_piece(piece.clone());
        events.push(Event::PieceRevived {
            piece: id,
            previous: piece.revival.as_ref().unwrap().previous_entity,
            death_index: index,
            archetype: piece.archetype.clone(),
            position: piece.position,
            cause: frame.context.cause.clone(),
        });
        self.enqueue_lifecycle(&piece, TriggerEvent::Spawned, &frame.context, events)?;
        Ok(())
    }
}
