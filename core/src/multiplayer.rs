//! Four-player free-for-all setup, elimination, and live-seat turn order.
use crate::{
    cards::{CardZone, DeckDef},
    commands::Event,
    effects::ResolutionContext,
    state::{DeathRecord, Placement, Player, PlayerId},
    Bounds, Direction, Game, Pos, Registry, RuleError,
};

impl Game {
    pub fn is_playable(&self, pos: Pos) -> bool {
        self.bounds().contains(pos) && !self.state.excluded_tiles.contains(&pos)
    }

    pub fn players(&self) -> &[Player] {
        &self.state.players
    }
    pub fn is_eliminated(&self, player: PlayerId) -> bool {
        self.state.eliminated.contains(&player)
    }
    pub fn is_draw(&self) -> bool {
        self.state.drawn
    }
    pub fn is_over(&self) -> bool {
        self.state.winner.is_some() || self.is_draw()
    }

    pub(crate) fn next_active_index(&self, after: usize) -> usize {
        (1..=self.players().len())
            .map(|n| (after + n) % self.players().len())
            .find(|&i| !self.is_eliminated(self.players()[i].id))
            .expect("nonterminal match has active players")
    }

    pub(crate) fn emit_outcome(&mut self, events: &mut Vec<Event>) -> bool {
        if let Some(player) = self.winner() {
            self.state.pending_promotion = None;
            events.push(Event::GameWon { player });
            true
        } else if self.is_draw() {
            self.state.pending_promotion = None;
            events.push(Event::GameDrawn);
            true
        } else {
            false
        }
    }

    pub(crate) fn eliminate_player(
        &mut self,
        player: PlayerId,
        context: &ResolutionContext,
        events: &mut Vec<Event>,
    ) -> Result<(), RuleError> {
        if !self.state.eliminated.insert(player) {
            return Ok(());
        }
        events.push(Event::PlayerEliminated { player });
        // Administrative cleanup does not fire death rewards or elimination cascades.
        let ids: Vec<_> = self
            .pieces()
            .filter(|p| p.owner == player)
            .map(|p| p.id)
            .collect();
        for id in ids {
            self.charge_resolution()?;
            let piece = self.detach_piece(id).unwrap();
            self.state.dead.push(DeathRecord {
                revived_as: None,
                piece,
                action_id: context.action_id,
                source: context.cause.source,
                cause: context.cause.clone(),
            });
            events.push(Event::PieceRemoved {
                piece: id,
                source: context.cause.source,
            });
        }
        let cards: Vec<_> = self
            .card_instances()
            .filter(|c| {
                c.owner == player
                    && matches!(c.zone, CardZone::OnBoard | CardZone::Hand | CardZone::Deck)
            })
            .map(|c| c.id)
            .collect();
        for id in cards {
            self.charge_resolution()?;
            self.move_card(id, CardZone::Discard, &context.cause, events)?;
        }
        let jobs: Vec<_> = self
            .state
            .scheduled
            .iter()
            .filter(|j| j.owner == player)
            .map(|j| j.id)
            .collect();
        self.state.scheduled.retain(|j| j.owner != player);
        for id in jobs {
            events.push(Event::ScheduledCancelled {
                id,
                cause: context.cause.clone(),
            });
        }
        if self.pending_promotion().is_some_and(|p| p.player == player) {
            self.state.pending_promotion = None;
        }
        let remaining: Vec<_> = self
            .players()
            .iter()
            .filter(|p| !self.is_eliminated(p.id))
            .map(|p| p.id)
            .collect();
        self.state.winner = if remaining.len() == 1 {
            Some(remaining[0])
        } else {
            None
        };
        self.state.drawn = remaining.is_empty();
        Ok(())
    }

    /// Cross-shaped 14x14 board with four 3x3 corners removed.
    pub fn four_player(registry: Registry) -> Result<Self, String> {
        let back = [
            "rook", "knight", "bishop", "queen", "king", "bishop", "knight", "rook",
        ];
        let mut placements = Vec::new();
        for owner in 0..4 {
            let rotate = |x, y| match owner {
                0 => Pos::new(x, y),
                1 => Pos::new(y, 13 - x),
                2 => Pos::new(13 - x, 13 - y),
                _ => Pos::new(13 - y, x),
            };
            for (i, name) in back.iter().enumerate() {
                let x = i as i32 + 3;
                placements.push(Placement::new(name, owner, rotate(x, 0)));
                placements.push(Placement::new("pawn", owner, rotate(x, 1)));
            }
        }
        let players = [Direction::N, Direction::E, Direction::S, Direction::W]
            .into_iter()
            .enumerate()
            .map(|(id, forward)| {
                let mut p = Player::new(id as u8, forward);
                p.mana = 1.into();
                p.mana_per_turn = 1;
                p
            })
            .collect();
        let has_cards = registry.cards().next().is_some();
        let game = Self::new(
            registry,
            Bounds {
                width: 14,
                height: 14,
            },
            players,
            placements,
        )?;
        if has_cards {
            let deck: DeckDef = ron::from_str(include_str!("../../assets/decks/starter.ron"))
                .map_err(|e| e.to_string())?;
            game.with_decks(
                (0..4).map(|owner| (owner, deck.clone())).collect(),
                42,
                3,
                1,
                true,
            )
        } else {
            Ok(game)
        }
    }
}
