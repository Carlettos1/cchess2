//! Directional placement and ordered traversal; neither dispatches on piece IDs.
use crate::{
    commands::Event,
    content::{TraverseDef, TraverseEffect},
    events::Frame,
    geometry::{Direction, Pos, Relative},
    rules::RuleError,
    state::{EntityId, Game},
};

impl Game {
    pub(crate) fn oriented_spawn_positions(
        &self,
        source: EntityId,
        toward: Pos,
        directions: &[Relative],
    ) -> Result<Vec<Pos>, RuleError> {
        let origin = self
            .piece(source)
            .ok_or(RuleError::SourceMissing(source))?
            .position;
        let facing = match (toward.x - origin.x, toward.y - origin.y) {
            (0, 1) => Direction::N,
            (1, 0) => Direction::E,
            (0, -1) => Direction::S,
            (-1, 0) => Direction::W,
            _ => return Err(RuleError::InvalidTarget),
        };
        let mut positions = Vec::new();
        for relative in directions {
            let (dx, dy) = relative.absolute(facing);
            let position = origin.offset(dx, dy).ok_or(RuleError::InvalidTarget)?;
            if !self.is_playable(position) || self.at(position).is_some() {
                return Err(RuleError::InvalidTarget);
            }
            positions.push(position);
        }
        positions.sort();
        positions.dedup();
        Ok(positions)
    }

    pub(crate) fn traverse(
        &mut self,
        frame: &Frame,
        def: &TraverseDef,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let source = frame.context.cause.source.piece().unwrap_or(0);
        let Some(piece) = self.piece(source) else {
            self.skip_effect(frame, "traversal source removed", events);
            return Ok(());
        };
        let origin = piece.position;
        let direction = (
            frame.target.position.x - origin.x,
            frame.target.position.y - origin.y,
        );
        if direction == (0, 0) || direction.0.abs() > 1 || direction.1.abs() > 1 {
            return Err(RuleError::InvalidTarget);
        }
        let strength = piece.stats.get(def.stop_when.source);
        let mut cursor = origin;
        let mut last_empty = origin;
        events.push(Event::TraversalStarted {
            source,
            from: origin,
            direction,
        });
        while let Some(position) = cursor
            .offset(direction.0, direction.1)
            .filter(|p| self.is_playable(*p))
        {
            self.charge_resolution()?;
            cursor = position;
            events.push(Event::TraversalVisited { source, position });
            let target = self.at(position).map(|p| p.id);
            let blocked = target.is_some_and(|id| {
                def.stop_when.comparison.test(
                    self.piece(id).unwrap().stats.get(def.stop_when.target),
                    strength,
                )
            });
            if blocked {
                events.push(Event::TraversalStopped {
                    source,
                    blocker: target.unwrap(),
                    position,
                });
                self.traversal_effects(frame, &def.on_stop, target, &mut last_empty, None, events)?;
                // Stop hooks do not turn a destroyed blocker into a pass square.
                break;
            }
            if target.is_none() {
                last_empty = position;
            }
            self.traversal_effects(
                frame,
                &def.on_pass,
                target,
                &mut last_empty,
                Some(position),
                events,
            )?;
            // Occupants which survive (including protected pieces) are not landing squares.
            // They do not block the ray unless the comparison above says so.
            if self.at(position).is_none() {
                last_empty = position;
            }
            if self.piece(source).is_none() {
                break;
            }
        }
        // End effects run after both a blocker stop and reaching the board edge.
        self.traversal_effects(frame, &def.on_end, None, &mut last_empty, None, events)?;
        events.push(Event::TraversalEnded { source, last_empty });
        Ok(())
    }

    fn traversal_effects(
        &mut self,
        frame: &Frame,
        effects: &[TraverseEffect],
        target: Option<EntityId>,
        last_empty: &mut Pos,
        pass_square: Option<Pos>,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let source = frame.context.cause.source.piece().unwrap_or(0);
        for effect in effects {
            self.charge_resolution()?;
            if let Some(position) = pass_square.filter(|p| self.at(*p).is_none()) {
                *last_empty = position;
            }
            match effect {
                TraverseEffect::DamageTarget(amount) => {
                    if let Some(id) = target.filter(|id| self.piece(*id).is_some()) {
                        if self.can_receive(source, id, frame.context.kind) {
                            self.damage_piece(&frame.context, id, *amount, events)?;
                        } else {
                            self.skip_effect(frame, "traversal target protected", events);
                        }
                    }
                }
                TraverseEffect::DamageSource(amount) => {
                    self.damage_piece(&frame.context, source, *amount, events)?;
                }
                TraverseEffect::MoveSourceToLastEmpty => {
                    if !self.can_receive(source, source, frame.context.kind) {
                        continue;
                    }
                    if self
                        .piece(source)
                        .is_some_and(|p| p.position != *last_empty)
                        && self.at(*last_empty).is_none()
                    {
                        self.move_entity(source, *last_empty, events);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Game {
    pub(crate) fn validate_transport(
        &self,
        player: crate::state::PlayerId,
        source: EntityId,
        ability: &str,
        target: Pos,
        destination: Pos,
    ) -> Result<(), RuleError> {
        let (definition, _) = self.validate_ability(player, source, ability, Some(target))?;
        let [crate::content::EffectDef::Transport { max_range, .. }] =
            definition.definition.effects.as_slice()
        else {
            return Err(RuleError::UnsupportedAction);
        };
        self.transport_direction(target, destination, *max_range)?;
        Ok(())
    }

    pub(crate) fn transport_direction(
        &self,
        from: Pos,
        destination: Pos,
        max_range: u16,
    ) -> Result<((i32, i32), u32), RuleError> {
        if !self.is_playable(destination) {
            return Err(RuleError::OutOfBounds(destination));
        }
        let dx = destination.x - from.x;
        let dy = destination.y - from.y;
        let distance = dx.unsigned_abs().max(dy.unsigned_abs());
        if distance == 0
            || distance > u32::from(max_range)
            || (dx != 0 && dy != 0 && dx.abs() != dy.abs())
        {
            return Err(RuleError::InvalidTarget);
        }
        Ok(((dx.signum(), dy.signum()), distance))
    }

    /// Walk a selected piece along an eight-direction ray, stopping before the
    /// first occupant. Collision damage never permits entry into its square.
    pub(crate) fn transport(
        &mut self,
        frame: &Frame,
        max_range: u16,
        damage: u32,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let source = frame
            .context
            .cause
            .source
            .piece()
            .ok_or(RuleError::InvalidTarget)?;
        let target = frame.target.entity.ok_or(RuleError::InvalidTarget)?;
        let destination = frame.target.destination.ok_or(RuleError::InvalidTarget)?;
        let piece = self.piece(target).ok_or(RuleError::SourceMissing(target))?;
        let from = piece.position;
        let (direction, distance) = self.transport_direction(from, destination, max_range)?;
        events.push(Event::TransportStarted {
            source,
            target,
            from,
            destination,
        });
        let mut last_empty = from;
        for _ in 0..distance {
            self.charge_resolution()?;
            let position = last_empty
                .offset(direction.0, direction.1)
                .ok_or(RuleError::InvalidTarget)?;
            if !self.is_playable(position) {
                break;
            }
            if let Some(blocker) = self.at(position).map(|p| p.id) {
                events.push(Event::TransportCollision {
                    source,
                    target,
                    blocker,
                    position,
                });
                // Relocate first so death records and removal reactions use the impact site.
                if last_empty != from {
                    self.move_entity(target, last_empty, events);
                }
                // Both collision participants receive damage, with normal ability protection.
                for id in [blocker, target] {
                    if self.can_receive(source, id, frame.context.kind) {
                        self.damage_piece(&frame.context, id, damage, events)?;
                    } else {
                        self.skip_effect(frame, "collision target protected", events);
                    }
                }
                break;
            }
            last_empty = position;
        }
        if self.piece(target).is_some_and(|p| p.position != last_empty) {
            self.move_entity(target, last_empty, events);
        }
        Ok(())
    }
}

impl Game {
    pub(crate) fn push_destination(
        &self,
        source: EntityId,
        target: Pos,
        distance: u16,
    ) -> Result<Pos, RuleError> {
        let origin = self
            .piece(source)
            .ok_or(RuleError::SourceMissing(source))?
            .position;
        let direction = (target.x - origin.x, target.y - origin.y);
        if direction == (0, 0) || direction.0.abs() > 1 || direction.1.abs() > 1 {
            return Err(RuleError::InvalidTarget);
        }
        let mut destination = target;
        for _ in 0..distance {
            let Some(next) = destination
                .offset(direction.0, direction.1)
                .filter(|p| self.is_playable(*p))
            else {
                break;
            };
            destination = next;
        }
        if destination == target {
            return Err(RuleError::InvalidTarget);
        }
        Ok(destination)
    }

    /// Push directly away from the caster; share collision and death handling
    /// with transport, but require no Transportable stat or chosen destination.
    pub(crate) fn push_piece(
        &mut self,
        frame: &Frame,
        distance: u16,
        damage: u32,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let source = frame
            .context
            .cause
            .source
            .piece()
            .ok_or(RuleError::InvalidTarget)?;
        let target = frame
            .target
            .entity
            .and_then(|id| self.piece(id))
            .ok_or(RuleError::InvalidTarget)?
            .position;
        let mut selected = frame.clone();
        selected.target.destination = Some(self.push_destination(source, target, distance)?);
        self.transport(&selected, distance, damage, events)
    }
}

impl Game {
    /// The first occupant blocks. A matching tag allows landing immediately
    /// beyond it; otherwise use the last empty square before it.
    pub(crate) fn seek_path(
        &self,
        source: EntityId,
        toward: Pos,
        tag: crate::content::Tag,
    ) -> Result<(Pos, Vec<Pos>), RuleError> {
        let origin = self
            .piece(source)
            .ok_or(RuleError::SourceMissing(source))?
            .position;
        let direction = (toward.x - origin.x, toward.y - origin.y);
        if direction == (0, 0) || direction.0.abs() > 1 || direction.1.abs() > 1 {
            return Err(RuleError::InvalidTarget);
        }
        let mut cursor = origin;
        let mut last_empty = origin;
        let mut visited = Vec::new();
        while let Some(pos) = cursor
            .offset(direction.0, direction.1)
            .filter(|p| self.is_playable(*p))
        {
            visited.push(pos);
            if let Some(piece) = self.at(pos) {
                if self.has_tag(piece.id, tag) {
                    if let Some(beyond) = pos
                        .offset(direction.0, direction.1)
                        .filter(|p| self.is_playable(*p) && self.at(*p).is_none())
                    {
                        return Ok((beyond, visited));
                    }
                }
                return Ok((last_empty, visited));
            }
            last_empty = pos;
            cursor = pos;
        }
        Ok((last_empty, visited))
    }

    pub(crate) fn seek_tag(
        &mut self,
        frame: &Frame,
        tag: crate::content::Tag,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let source = frame
            .context
            .cause
            .source
            .piece()
            .ok_or(RuleError::InvalidTarget)?;
        let from = self
            .piece(source)
            .ok_or(RuleError::SourceMissing(source))?
            .position;
        let (destination, path) = self.seek_path(source, frame.target.position, tag)?;
        events.push(Event::TraversalStarted {
            source,
            from,
            direction: (
                frame.target.position.x - from.x,
                frame.target.position.y - from.y,
            ),
        });
        for position in path {
            self.charge_resolution()?;
            events.push(Event::TraversalVisited { source, position });
            if let Some(blocker) = self.at(position) {
                events.push(Event::TraversalStopped {
                    source,
                    blocker: blocker.id,
                    position,
                });
            }
        }
        if destination != from {
            self.move_entity(source, destination, events);
        }
        events.push(Event::TraversalEnded {
            source,
            last_empty: destination,
        });
        Ok(())
    }
}

impl Game {
    /// Freeze same-archetype participants in the initial square, then advance
    /// front to back. Ownership is intentionally not a selection filter.
    pub(crate) fn advance_peers(
        &mut self,
        frame: &Frame,
        halfside: u16,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        use crate::{content::ActionKind, state::ActionSource};
        let caster = frame
            .context
            .cause
            .source
            .piece()
            .ok_or(RuleError::InvalidTarget)?;
        let piece = self.piece(caster).ok_or(RuleError::SourceMissing(caster))?;
        let origin = piece.position;
        let direction = (
            frame.target.position.x - origin.x,
            frame.target.position.y - origin.y,
        );
        let mut selected: Vec<_> = self
            .pieces()
            .filter(|p| {
                p.archetype == piece.archetype
                    && (p.position.x - origin.x).abs() <= i32::from(halfside)
                    && (p.position.y - origin.y).abs() <= i32::from(halfside)
            })
            .map(|p| (p.id, p.position))
            .collect();
        selected.sort_by_key(|(id, p)| (-(p.x * direction.0 + p.y * direction.1), *id));
        for (source, _) in selected {
            self.charge_resolution()?;
            if self.piece(source).is_none() {
                continue;
            }
            if !self.can_receive(caster, source, crate::content::ActionKind::Ability) {
                continue;
            }
            let piece = self.piece(source).unwrap();
            let owner = piece.owner;
            let from = piece.position;
            let mut destination = from;
            let mut context = frame.context.clone();
            context.kind = ActionKind::Take;
            context.cause.source = ActionSource::Piece(source);
            context.cause.player = owner;
            events.push(Event::TraversalStarted {
                source,
                from,
                direction,
            });
            while let Some(next) = destination
                .offset(direction.0, direction.1)
                .filter(|p| self.is_playable(*p))
            {
                self.charge_resolution()?;
                events.push(Event::TraversalVisited {
                    source,
                    position: next,
                });
                if let Some(blocker) = self.at(next) {
                    let target = blocker.id;
                    let enemy = blocker.owner != owner;
                    events.push(Event::TraversalStopped {
                        source,
                        blocker: target,
                        position: next,
                    });
                    if enemy && self.can_receive(source, target, ActionKind::Take) {
                        self.damage_piece(&context, target, 1, events)?;
                        if self.at(next).is_none() {
                            destination = next;
                        }
                    }
                    break;
                }
                destination = next;
            }
            if self.piece(source).is_some() {
                if destination != from && self.at(destination).is_none() {
                    self.move_entity(source, destination, events);
                }
                self.state.pieces.get_mut(&source).unwrap().state.acted = true;
            }
            events.push(Event::TraversalEnded {
                source,
                last_empty: destination,
            });
            if self.is_over() {
                break;
            }
        }
        Ok(())
    }
}
