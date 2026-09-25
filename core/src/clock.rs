//! Logical match clocks. No wall-clock time enters deterministic game state.
use crate::{
    commands::Event,
    rules::RuleError,
    state::{Game, PlayerId},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockUnit {
    #[default]
    OwnerTurns,
    Movements,
    Turns,
    Rounds,
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameClock {
    pub movements: u64,
    pub turns: u64,
    pub rounds: u64,
}
impl Game {
    pub fn clock(&self) -> &GameClock {
        &self.state.clock
    }
    pub fn clock_value(&self, owner: PlayerId, clock: ClockUnit) -> u64 {
        match clock {
            ClockUnit::OwnerTurns => self.owner_turn(owner).unwrap_or(0),
            ClockUnit::Movements => self.state.clock.movements,
            ClockUnit::Turns => self.state.clock.turns,
            ClockUnit::Rounds => self.state.clock.rounds,
        }
    }
    pub(crate) fn tick_clock(
        &mut self,
        clock: ClockUnit,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        self.advance_clock_counter(clock, events)?;
        self.resolve_clock_tick(clock, action_id, events)
    }
    pub(crate) fn advance_clock_counter(
        &mut self,
        clock: ClockUnit,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        let value = match clock {
            ClockUnit::Movements => &mut self.state.clock.movements,
            ClockUnit::Turns => &mut self.state.clock.turns,
            ClockUnit::Rounds => &mut self.state.clock.rounds,
            ClockUnit::OwnerTurns => unreachable!("owner starts belong to the turn loop"),
        };
        *value = value.checked_add(1).ok_or(RuleError::CounterOverflow)?;
        events.push(Event::ClockAdvanced {
            clock,
            value: *value,
        });
        Ok(())
    }
    pub(crate) fn resolve_clock_tick(
        &mut self,
        clock: ClockUnit,
        action_id: u64,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if !self.is_over() {
            self.resolve_due_on(clock, action_id, events)?;
            self.fire_clock_triggers(clock, action_id, events)?;
        }
        Ok(())
    }
}
