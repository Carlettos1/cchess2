//! Reference enumeration kept deliberately unoptimized to detect changes in legality or order.
use cchess_engine::GameAi;
use cchess_engine::{
    cards::{CardChoice, CardTarget, CardZone},
    content::{EffectDef, Selection},
    state::{CardId, EntityId},
    targeting, *,
};
struct Reference<'a>(&'a Game);
impl std::ops::Deref for Reference<'_> {
    type Target = Game;
    fn deref(&self) -> &Game {
        self.0
    }
}
impl Reference<'_> {
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
        for rule in &self.registry().get(&piece.archetype).unwrap().rules {
            for target in targeting::resolve(self, source, piece.owner, &rule.target) {
                let intent = ActionIntent::Piece {
                    player: self.turn().player,
                    source,
                    kind: rule.kind,
                    target,
                };
                if self.validate(&intent).is_ok() {
                    actions.push(intent);
                }
            }
        }
        actions.extend(self.legal_abilities(source));
        actions
    }
    pub fn legal_piece_actions(&self, source: EntityId, kind: ActionKind) -> Vec<ActionIntent> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        if self.pending_promotion().is_some() || self.is_over() {
            return vec![];
        }
        let mut actions = Vec::new();
        for rule in self
            .registry()
            .get(&piece.archetype)
            .unwrap()
            .rules
            .iter()
            .filter(|r| r.kind == kind)
        {
            for target in targeting::resolve(self, source, piece.owner, &rule.target) {
                let intent = ActionIntent::Piece {
                    player: self.turn().player,
                    source,
                    kind,
                    target,
                };
                if self.validate(&intent).is_ok() && !actions.contains(&intent) {
                    actions.push(intent);
                }
            }
        }
        actions
    }
    pub fn legal_abilities(&self, source: EntityId) -> Vec<ActionIntent> {
        let Some(piece) = self.piece(source) else {
            return vec![];
        };
        let mut result = Vec::new();
        for ability in &self.registry().get(&piece.archetype).unwrap().abilities {
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
            let targets = match ability.definition.selection {
                Selection::One
                | Selection::Direction
                | Selection::Transport
                | Selection::ScheduledSummon => {
                    targeting::resolve(self, source, piece.owner, &ability.target)
                        .into_iter()
                        .map(Some)
                        .collect()
                }
                Selection::All => vec![None],
            };
            for target in targets {
                if ability.definition.selection == Selection::Transport {
                    let [EffectDef::Transport { max_range, .. }] =
                        ability.definition.effects.as_slice()
                    else {
                        unreachable!("validated transport ability")
                    };
                    let origin = target.unwrap();
                    let destinations = cchess_engine::geometry::SubDirection::ALL
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
                        if self.validate(&intent).is_ok() {
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
                if self.validate(&intent).is_ok() {
                    result.push(intent);
                }
            }
        }
        result
    }
    pub fn legal_card_plays(&self, id: CardId) -> Vec<ActionIntent> {
        let Some(instance) = self.card_instance(id) else {
            return vec![];
        };
        let card = self.registry().card(&instance.definition).unwrap();
        let choices = match &card.definition.target {
            CardTarget::None => vec![CardChoice::None],
            CardTarget::Square(_) => {
                targeting::resolve(self, 0, instance.owner, card.target.as_ref().unwrap())
                    .into_iter()
                    .map(CardChoice::Square)
                    .collect()
            }
            CardTarget::FriendlyScheduled => self
                .scheduled()
                .iter()
                .filter(|j| j.owner == instance.owner)
                .map(|j| CardChoice::Scheduled(j.id))
                .collect(),
            CardTarget::EnemyBoardCard => self
                .card_instances()
                .filter(|c| c.owner != instance.owner && c.zone == CardZone::OnBoard)
                .map(|c| CardChoice::Card(c.id))
                .collect(),
        };
        choices
            .into_iter()
            .map(|target| ActionIntent::PlayCard {
                player: self.turn().player,
                card: id,
                target,
            })
            .filter(|intent| self.validate(intent).is_ok())
            .collect()
    }
    pub fn ai_legal_actions(&self) -> Vec<ActionIntent> {
        if self.is_over() {
            return vec![];
        }
        if let Some(pending) = self.pending_promotion() {
            return self.legal_actions(pending.piece);
        }
        let owner = self.turn().player;
        let mut actions = Vec::new();
        for card in &self.player(owner).unwrap().cards.hand {
            actions.extend(self.legal_card_plays(*card));
        }
        for piece in self.pieces().filter(|p| p.owner == owner) {
            actions.extend(self.legal_actions(piece.id));
        }
        let mut unique = Vec::new();
        for action in actions {
            if !unique.contains(&action) {
                unique.push(action);
            }
        }
        if unique.is_empty() {
            unique.push(ActionIntent::EndTurn { player: owner });
        }
        unique
    }
}
fn compare(game: &Game) {
    let reference = Reference(game);
    assert_eq!(game.ai_legal_actions(), reference.ai_legal_actions());
    for piece in game.pieces() {
        assert_eq!(
            game.legal_actions(piece.id),
            reference.legal_actions(piece.id),
            "{}",
            piece.archetype
        );
        for kind in [ActionKind::Move, ActionKind::Take, ActionKind::Attack] {
            assert_eq!(
                game.legal_piece_actions(piece.id, kind),
                reference.legal_piece_actions(piece.id, kind)
            );
        }
    }
    for card in game.card_instances() {
        assert_eq!(
            game.legal_card_plays(card.id),
            reference.legal_card_plays(card.id)
        );
    }
    for pos in game.bounds().positions() {
        assert_eq!(game.at(pos), game.pieces().find(|p| p.position == pos));
    }
    assert!(game.at(Pos::new(-1, 0)).is_none());
}
#[test]
fn optimized_enumeration_preserves_reference_order_for_full_boards_and_demos() {
    let registry = Registry::built_in().unwrap();
    for game in [
        Game::full_board(registry.clone(), false),
        Game::full_board(registry.clone(), true),
        Game::expansion_demo(registry.clone()),
        Game::abilities_demo(registry.clone()),
        Game::siege_demo(registry.clone()),
        Game::cards_demo(registry),
    ]
    .map(Result::unwrap)
    {
        let seats = game
            .players()
            .iter()
            .map(|p| (p.id, ai::AiPolicy::Random))
            .collect();
        let mut game = game.with_ai(seats, 42).unwrap();
        for step in 0..32 {
            if step % 8 == 0 {
                compare(&game);
            }
            if game.is_over() {
                break;
            }
            game.play_ai_step().unwrap();
        }
        compare(&game);
    }
}

#[test]
fn optimized_full_board_preserves_pre_optimization_trace() {
    let path = std::env::temp_dir().join(format!(
        "cchess-optimization-golden-{}.txt",
        std::process::id()
    ));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args(["--full-board", "--auto-play"])
        .arg(&path)
        .args(["--ai-seed", "42", "--max-actions", "1000"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    // Frozen before optimization: initial state, all 833 intents and events,
    // result, and final snapshot (including the complete retained history).
    let fingerprint = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    assert_eq!(bytes.len(), 3_072_408);
    assert_eq!(fingerprint, 0xbb40b88eca26cbec_u64);
}
