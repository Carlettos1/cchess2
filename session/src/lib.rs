mod replay;
// Authoritative local game session shared by native IPC and the browser dev bridge.
use cchess_ai::{AiPolicy, GameAi};
use cchess_core::{
    cards::{CardChoice, CardZone},
    ActionIntent, Game, Pos, Registry,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub preset: String,
    #[serde(default)]
    pub seats: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            preset: "classic".into(),
            seats: vec!["human".into(), "heuristic".into()],
        }
    }
}
fn create(config: &Config) -> Result<Game, String> {
    let registry = Registry::built_in().map_err(|e| format!("Content error: {e:?}"))?;
    let game = match config.preset.as_str() {
        "classic" => Game::classical(registry),
        "expansion" => Game::full_board(registry, false),
        "cross14" => Game::four_player(registry),
        "cross24" => Game::full_board(registry, true),
        "abilities" => Game::abilities_demo(registry),
        "cards" => Game::cards_demo(registry),
        "siege" => Game::siege_demo(registry),
        "events" => Game::events_demo(registry),
        "catalogue" => Game::expansion_demo(registry),
        _ => return Err("Unknown board preset".into()),
    }?;
    if config.seats.len() != game.players().len() {
        return Err("Configure exactly one controller per seat".into());
    }
    let mut seats = vec![];
    for (i, seat) in config.seats.iter().enumerate() {
        let policy = match seat.as_str() {
            "human" => continue,
            "random" => AiPolicy::Random,
            "first" => AiPolicy::First,
            "heuristic" => AiPolicy::Heuristic,
            "mcts" => AiPolicy::Mcts,
            _ => return Err("Unknown AI policy".into()),
        };
        seats.push((i as u8, policy));
    }
    game.with_ai(seats, 42)
}

pub struct Session {
    game: Game,
    config: Config,
    revision: u64,
    generation: u64,
    history: Vec<Game>,
    paused: bool,
    choices: Vec<ActionIntent>,
    storage: Option<std::path::PathBuf>,
    replay: Option<replay::Replay>,
}
impl Session {
    pub fn new(config: Config) -> Result<Self, String> {
        let game = create(&config)?;
        Ok(Self {
            game,
            config,
            revision: 1,
            generation: 0,
            history: vec![],
            paused: false,
            choices: vec![],
            storage: None,
            replay: None,
        })
    }
    pub fn with_storage(mut self, path: std::path::PathBuf) -> Self {
        self.storage = Some(path);
        self
    }
    fn touch(&mut self) {
        self.revision += 1;
        self.generation += 1;
        self.choices.clear();
    }
    fn guard(&self, request: &Value) -> Result<(), String> {
        if request["revision"].as_str() != Some(&self.revision.to_string()) {
            return Err("Position changed. Select the action again.".into());
        }
        Ok(())
    }
    fn commit(&mut self, action: ActionIntent) -> Result<(), String> {
        let before = self.game.clone();
        self.game.apply(action).map_err(|e| e.to_string())?;
        self.history.push(before);
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.touch();
        Ok(())
    }
    fn selected_choice(&self, request: &Value) -> Result<ActionIntent, String> {
        self.guard(request)?;
        if self.game.ai_controller(self.game.turn().player).is_some() {
            return Err("It is an AI seat's turn.".into());
        }
        let id = request["choiceId"]
            .as_str()
            .ok_or("Missing action choice")?;
        let (revision, index) = id.split_once(':').ok_or("Invalid choice")?;
        if revision != self.revision.to_string() {
            return Err("Expired action choice".into());
        }
        let index: usize = index.parse().map_err(|_| "Invalid choice")?;
        self.choices
            .get(index)
            .cloned()
            .ok_or("Unknown choice".into())
    }
    pub fn request(&mut self, request: Value) -> Result<Value, String> {
        let op = request["op"].as_str().ok_or("Missing operation")?;
        if self.replay.is_some() && !matches!(op, "view" | "replay_seek" | "replay_exit") {
            return Err("Exit replay to change the live match".into());
        }
        match op {
            "replay_start" => {
                let actions = self.game.log().iter().map(|a| a.intent.clone()).collect();
                let replay = replay::Replay::new(&self.config, actions)?;
                self.paused = true;
                self.touch();
                self.replay = Some(replay);
                self.view()
            }
            "replay_seek" => {
                self.guard(&request)?;
                let index = request["index"].as_u64().ok_or("Missing replay position")?;
                self.replay
                    .as_mut()
                    .ok_or("No replay open")?
                    .seek(usize::try_from(index).map_err(|_| "Invalid replay position")?)?;
                self.touch();
                self.view()
            }
            "replay_exit" => {
                self.replay = None;
                self.touch();
                self.view()
            }
            "view" => self.view(),
            "new" => {
                let config: Config =
                    serde_json::from_value(request["config"].clone()).map_err(|e| e.to_string())?;
                let game = create(&config)?;
                self.game = game;
                self.config = config;
                self.history.clear();
                self.paused = false;
                self.touch();
                self.view()
            }
            "preview" => {
                let action = self.selected_choice(&request)?;
                self.game.preview(&action).map_err(|e| e.to_string())?;
                let mut after = self.game.clone();
                after.apply(action.clone()).map_err(|e| e.to_string())?;
                Ok(
                    json!({"revision":self.revision.to_string(),"choiceId":request["choiceId"],"label":choice(&self.game,&action,"")["label"],"lines":changes(&self.game,&after)}),
                )
            }
            "submit" => {
                let action = self.selected_choice(&request)?;
                self.commit(action)?;
                self.view()
            }
            "pause" => {
                self.paused = request["paused"].as_bool().ok_or("Missing pause state")?;
                self.generation += 1;
                self.view()
            }
            "undo" => {
                self.guard(&request)?;
                let mut game = self.history.pop().ok_or("No action to undo")?;
                while game.ai_controller(game.turn().player).is_some() && !self.history.is_empty() {
                    game = self.history.pop().unwrap();
                }
                self.game = game;
                self.paused = true;
                self.touch();
                self.view()
            }
            "save_local" => {
                let path = self
                    .storage
                    .clone()
                    .ok_or("Local save storage is not configured")?;
                let save = self.request(json!({"op":"save"}))?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let temp = path.with_extension("tmp");
                let bytes = serde_json::to_vec_pretty(&save).map_err(|e| e.to_string())?;
                use std::io::Write;
                let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
                file.write_all(&bytes).map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                std::fs::rename(&temp, &path).map_err(|e| e.to_string())?;
                Ok(json!({"saved":true}))
            }
            "load_local" => {
                let path = self
                    .storage
                    .as_ref()
                    .ok_or("Local save storage is not configured")?;
                let metadata = std::fs::metadata(path).map_err(|_| {
                    "No local save found. Save a match first or import a save file."
                })?;
                if metadata.len() > 8_000_000 {
                    return Err("Save is too large".into());
                }
                let data = std::fs::read(path).map_err(|e| e.to_string())?;
                let save: Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
                self.request(json!({"op":"load","save":save}))
            }
            "save" => {
                // RON keeps all u64 action IDs lossless inside the portable JSON envelope.
                Ok(
                    json!({"format":"cchess-replay-1","content":content_signature(&self.game),"config":self.config,
                    "actions":ron::to_string(&self.game.log().iter().map(|a| &a.intent).collect::<Vec<_>>()).map_err(|e|e.to_string())?}),
                )
            }
            "load" => {
                let save = &request["save"];
                if save["format"] != "cchess-replay-1" {
                    return Err("Unsupported save format".into());
                }
                let config: Config =
                    serde_json::from_value(save["config"].clone()).map_err(|e| e.to_string())?;
                let mut game = create(&config)?;
                if save["content"] != content_signature(&game) {
                    return Err("Save uses different game content".into());
                }
                let actions: Vec<ActionIntent> =
                    ron::from_str(save["actions"].as_str().ok_or("Missing saved actions")?)
                        .map_err(|e| e.to_string())?;
                if actions.len() > 100_000 {
                    return Err("Save exceeds action limit".into());
                }
                for action in actions {
                    game.apply(action)
                        .map_err(|e| format!("Invalid saved action: {e:?}"))?;
                }
                self.game = game;
                self.config = config;
                self.history.clear();
                self.paused = true;
                self.touch();
                self.view()
            }
            _ => Err("Unknown operation".into()),
        }
    }
    fn view(&mut self) -> Result<Value, String> {
        if let Some(replay) = &self.replay {
            return replay.view(&self.config, self.revision);
        }
        self.choices.clear();
        if !self.game.is_over() {
            for p in self
                .game
                .pieces()
                .filter(|p| p.owner == self.game.turn().player)
            {
                self.choices.extend(self.game.legal_actions(p.id));
            }
            for id in &self
                .game
                .player(self.game.turn().player)
                .unwrap()
                .cards
                .hand
            {
                self.choices.extend(self.game.legal_card_plays(*id));
            }
            let pass = ActionIntent::EndTurn {
                player: self.game.turn().player,
            };
            if self.game.validate(&pass).is_ok() {
                self.choices.push(pass);
            }
        }
        project(
            &self.game,
            &self.config,
            self.revision,
            &self.choices,
            self.paused,
            !self.history.is_empty(),
        )
    }
}
fn project(
    g: &Game,
    config: &Config,
    revision: u64,
    actions: &[ActionIntent],
    paused: bool,
    can_undo: bool,
) -> Result<Value, String> {
    let choices: Vec<_> = actions
        .iter()
        .enumerate()
        .map(|(i, a)| choice(g, a, &format!("{}:{i}", revision)))
        .collect();
    let manifest: Value = serde_json::from_str(include_str!("../../assets/gui/manifest.json"))
        .map_err(|e| e.to_string())?;
    let pieces: Vec<_> = g.pieces().map(|p| {
            let def = &g.registry().get(&p.archetype).unwrap().definition;
            let abilities: Vec<_> = def.abilities.iter().map(|a| {
                let available = choices.iter().any(|c|c["source"] == p.id.to_string() && c["ability"] == a.id);
                let cost = g.effective_ability_cost(p.id,a.cost);
                json!({"id":a.id,"name":a.display_name,"description":format!("{} Cooldown after use: {} owner turns.", describe_effects(&a.effects), a.cooldown),
                    "actions":cost.actions,"mana":cost.mana,"cooldown":p.state.cooldowns.get(&a.id).map(|c|c.remaining).unwrap_or(0),
                    "available":available,"reason":if available {String::new()} else {g.ability_availability(p.id,&a.id).err().map(|e|e.to_string()).unwrap_or_default()}})
            }).collect();
            json!({"id":p.id.to_string(),"archetype":p.archetype,"label":manifest["pieces"][&p.archetype]["label"].as_str().unwrap_or("?"),
                "owner":p.owner,"position":p.position,"damage":p.state.damage,"tough":p.stats.tough,"acted":p.state.acted,
                "tags":g.effective_tags(p).iter().map(|t|format!("{t:?}")).collect::<Vec<_>>(),
                "statuses":p.statuses.values().map(|s|json!({"kind":format!("{:?}",s.kind),"remaining":s.remaining})).collect::<Vec<_>>(),"abilities":abilities})
        }).collect();
    let human = g.ai_controller(g.turn().player).is_none();
    let hand: Vec<_> = if human {
        g.player(g.turn().player)
            .unwrap()
            .cards
            .hand
            .iter()
            .map(|id| card(g, *id, &choices))
            .collect()
    } else {
        vec![]
    };
    let onboard: Vec<_> = g
        .card_instances()
        .filter(|c| c.zone == CardZone::OnBoard)
        .map(|c| card(g, c.id, &choices))
        .collect();
    Ok(
        json!({"sessionId":"local","revision":revision.to_string(),"preset":config.preset,"config":config,
            "width":g.bounds().width,"height":g.bounds().height,
            "cells":g.bounds().positions().map(|p|json!({"position":p,"playable":g.is_playable(p),"magic":g.is_magic_tile(p),"buildable":g.is_buildable_tile(p)})).collect::<Vec<_>>(),
            "pieces":pieces,"players":g.players().iter().map(|p|json!({"id":p.id,"name":format!("Player {}",p.id),"mana":p.mana.to_string(),
                "actions":if p.id==g.turn().player {g.turn().remaining_actions}else{0},"ai":g.ai_controller(p.id).map(|a|format!("{:?}",a.policy)),"eliminated":g.is_eliminated(p.id)})).collect::<Vec<_>>(),
            "currentPlayer":g.turn().player,"turn":g.turn().number.to_string(),"hand":hand,"onboard":onboard,
            "choices":if human {choices}else{vec![]},"aiPaused":paused,"canUndo":can_undo,
            "promotion":g.pending_promotion().is_some(),"winner":g.winner(),"draw":g.is_draw(),"over":g.is_over(),
            "scheduled":g.scheduled().iter().map(|s|json!({"id":s.id.to_string(),"text":format!("Event {} · P{} · {} {:?} remaining · {}",s.id,s.owner,s.due_tick.saturating_sub(g.clock_value(s.owner,s.clock)),s.clock,s.source_archetype.as_deref().unwrap_or("card"))})).collect::<Vec<_>>(),
            "log":g.log().iter().rev().take(100).map(|a|json!({"id":a.id.to_string(),"turn":"","text":choice(g,&a.intent,"")["label"]})).collect::<Vec<_>>() }),
    )
}
fn card(g: &Game, id: u64, choices: &[Value]) -> Value {
    let c = g.card_instance(id).unwrap();
    let d = &g.registry().card(&c.definition).unwrap().definition;
    let available = choices.iter().any(|a| a["card"] == id.to_string());
    json!({"id":id.to_string(),"name":d.display_name,"kind":format!("{:?}",d.kind).to_lowercase(),"cost":d.cost.mana,"actions":d.cost.actions,"owner":c.owner,
        "description":card_description(g,d),"available":available,"reason":if available{String::new()}else{g.card_availability(id).err().map(|e|e.to_string()).unwrap_or_default()}})
}
fn coord(p: Pos) -> String {
    let mut n = p.x + 1;
    let mut f = String::new();
    while n > 0 {
        n -= 1;
        f.insert(0, (b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    format!("{f}{}", p.y + 1)
}
fn choice(g: &Game, a: &ActionIntent, id: &str) -> Value {
    let mut v = json!({"id":id,"kind":"ability","label":""});
    let label = match a {
        ActionIntent::Piece {
            source,
            kind,
            target,
            ..
        } => {
            v["source"] = json!(source.to_string());
            v["position"] = json!(target);
            v["kind"] = json!(format!("{kind:?}").to_lowercase());
            format!(
                "{kind:?} {} → {}",
                g.piece(*source)
                    .and_then(|p| g.registry().get(&p.archetype))
                    .map(|p| p.definition.display_name.clone())
                    .unwrap_or_else(|| format!("piece {source}")),
                coord(*target)
            )
        }
        ActionIntent::Ability {
            source,
            ability,
            target,
            ..
        } => {
            v["source"] = json!(source.to_string());
            v["ability"] = json!(ability);
            v["position"] = json!(target);
            format!(
                "{}{}",
                ability,
                target
                    .map(|p| format!(" → {}", coord(p)))
                    .unwrap_or_default()
            )
        }
        ActionIntent::Transport {
            source,
            ability,
            target,
            destination,
            ..
        } => {
            v["source"] = json!(source.to_string());
            v["ability"] = json!(ability);
            v["passenger"] = json!(target);
            v["position"] = json!(destination);
            format!(
                "{ability}: passenger {} → {}",
                coord(*target),
                coord(*destination)
            )
        }
        ActionIntent::ScheduledAbility {
            source,
            ability,
            scheduled,
            ..
        } => {
            v["source"] = json!(source.to_string());
            v["ability"] = json!(ability);
            format!("{ability} → event {scheduled}")
        }
        ActionIntent::ChoosePromotion { archetype, .. } => {
            v["kind"] = json!("promotion");
            format!("Promote to {archetype}")
        }
        ActionIntent::EndTurn { player } => {
            v["kind"] = json!("end");
            format!("P{player}: end turn")
        }
        ActionIntent::PlayCard { card, target, .. } => {
            v["kind"] = json!("card");
            v["card"] = json!(card.to_string());
            let name = g
                .card_instance(*card)
                .and_then(|c| g.registry().card(&c.definition))
                .map(|c| c.definition.display_name.as_str())
                .unwrap_or("Card");
            let suffix = match target {
                CardChoice::None => String::new(),
                CardChoice::Square(p) => {
                    v["position"] = json!(p);
                    format!(" → {}", coord(*p))
                }
                CardChoice::Scheduled(id) => format!(" → event {id}"),
                CardChoice::Card(id) => format!(" → on-board card {id}"),
            };
            format!("{name}{suffix}")
        }
    };
    v["label"] = json!(label);
    v
}
fn changes(before: &Game, after: &Game) -> Vec<String> {
    let mut lines = vec![];
    for p in before.pieces() {
        match after.piece(p.id) {
            None => lines.push(format!("{} at {} removed", p.archetype, coord(p.position))),
            Some(q) => {
                if p.position != q.position {
                    lines.push(format!(
                        "{}: {} → {}",
                        p.archetype,
                        coord(p.position),
                        coord(q.position)
                    ));
                }
                if p.state.damage != q.state.damage {
                    lines.push(format!(
                        "{} damage: {} → {}",
                        q.archetype, p.state.damage, q.state.damage
                    ));
                }
                if p.archetype != q.archetype {
                    lines.push(format!("{} becomes {}", p.archetype, q.archetype));
                }
                if p.statuses != q.statuses {
                    lines.push(format!(
                        "{} statuses: {}",
                        q.archetype,
                        q.statuses
                            .values()
                            .map(|s| format!("{:?} {}", s.kind, s.remaining))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
    }
    for p in after.pieces().filter(|p| before.piece(p.id).is_none()) {
        lines.push(format!("Summon {} at {}", p.archetype, coord(p.position)));
    }
    for p in after.players() {
        let old = before.player(p.id).unwrap();
        if old.mana != p.mana {
            lines.push(format!("P{} mana: {} → {}", p.id, old.mana, p.mana));
        }
    }
    if before.turn() != after.turn() {
        lines.push(format!(
            "P{} · turn {} · {} actions remaining",
            after.turn().player,
            after.turn().number,
            after.turn().remaining_actions
        ));
    }
    if before.scheduled() != after.scheduled() {
        lines.push(format!(
            "Scheduled work updated: {} pending",
            after.scheduled().len()
        ));
    }
    if before.card_instances().collect::<Vec<_>>() != after.card_instances().collect::<Vec<_>>() {
        lines.push("Card zones or uses updated".into());
    }
    if let Some(p) = after.winner() {
        lines.push(format!("Player {p} wins"));
    } else if after.is_draw() {
        lines.push("Draw".into());
    }
    if after.pending_promotion().is_some() {
        lines.push("Promotion choice required".into());
    }
    if lines.is_empty() {
        lines.push("Action resolves successfully; cooldown/use state may change.".into());
    }
    lines
}
fn content_signature(g: &Game) -> String {
    // Stable explicit FNV-1a fingerprint; compatibility check, not a security hash.
    let content = format!(
        "rules-1:{:?}:{:?}:{}",
        g.registry()
            .iter()
            .map(|p| &p.definition)
            .collect::<Vec<_>>(),
        g.registry()
            .cards()
            .map(|c| &c.definition)
            .collect::<Vec<_>>(),
        include_str!("../../assets/decks/starter.ron")
    );
    let hash = content.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

pub fn handle(session: &Arc<Mutex<Session>>, request: Value) -> Result<Value, String> {
    if request["op"] != "ai" {
        return session
            .lock()
            .map_err(|_| "Session unavailable")?
            .request(request);
    }
    let (game, revision, generation) = {
        let s = session.lock().map_err(|_| "Session unavailable")?;
        s.guard(&request)?;
        if s.paused || s.game.is_over() || s.game.ai_controller(s.game.turn().player).is_none() {
            return Err("AI is paused or not active".into());
        }
        (s.game.clone(), s.revision, s.generation)
    };
    let action = game.choose_ai_action().ok_or("AI found no action")?;
    let mut s = session.lock().map_err(|_| "Session unavailable")?;
    finish_ai(&mut s, action, revision, generation)
}
fn finish_ai(
    s: &mut Session,
    action: ActionIntent,
    revision: u64,
    generation: u64,
) -> Result<Value, String> {
    if s.revision != revision || s.generation != generation || s.paused {
        return Err("AI result expired".into());
    }
    s.commit(action)?;
    s.view()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn local(preset: &str) -> Session {
        Session::new(Config {
            preset: preset.into(),
            seats: vec!["human".into(); if preset.starts_with("cross") { 4 } else { 2 }],
        })
        .unwrap()
    }
    fn matching(s: &mut Session, predicate: impl Fn(&Value) -> bool) -> Value {
        let v = s.view().unwrap();
        v["choices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| predicate(c))
            .unwrap()
            .clone()
    }
    fn submit(s: &mut Session, c: &Value) -> Value {
        s.request(json!({"op":"submit","revision":s.revision.to_string(),"choiceId":c["id"]}))
            .unwrap()
    }
    #[test]
    fn opening_moves_are_engine_moves_and_commit_advances_turn() {
        let mut s = local("classic");
        let knight = s.game.at(Pos::new(1, 0)).unwrap().id.to_string();
        let v = s.view().unwrap();
        let targets: Vec<_> = v["choices"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["source"] == knight && c["kind"] == "move")
            .map(|c| c["position"].clone())
            .collect();
        assert!(targets.contains(&json!({"x":0,"y":2})));
        assert!(targets.contains(&json!({"x":2,"y":2})));
        let pawn = s.game.at(Pos::new(4, 1)).unwrap().id.to_string();
        let c = matching(&mut s, |c| {
            c["source"] == pawn && c["position"] == json!({"x":4,"y":3})
        });
        let revision = s.revision.to_string();
        let before = s.game.snapshot();
        let preview = s
            .request(json!({"op":"preview","revision":revision,"choiceId":c["id"]}))
            .unwrap();
        assert!(!preview["lines"].as_array().unwrap().is_empty());
        assert_eq!(before, s.game.snapshot());
        let view = submit(&mut s, &c);
        assert_eq!(view["currentPlayer"], 1);
        assert!(s.game.at(Pos::new(4, 3)).is_some());
        assert!(s
            .request(json!({"op":"submit","revision":revision,"choiceId":c["id"]}))
            .is_err());
    }
    #[test]
    fn presets_are_real_armies_and_card_costs_are_content() {
        assert_eq!(local("expansion").game.pieces().count(), 92);
        assert_eq!(local("cross24").game.pieces().count(), 184);
        let mut s = local("cards");
        let v = s.view().unwrap();
        let cards = v["hand"].as_array().unwrap();
        let march = cards.iter().find(|c| c["name"] == "Forced March").unwrap();
        assert_eq!(march["cost"], 1);
        let c = matching(&mut s, |c| c["card"] == march["id"]);
        assert!(c.get("position").is_none());
        let view = submit(&mut s, &c);
        assert_eq!(view["players"][0]["actions"], 3);
        assert_eq!(view["players"][0]["mana"], "0");
    }
    #[test]
    fn save_load_replays_exactly_and_invalid_load_preserves_position() {
        let mut s = local("classic");
        for _ in 0..5 {
            let c = matching(&mut s, |c| c["kind"] == "end");
            submit(&mut s, &c);
        }
        let snapshot = s.game.snapshot();
        let save = s.request(json!({"op":"save"})).unwrap();
        let mut restored = local("cards");
        restored.request(json!({"op":"load","save":save})).unwrap();
        assert_eq!(snapshot, restored.game.snapshot());
        let mut invalid = save.clone();
        invalid["actions"] = json!("invalid");
        assert!(restored
            .request(json!({"op":"load","save":invalid}))
            .is_err());
        assert_eq!(snapshot, restored.game.snapshot());
    }
    #[test]
    fn ai_and_undo_use_authoritative_history() {
        let shared = Arc::new(Mutex::new(Session::new(Config::default()).unwrap()));
        let before = shared.lock().unwrap().game.snapshot();
        let c = matching(&mut shared.lock().unwrap(), |c| c["kind"] == "move");
        submit(&mut shared.lock().unwrap(), &c);
        let revision = shared.lock().unwrap().revision.to_string();
        handle(&shared, json!({"op":"ai","revision":revision})).unwrap();
        let mut s = shared.lock().unwrap();
        let revision = s.revision.to_string();
        s.request(json!({"op":"undo","revision":revision})).unwrap();
        assert_eq!(before, s.game.snapshot());
        assert!(s.paused);
    }
    #[test]
    fn unavailable_and_stale_choices_are_rejected_atomically() {
        let mut s = local("classic");
        s.view().unwrap();
        let before = s.game.snapshot();
        assert!(s
            .request(json!({"op":"submit","revision":"1","choiceId":"1:999999"}))
            .is_err());
        assert_eq!(before, s.game.snapshot());
        let c = matching(&mut s, |c| c["kind"] == "move");
        s.request(json!({"op":"new","config":{"preset":"classic","seats":["human","human"]}}))
            .unwrap();
        assert!(s
            .request(json!({"op":"submit","revision":s.revision.to_string(),"choiceId":c["id"]}))
            .is_err());
    }
    #[test]
    fn promotion_choices_can_be_previewed_committed_and_restored() {
        let mut s = local("abilities");
        let pawn = s.game.at(Pos::new(1, 6)).unwrap().id.to_string();
        let c = matching(&mut s, |c| {
            c["source"] == pawn && c["position"] == json!({"x":1,"y":7})
        });
        let v = submit(&mut s, &c);
        assert_eq!(v["promotion"], true);
        let save = s.request(json!({"op":"save"})).unwrap();
        let c = matching(&mut s, |c| {
            c["kind"] == "promotion" && c["label"] == "Promote to queen"
        });
        submit(&mut s, &c);
        assert!(s.game.pending_promotion().is_none());
        s.request(json!({"op":"load","save":save})).unwrap();
        assert!(s.game.pending_promotion().is_some());
    }
    #[test]
    fn paused_and_resumed_ai_discards_its_old_search_result() {
        let mut s = Session::new(Config::default()).unwrap();
        let c = matching(&mut s, |c| c["kind"] == "end");
        submit(&mut s, &c);
        let action = s.game.choose_ai_action().unwrap();
        let revision = s.revision;
        let generation = s.generation;
        s.request(json!({"op":"pause","paused":true})).unwrap();
        s.request(json!({"op":"pause","paused":false})).unwrap();
        let before = s.game.snapshot();
        assert!(finish_ai(&mut s, action, revision, generation).is_err());
        assert_eq!(before, s.game.snapshot());
    }
    #[test]
    fn take_damages_a_tough_defender_without_removing_it_or_relocating_attacker() {
        use cchess_core::{
            state::{Placement, Player},
            Bounds, Direction,
        };
        let mut s = local("classic");
        s.game = Game::new(
            Registry::built_in().unwrap(),
            Bounds {
                width: 8,
                height: 8,
            },
            vec![Player::new(0, Direction::N), Player::new(1, Direction::S)],
            vec![
                Placement::new("king", 0, Pos::new(4, 0)),
                Placement::new("king", 1, Pos::new(4, 7)),
                Placement::new("rook", 0, Pos::new(0, 0)),
                Placement::new("dragon", 1, Pos::new(0, 1)),
            ],
        )
        .unwrap();
        let c = matching(&mut s, |c| {
            c["kind"] == "take" && c["position"] == json!({"x":0,"y":1})
        });
        submit(&mut s, &c);
        assert_eq!(s.game.at(Pos::new(0, 0)).unwrap().archetype, "rook");
        assert_eq!(s.game.at(Pos::new(0, 1)).unwrap().state.damage, 1);
        assert_eq!(s.game.turn().player, 1);
    }
    #[test]
    fn availability_and_descriptions_are_derived_from_rules() {
        let mut s = local("classic");
        let v = s.view().unwrap();
        let king = v["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["owner"] == 0 && p["archetype"] == "king")
            .unwrap();
        assert!(king["abilities"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("requires 2 mana"));
        let mut s = local("cards");
        let v = s.view().unwrap();
        let march = v["hand"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "Forced March")
            .unwrap();
        assert_eq!(march["description"], "Gain 2 actions this turn");
    }
    #[test]
    fn local_save_survives_a_new_session_and_corrupt_load_is_atomic() {
        let directory = std::env::temp_dir().join(format!(
            "cchess-save-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("match.json");
        let mut s = local("classic").with_storage(path.clone());
        let c = matching(&mut s, |c| c["kind"] == "move");
        submit(&mut s, &c);
        let expected = s.game.snapshot();
        s.request(json!({"op":"save_local"})).unwrap();
        let mut restored = local("cards").with_storage(path.clone());
        restored.request(json!({"op":"load_local"})).unwrap();
        assert_eq!(expected, restored.game.snapshot());
        std::fs::write(&path, b"invalid").unwrap();
        assert!(restored.request(json!({"op":"load_local"})).is_err());
        assert_eq!(expected, restored.game.snapshot());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn all_initial_demo_choices_project_and_preview_without_mutation() {
        for preset in [
            "classic",
            "abilities",
            "cards",
            "siege",
            "events",
            "catalogue",
            "cross14",
            "cross24",
        ] {
            let mut s = local(preset);
            let v = s.view().unwrap();
            let before = s.game.snapshot();
            for c in v["choices"].as_array().unwrap().iter().take(100) {
                s.request(
                    json!({"op":"preview","revision":s.revision.to_string(),"choiceId":c["id"]}),
                )
                .unwrap();
            }
            assert_eq!(before, s.game.snapshot());
        }
    }
}

fn describe_effects(effects: &[cchess_core::content::EffectDef]) -> String {
    use cchess_core::content::EffectDef::*;
    effects
        .iter()
        .map(|effect| match effect {
            MoveSource => "Move to the selected square".into(),
            RemoveSource => "Remove the caster".into(),
            DamageTarget(n) => format!("Deal {n} damage"),
            MoveSourceIfTargetRemoved => "Move only if the defender is removed".into(),
            AddStatus { kind, duration } => format!("Apply {kind:?} for {duration} owner turns"),
            RemoveStatuses => "Cleanse statuses".into(),
            SpawnRelative { archetype, .. }
            | SpawnOriented { archetype, .. }
            | SpawnTarget(archetype) => format!("Summon {}", archetype.replace('_', " ")),
            AddMana(n) => format!("Gain {n} mana"),
            Sequence(e) => describe_effects(e),
            ForEach { effects, .. } => {
                format!("For each eligible target: {}", describe_effects(effects))
            }
            Schedule { after, effects } => {
                format!("After {after} owner turns: {}", describe_effects(effects))
            }
            ScheduleOn {
                clock,
                after,
                effects,
            } => format!("After {after} {clock:?}: {}", describe_effects(effects)),
            TransformSource(a) => format!("Transform into {}", a.replace('_', " ")),
            ModifyStat { stat, amount } => format!("Modify {stat:?} by {amount}"),
            Traverse(_) => "Traverse the selected ray; preview damage and collisions".into(),
            SeekTag(t) => format!("Seek a {t:?} piece along the selected direction"),
            AdvancePeers { halfside } => format!("Advance eligible peers in radius {halfside}"),
            ReviveLatest { undead } => format!(
                "Revive latest friendly death{}",
                if *undead { " as Undead" } else { "" }
            ),
            Transport {
                max_range,
                collision_damage,
            } => {
                format!("Transport up to {max_range} squares; collision damage {collision_damage}")
            }
            Push {
                distance,
                collision_damage,
            } => format!("Push {distance} squares; collision damage {collision_damage}"),
            DrawCard(n) => format!("Draw {n} cards"),
            ShuffleDeck => "Shuffle deck".into(),
            ModifyActions(n) => format!("Gain {n} actions this turn"),
            IncreaseActionsPerTurn(n) => format!("Gain {n} actions per turn"),
            ResetAction => "Reset acted state".into(),
            ModifyScheduledEvent(n) => format!("Advance scheduled work by {n} ticks"),
            CancelScheduledEvent => "Cancel scheduled work".into(),
            RemoveBoardCard => "Remove the selected on-board card".into(),
        })
        .collect::<Vec<String>>()
        .join(". ")
}
fn card_description(g: &Game, d: &cchess_core::cards::CardDef) -> String {
    let mut parts = vec![describe_effects(&d.effects)];
    for trigger in &d.triggers {
        parts.push(format!(
            "On {:?}: {}",
            trigger.event,
            describe_effects(&trigger.effects)
        ));
    }
    for discount in &d.ability_discounts {
        parts.push(format!(
            "Friendly {:?} abilities cost {} less mana",
            discount.source_tag, discount.mana
        ));
    }
    // Persistent unlock conditions are part of the piece definitions, not a second card catalogue.
    for p in g.registry().iter() {
        for a in &p.definition.abilities {
            if a.consume_cards.contains(&d.id)
                || a.conditions.iter().any(
                    |c| matches!(c,cchess_core::content::Condition::CardOnBoard(id) if id==&d.id),
                )
            {
                parts.push(format!(
                    "Unlocks {}: {}",
                    p.definition.display_name, a.display_name
                ));
            }
        }
    }
    parts
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(". ")
}
