use super::*;
use cchess_core::{simulation::Simulation, Event};
use std::collections::HashMap;

/// Sparse history-free checkpoints bound memory and make scrubbing independent of live play.
pub(super) struct Replay {
    actions: Vec<ActionIntent>,
    checkpoints: Vec<(usize, Simulation)>,
    state: Simulation,
    index: usize,
    labels: Vec<Value>,
}
impl Replay {
    pub fn new(config: &Config, actions: Vec<ActionIntent>) -> Result<Self, String> {
        let initial = create(config)?.simulation();
        let mut state = initial.clone();
        let stride = actions.len().div_ceil(64).max(1);
        let mut checkpoints = vec![(0, initial.clone())];
        let mut labels = vec![];
        for (i, action) in actions.iter().enumerate() {
            labels.push(json!({"index":i,"label":choice(&state,action,"")["label"],"turn":state.turn().number.to_string(),"player":state.turn().player}));
            state
                .apply(action.clone())
                .map_err(|e| format!("Replay action {}: {e}", i + 1))?;
            if (i + 1) % stride == 0 {
                checkpoints.push((i + 1, state.clone()));
            }
        }
        Ok(Self {
            actions,
            checkpoints,
            state: initial,
            index: 0,
            labels,
        })
    }
    pub fn seek(&mut self, index: usize) -> Result<(), String> {
        if index > self.actions.len() {
            return Err("Replay position out of range".into());
        }
        let (start, checkpoint) = self
            .checkpoints
            .iter()
            .rev()
            .find(|(i, _)| *i <= index)
            .unwrap();
        if self.index > index || self.index < *start {
            self.state = checkpoint.clone();
            self.index = *start;
        }
        while self.index < index {
            self.state
                .apply(self.actions[self.index].clone())
                .map_err(|e| e.to_string())?;
            self.index += 1;
        }
        Ok(())
    }
    pub fn view(&self, config: &Config, revision: u64) -> Result<Value, String> {
        let mut view = project(&self.state, config, revision, &[], true, false)?;
        view["hand"] = json!([]);
        let ghost = if let Some(action) = self.actions.get(self.index) {
            let mut after = self.state.clone();
            let resolved = after.apply(action.clone()).map_err(|e| e.to_string())?;
            ghost(&self.state, &after, action, &resolved.events)
        } else {
            Value::Null
        };
        // Only return the nearby timeline window; long matches stay inexpensive to seek.
        let start = self.index.saturating_sub(4);
        let end = (self.index + 5).min(self.labels.len());
        view["replay"] = json!({"index":self.index,"total":self.actions.len(),"entries":self.labels[start..end],"ghost":ghost});
        Ok(view)
    }
}
fn ghost(before: &Game, after: &Game, action: &ActionIntent, events: &[Event]) -> Value {
    let mut positions: HashMap<_, _> = before.pieces().map(|p| (p.id, p.position)).collect();
    let mut marks = vec![];
    let mut paths = vec![];
    let mut tokens = vec![];
    let meta = choice(before, action, "");
    let mut mark = |position: Pos, kind: &str| marks.push(json!({"position":position,"kind":kind}));
    if let Some(id) = meta["source"].as_str().and_then(|s| s.parse::<u64>().ok()) {
        if let Some(pos) = positions.get(&id) {
            mark(*pos, "source");
            if let Some(p) = before.piece(id) {
                tokens.push(json!({"position":pos,"archetype":p.archetype,"owner":p.owner}));
            }
        }
    }
    if let Ok(pos) = serde_json::from_value::<Pos>(meta["position"].clone()) {
        mark(pos, "aim");
    }
    for event in events {
        match event {
            Event::PieceMoved { piece, from, to } => {
                paths.push(json!({"from":from,"to":to}));
                mark(*from, "from");
                mark(*to, "move");
                positions.insert(*piece, *to);
                if let Some(p) = after.piece(*piece).or_else(|| before.piece(*piece)) {
                    tokens.push(json!({"position":to,"archetype":p.archetype,"owner":p.owner}));
                }
            }
            Event::PieceSpawned {
                piece,
                position,
                archetype,
                owner,
            } => {
                positions.insert(*piece, *position);
                mark(*position, "spawn");
                tokens.push(json!({"position":position,"archetype":archetype,"owner":owner}));
            }
            Event::PieceRevived {
                piece,
                position,
                archetype,
                ..
            } => {
                positions.insert(*piece, *position);
                mark(*position, "spawn");
                if let Some(p) = after.piece(*piece) {
                    tokens.push(json!({"position":position,"archetype":archetype,"owner":p.owner}));
                }
            }
            Event::PieceTransformed { piece, to, .. } => {
                if let Some(position) = positions.get(piece) {
                    mark(*position, "effect");
                    if let Some(p) = after.piece(*piece) {
                        tokens.push(json!({"position":position,"archetype":to,"owner":p.owner}));
                    }
                }
            }
            Event::TraversalVisited { position, .. }
            | Event::TransportCollision { position, .. } => mark(*position, "effect"),
            Event::DamageDealt { target, .. } => {
                if let Some(p) = positions.get(target) {
                    mark(*p, "damage");
                }
            }
            Event::PieceRemoved { piece, .. } => {
                if let Some(p) = positions.remove(piece) {
                    mark(p, "removed");
                }
            }
            Event::StatusApplied { piece, .. }
            | Event::StatusRemoved { piece, .. }
            | Event::StatModified { piece, .. }
            | Event::ActionReset { piece, .. }
            | Event::CooldownReady { piece, .. } => {
                if let Some(p) = positions.get(piece) {
                    mark(*p, "effect");
                }
            }
            _ => {}
        }
    }
    json!({"label":meta["label"],"lines":changes(before,after),"marks":marks,"paths":paths,"tokens":tokens})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_seeks_without_mutating_live_and_ghost_precedes_action() {
        let mut session = Session::new(Config {
            preset: "classic".into(),
            seats: vec!["human".into(); 2],
        })
        .unwrap();
        let initial = session.game.simulation();
        let source = session.game.at(Pos::new(1, 0)).unwrap().id;
        session
            .commit(ActionIntent::Piece {
                player: 0,
                source,
                kind: cchess_core::content::ActionKind::Move,
                target: Pos::new(0, 2),
            })
            .unwrap();
        let final_state = session.game.simulation();
        let v = session.request(json!({"op":"replay_start"})).unwrap();
        assert_eq!(v["replay"]["index"], 0);
        assert_eq!(session.replay.as_ref().unwrap().state.clone(), initial);
        assert_eq!(v["replay"]["ghost"]["paths"][0]["to"], json!({"x":0,"y":2}));
        assert!(v["choices"].as_array().unwrap().is_empty());
        for op in ["submit", "undo", "new", "pause", "load", "save_local"] {
            assert!(session.request(json!({"op":op})).is_err());
        }
        for index in [1, 0, 1] {
            let v=session.request(json!({"op":"replay_seek","index":index,"revision":session.revision.to_string()})).unwrap();
            assert_eq!(v["replay"]["ghost"].is_null(), index == 1);
            assert_eq!(session.game.simulation(), final_state);
        }
        assert_eq!(session.replay.as_ref().unwrap().state.clone(), final_state);
        assert!(session
            .request(json!({"op":"replay_seek","index":2,"revision":session.revision.to_string()}))
            .is_err());
        session.request(json!({"op":"replay_exit"})).unwrap();
        assert_eq!(session.game.simulation(), final_state);
        assert!(session.paused);
    }
    #[test]
    fn checkpoint_seeks_match_engine_across_presets() {
        for preset in [
            "classic",
            "cards",
            "abilities",
            "events",
            "siege",
            "cross24",
        ] {
            let config = Config {
                preset: preset.into(),
                seats: vec!["human".into(); if preset == "cross24" { 4 } else { 2 }],
            };
            let mut game = create(&config).unwrap();
            let mut actions = vec![];
            let mut snapshots = vec![game.simulation()];
            for _ in 0..140 {
                let action = ActionIntent::EndTurn {
                    player: game.turn().player,
                };
                if game.apply(action.clone()).is_err() {
                    break;
                }
                actions.push(action);
                snapshots.push(game.simulation());
            }
            let mut replay = Replay::new(&config, actions).unwrap();
            assert!(replay.checkpoints.len() <= 65);
            for index in [snapshots.len() - 1, 1, 0, snapshots.len() / 2, 2] {
                replay.seek(index).unwrap();
                assert_eq!(
                    replay.state.clone(),
                    snapshots[index],
                    "{preset} index {index}"
                );
            }
        }
    }
}

#[cfg(test)]
mod effect_tests {
    use super::*;
    #[test]
    fn demo_actions_mark_resolved_damage_removal_and_summons() {
        let mut damage = 0;
        let mut spawned = 0;
        let mut removed = 0;
        for preset in ["abilities", "cards", "siege", "events", "catalogue"] {
            let game = create(&Config {
                preset: preset.into(),
                seats: vec!["human".into(); 2],
            })
            .unwrap();
            let mut actions: Vec<_> = game
                .pieces()
                .filter(|p| p.owner == game.turn().player)
                .flat_map(|p| game.legal_actions(p.id))
                .collect();
            for id in &game.player(game.turn().player).unwrap().cards.hand {
                actions.extend(game.legal_card_plays(*id));
            }
            for action in actions {
                let mut after = game.simulation();
                let result = after.apply(action.clone()).unwrap();
                let preview = ghost(&game, &after, &action, &result.events);
                let marks = preview["marks"].as_array().unwrap();
                for event in result.events {
                    let kind = match event {
                        Event::DamageDealt { .. } => {
                            damage += 1;
                            "damage"
                        }
                        Event::PieceRemoved { .. } => {
                            removed += 1;
                            "removed"
                        }
                        Event::PieceSpawned { .. } => {
                            spawned += 1;
                            "spawn"
                        }
                        _ => continue,
                    };
                    assert!(
                        marks.iter().any(|m| m["kind"] == kind),
                        "Missing {kind} for {action:?}"
                    );
                }
            }
        }
        assert!(damage > 0 && spawned > 0 && removed > 0);
    }
}
