//! Keyboard-first terminal client. Rules and previews remain in the engine.
use cchess_ai::GameAi;
use cchess_core::{cards::CardChoice, state::EntityId, ActionIntent, ActionKind, Event, Game, Pos};
use crossterm::{
    cursor,
    event::{self, Event as Input, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    collections::BTreeSet,
    io::{self, Write},
    time::{Duration, Instant},
};

const BG: Color = Color::Rgb {
    r: 16,
    g: 22,
    b: 32,
};
const TEXT: Color = Color::Rgb {
    r: 221,
    g: 229,
    b: 239,
};
const MUTED: Color = Color::Rgb {
    r: 131,
    g: 151,
    b: 174,
};
const ACCENT: Color = Color::Rgb {
    r: 84,
    g: 218,
    b: 192,
};
fn owner_color(owner: u8) -> Color {
    match owner % 4 {
        0 => Color::Rgb {
            r: 246,
            g: 221,
            b: 155,
        },
        1 => Color::Rgb {
            r: 118,
            g: 182,
            b: 255,
        },
        2 => Color::Rgb {
            r: 217,
            g: 148,
            b: 255,
        },
        _ => Color::Rgb {
            r: 255,
            g: 139,
            b: 143,
        },
    }
}
fn label(pos: Pos) -> String {
    let mut x = pos.x + 1;
    let mut letters = Vec::new();
    while x > 0 {
        x -= 1;
        letters.push((b'a' + (x % 26) as u8) as char);
        x /= 26;
    }
    format!("{}{}", letters.iter().rev().collect::<String>(), pos.y + 1)
}
fn glyph(name: &str) -> &str {
    match name {
        "pawn" => "Pa",
        "rook" => "Ro",
        "knight" => "Kn",
        "bishop" => "Bi",
        "queen" => "Qu",
        "king" => "Ki",
        "super_pawn" => "SP",
        "crazy_pawn" => "CP",
        "shield_bearer" => "SB",
        "builder" => "Bu",
        "wall" => "Wa",
        "cannon" => "Ca",
        "catapult" => "Ct",
        "ship" => "Sh",
        "tesla_tower" => "Te",
        "archer" => "Ar",
        "ballista" => "Ba",
        "ram" => "Ra",
        "warlock" => "Wl",
        "paladin" => "Pl",
        "magician" => "Mg",
        "portal" => "Po",
        "basilisk" => "Bs",
        "dragon" => "Dr",
        "gargoyle" => "Ga",
        "golem" => "Go",
        "imp" => "Im",
        "mandragora" => "Ma",
        "mermaid" => "Me",
        "necromancer" => "Ne",
        "ogre" => "Og",
        "oni" => "On",
        "spider" => "Sp",
        "spider_egg" => "Eg",
        "succubus" => "Su",
        "witch" => "Wi",
        _ => "??",
    }
}
#[derive(Clone)]
struct Row {
    label: String,
    actions: Vec<ActionIntent>,
}
#[derive(Clone)]
enum Mode {
    Board,
    Menu {
        title: String,
        rows: Vec<Row>,
        index: usize,
    },
    Target {
        title: String,
        actions: Vec<ActionIntent>,
        destination: bool,
    },
    Confirm {
        intent: ActionIntent,
        lines: Vec<String>,
    },
    Help,
    Info {
        title: String,
        lines: Vec<String>,
    },
}
/// Public client model permits deterministic keyboard tests without a terminal.
pub struct Ui {
    pub game: Game,
    pub cursor: Pos,
    selected: Option<EntityId>,
    moves: Vec<ActionIntent>,
    mode: Mode,
    history: Vec<Game>,
    notice: String,
    recent: Vec<String>,
    pub ai_paused: bool,
    ai_steps: usize,
    scroll: usize,
}
impl Ui {
    pub fn new(game: Game) -> Self {
        let cursor = game
            .pieces()
            .find(|p| p.owner == game.turn().player)
            .map(|p| p.position)
            .unwrap_or(Pos::new(0, 0));
        let mut ui = Self {
            game,
            cursor,
            selected: None,
            moves: vec![],
            mode: Mode::Board,
            history: vec![],
            notice: "Choose a piece with Enter. A: abilities  C: cards  ?: help".into(),
            recent: vec![],
            ai_paused: false,
            ai_steps: 0,
            scroll: 0,
        };
        ui.promotion();
        ui
    }
    fn human_turn(&self) -> bool {
        !self.game.is_over() && self.game.ai_controller(self.game.turn().player).is_none()
    }
    fn promotion(&mut self) {
        if self.human_turn() {
            if let Some(p) = self.game.pending_promotion() {
                let rows = self
                    .game
                    .legal_actions(p.piece)
                    .into_iter()
                    .map(|action| Row {
                        label: action_label(&self.game, &action),
                        actions: vec![action],
                    })
                    .collect();
                self.mode = Mode::Menu {
                    title: "PROMOTION · choose your new piece".into(),
                    rows,
                    index: 0,
                };
            }
        }
    }
    fn reset_selection(&mut self) {
        self.scroll = 0;
        self.selected = None;
        self.moves.clear();
        self.mode = Mode::Board;
    }
    fn apply(&mut self, intent: ActionIntent) {
        if !self.human_turn() {
            self.notice = "AI controls this seat. Space pauses; U undoes.".into();
            return;
        }
        let before = self.game.clone();
        let description = action_label(&self.game, &intent);
        match self.game.apply(intent) {
            Ok(_) => {
                if self.history.len() == 100 {
                    self.history.remove(0);
                }
                self.history.push(before);
                self.recent.push(description.clone());
                self.notice = description;
                self.reset_selection();
                self.ai_steps = 0;
                self.promotion();
            }
            Err(e) => self.notice = e.to_string(),
        }
    }
    pub fn ai_step(&mut self) {
        if self.ai_paused
            || self.game.is_over()
            || self.game.ai_controller(self.game.turn().player).is_none()
        {
            return;
        }
        if self.ai_steps >= 256 {
            self.ai_paused = true;
            self.notice = "AI paused after 256 actions. Space resumes.".into();
            return;
        }
        let Some(intent) = self.game.choose_ai_action() else {
            return;
        };
        let description = format!(
            "P{} · {}",
            self.game.turn().player,
            action_label(&self.game, &intent)
        );
        match self.game.apply(intent) {
            Ok(_) => {
                self.recent.push(description.clone());
                self.notice = description;
                self.ai_steps += 1;
                self.reset_selection();
                self.promotion();
            }
            Err(e) => {
                self.notice = format!("AI paused: {e}");
                self.ai_paused = true;
            }
        }
    }
    fn choose_piece(&mut self) {
        if let Some(piece) = self
            .game
            .at(self.cursor)
            .filter(|p| p.owner == self.game.turn().player)
        {
            self.selected = Some(piece.id);
            self.moves = self
                .game
                .legal_actions(piece.id)
                .into_iter()
                .filter(|a| matches!(a, ActionIntent::Piece { .. }))
                .collect();
            self.notice = format!(
                "{} at {} · Enter a highlighted square, or A for abilities",
                piece.archetype.replace('_', " "),
                label(piece.position)
            );
        } else {
            self.notice = "Select a piece belonging to the current player.".into();
        }
    }
    fn abilities(&mut self) {
        let source = self
            .selected
            .or_else(|| self.game.at(self.cursor).map(|p| p.id));
        let Some(piece) = source.and_then(|id| self.game.piece(id)) else {
            self.notice = "Choose a piece first.".into();
            return;
        };
        if piece.owner != self.game.turn().player {
            self.notice = "That piece belongs to another player.".into();
            return;
        }
        let legal = self.game.legal_abilities(piece.id);
        let rows = self
            .game
            .registry()
            .get(&piece.archetype)
            .unwrap()
            .abilities
            .iter()
            .map(|ability| {
                let def = &ability.definition;
                let actions = legal
                    .iter()
                    .filter(|a| match a {
                        ActionIntent::Ability { ability, .. }
                        | ActionIntent::Transport { ability, .. }
                        | ActionIntent::ScheduledAbility { ability, .. } => ability == &def.id,
                        _ => false,
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let cost = self.game.effective_ability_cost(piece.id, def.cost);
                let state = if let Some(cd) = piece.state.cooldowns.get(&def.id) {
                    format!("CD {}", cd.remaining)
                } else if piece.state.used_abilities.contains(&def.id) {
                    "used".into()
                } else if actions.is_empty() {
                    "unavailable".into()
                } else {
                    "ready".into()
                };
                Row {
                    label: format!(
                        "{} · {} mana / {} act · {state}",
                        def.display_name, cost.mana, cost.actions
                    ),
                    actions,
                }
            })
            .collect();
        self.mode = Mode::Menu {
            title: "ABILITIES".into(),
            rows,
            index: 0,
        };
    }
    fn cards(&mut self) {
        let rows = self
            .game
            .player(self.game.turn().player)
            .unwrap()
            .cards
            .hand
            .iter()
            .map(|id| {
                let card = self.game.card_instance(*id).unwrap();
                let def = &self
                    .game
                    .registry()
                    .card(&card.definition)
                    .unwrap()
                    .definition;
                let actions = self.game.legal_card_plays(*id);
                Row {
                    label: format!(
                        "{} · {} mana / {} act{}",
                        def.display_name,
                        def.cost.mana,
                        def.cost.actions,
                        if actions.is_empty() {
                            " · unavailable"
                        } else {
                            ""
                        }
                    ),
                    actions,
                }
            })
            .collect();
        self.mode = Mode::Menu {
            title: "HAND · choose a card".into(),
            rows,
            index: 0,
        };
    }
    fn begin(&mut self, title: String, actions: Vec<ActionIntent>) {
        if actions.is_empty() {
            self.notice =
                "Unavailable: check mana, cooldown, acted state, protection and legal targets."
                    .into();
            return;
        }
        if actions
            .iter()
            .any(|a| target(&self.game, a, false).is_some())
        {
            self.mode = Mode::Target {
                title,
                actions,
                destination: false,
            };
            self.notice =
                "Choose a highlighted target with arrows + Enter. Tab cycles targets.".into();
        } else if actions.len() == 1 {
            self.confirm(actions[0].clone());
        } else {
            self.mode = Mode::Menu {
                title,
                rows: actions
                    .into_iter()
                    .map(|a| Row {
                        label: action_label(&self.game, &a),
                        actions: vec![a],
                    })
                    .collect(),
                index: 0,
            };
        }
    }
    fn confirm(&mut self, intent: ActionIntent) {
        self.scroll = 0;
        match self.game.preview(&intent) {
            Ok(trace) => {
                let mut lines = vec![action_label(&self.game, &intent)];
                for event in trace.events {
                    match event {
                        Event::ActionsPerTurnModified { before, after, .. } => lines.push(format!(
                            "Movements per turn: {before} → {after} (permanent)"
                        )),
                        Event::CostsPaid { mana, actions, .. } => {
                            lines.push(format!("Cost: {mana} mana · {actions} action(s)"))
                        }
                        Event::PieceMoved { from, to, .. } => {
                            lines.push(format!("Move {} → {}", label(from), label(to)))
                        }
                        Event::DamageDealt { amount, target, .. } => lines.push(format!(
                            "{amount} damage to {}",
                            self.game
                                .piece(target)
                                .map(|p| format!("{} {}", p.archetype, label(p.position)))
                                .unwrap_or_else(|| format!("piece {target}"))
                        )),
                        Event::PieceSpawned {
                            archetype,
                            position,
                            ..
                        }
                        | Event::PieceRevived {
                            archetype,
                            position,
                            ..
                        } => lines.push(format!("{archetype} at {}", label(position))),
                        Event::StatusApplied { kind, duration, .. } => {
                            lines.push(format!("{kind:?} for {duration}"))
                        }
                        Event::ScheduledChanged {
                            id, before, after, ..
                        } => lines.push(format!("Event {id}: {before} → {after}")),
                        Event::GameWon { player } => lines.push(format!("Player {player} wins")),
                        _ => {}
                    }
                }
                self.mode = Mode::Confirm { intent, lines };
            }
            Err(e) => self.notice = e.to_string(),
        }
    }
    fn targets(&self) -> BTreeSet<Pos> {
        match &self.mode {
            Mode::Target {
                actions,
                destination,
                ..
            } => actions
                .iter()
                .filter_map(|a| target(&self.game, a, *destination))
                .collect(),
            Mode::Board => self
                .moves
                .iter()
                .filter_map(|a| target(&self.game, a, false))
                .collect(),
            _ => BTreeSet::new(),
        }
    }
    /// Returns false when the user requests exit.
    pub fn key(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return true;
        }
        if matches!(key.code, KeyCode::Char('q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return false;
        }
        if key.code == KeyCode::Char('u') {
            if let Some(game) = self.history.pop() {
                self.game = game;
                self.reset_selection();
                self.recent.clear();
                self.ai_steps = 0;
                self.notice = "Undone, including AI responses.".into();
                self.promotion();
            } else {
                self.notice = "Nothing to undo.".into();
            }
            return true;
        }
        if key.code == KeyCode::Char(' ') {
            self.ai_paused = !self.ai_paused;
            self.ai_steps = 0;
            return true;
        }
        if key.code == KeyCode::Esc {
            self.reset_selection();
            self.promotion();
            return true;
        }
        if key.code == KeyCode::Char('i') {
            self.scroll = 0;
            self.mode = Mode::Info {
                title: format!("INSPECT {}", label(self.cursor)),
                lines: self.inspection(),
            };
            return true;
        }
        if key.code == KeyCode::Char('t') {
            self.scroll = 0;
            let mut lines = self
                .game
                .scheduled()
                .iter()
                .map(|s| {
                    format!(
                        "Event {} · P{} · {:?} tick {}",
                        s.id, s.owner, s.clock, s.due_tick
                    )
                })
                .collect::<Vec<_>>();
            if lines.is_empty() {
                lines.push("No pending events".into());
            }
            self.mode = Mode::Info {
                title: "SCHEDULED EVENTS".into(),
                lines,
            };
            return true;
        }
        if key.code == KeyCode::Char('?') {
            self.scroll = 0;
            self.mode = Mode::Help;
            return true;
        }
        if matches!(
            self.mode,
            Mode::Help | Mode::Info { .. } | Mode::Confirm { .. }
        ) {
            let count = match &self.mode {
                Mode::Confirm { lines, .. } | Mode::Info { lines, .. } => lines.len(),
                _ => 10,
            };
            match key.code {
                KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::Down => self.scroll = (self.scroll + 1).min(count.saturating_sub(1)),
                _ => {}
            }
        }
        if matches!(self.mode, Mode::Help | Mode::Info { .. }) {
            if key.code == KeyCode::Enter {
                self.mode = Mode::Board;
            }
            return true;
        }
        if let Mode::Confirm { intent, .. } = &self.mode {
            if key.code == KeyCode::Enter {
                let intent = intent.clone();
                self.apply(intent);
            }
            return true;
        }
        if let Mode::Menu { title, rows, index } = &mut self.mode {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => *index = index.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    *index = (*index + 1).min(rows.len().saturating_sub(1))
                }
                KeyCode::Enter => {
                    if let Some(row) = rows.get(*index).cloned() {
                        let title = title.clone();
                        self.begin(title, row.actions);
                    }
                }
                _ => {}
            }
            return true;
        }
        let delta = match key.code {
            KeyCode::Left | KeyCode::Char('h') => (-1, 0),
            KeyCode::Right | KeyCode::Char('l') => (1, 0),
            KeyCode::Up | KeyCode::Char('k') => (0, 1),
            KeyCode::Down | KeyCode::Char('j') => (0, -1),
            _ => (0, 0),
        };
        if delta != (0, 0) {
            if let Some(p) = self
                .cursor
                .offset(delta.0, delta.1)
                .filter(|p| self.game.is_playable(*p))
            {
                self.cursor = p;
            }
            return true;
        }
        if key.code == KeyCode::Tab {
            let targets = self.targets();
            if let Some(p) = targets
                .iter()
                .find(|p| **p > self.cursor)
                .or_else(|| targets.first())
            {
                self.cursor = *p;
            }
            return true;
        }
        if !self.human_turn() {
            return true;
        }
        if let Mode::Target {
            title,
            actions,
            destination,
        } = &self.mode
        {
            if key.code == KeyCode::Enter {
                let choices: Vec<_> = actions
                    .iter()
                    .filter(|a| target(&self.game, a, *destination) == Some(self.cursor))
                    .cloned()
                    .collect();
                if !*destination
                    && choices
                        .iter()
                        .any(|a| matches!(a, ActionIntent::Transport { .. }))
                {
                    self.mode = Mode::Target {
                        title: format!("{title} · choose landing direction / distance"),
                        actions: choices,
                        destination: true,
                    };
                } else if choices.len() == 1 {
                    self.confirm(choices[0].clone());
                } else if !choices.is_empty() {
                    self.mode = Mode::Menu {
                        title: title.clone(),
                        rows: choices
                            .into_iter()
                            .map(|a| Row {
                                label: action_label(&self.game, &a),
                                actions: vec![a],
                            })
                            .collect(),
                        index: 0,
                    };
                } else {
                    self.notice = "Choose a highlighted square. Tab cycles legal targets.".into();
                }
            }
            return true;
        }
        match key.code {
            KeyCode::Char('a') => self.abilities(),
            KeyCode::Char('c') => self.cards(),
            KeyCode::Char('p') => self.confirm(ActionIntent::EndTurn {
                player: self.game.turn().player,
            }),
            KeyCode::Enter => {
                let choices: Vec<_> = self
                    .moves
                    .iter()
                    .filter(|a| target(&self.game, a, false) == Some(self.cursor))
                    .cloned()
                    .collect();
                if choices.len() == 1 {
                    self.apply(choices[0].clone());
                } else if choices.len() > 1 {
                    self.mode = Mode::Menu {
                        title: "CHOOSE ACTION".into(),
                        rows: choices
                            .into_iter()
                            .map(|a| Row {
                                label: action_label(&self.game, &a),
                                actions: vec![a],
                            })
                            .collect(),
                        index: 0,
                    };
                } else {
                    self.choose_piece();
                }
            }
            _ => {}
        }
        true
    }
}
fn target(game: &Game, action: &ActionIntent, destination: bool) -> Option<Pos> {
    match action {
        ActionIntent::Piece { target, .. } => Some(*target),
        ActionIntent::Ability { target, .. } => *target,
        ActionIntent::Transport {
            target,
            destination: to,
            ..
        } => Some(if destination { *to } else { *target }),
        ActionIntent::ScheduledAbility { scheduled, .. } => game
            .scheduled()
            .iter()
            .find(|s| s.id == *scheduled)
            .and_then(|s| s.cause.source.piece())
            .and_then(|id| game.piece(id))
            .map(|p| p.position),
        ActionIntent::PlayCard {
            target: CardChoice::Square(p),
            ..
        } => Some(*p),
        _ => None,
    }
}
fn action_label(game: &Game, action: &ActionIntent) -> String {
    match action {
        ActionIntent::Piece {
            source,
            kind,
            target,
            ..
        } => format!(
            "{kind:?} {} → {}",
            game.piece(*source)
                .map(|p| label(p.position))
                .unwrap_or_default(),
            label(*target)
        ),
        ActionIntent::Ability {
            ability, target, ..
        } => format!("{} {}", ability, target.map(label).unwrap_or_default()),
        ActionIntent::Transport {
            ability,
            target,
            destination,
            ..
        } => format!("{ability}: {} → {}", label(*target), label(*destination)),
        ActionIntent::ScheduledAbility {
            ability, scheduled, ..
        } => format!("{ability} · event {scheduled}"),
        ActionIntent::PlayCard { card, target, .. } => format!(
            "{} · {}",
            game.card_instance(*card)
                .map(|c| c.definition.replace('_', " "))
                .unwrap_or_default(),
            match target {
                CardChoice::None => "play".into(),
                CardChoice::Square(p) => label(*p),
                CardChoice::Scheduled(id) => format!("event {id}"),
                CardChoice::Card(id) => game
                    .card_instance(*id)
                    .map(|c| format!("{} (P{})", c.definition, c.owner))
                    .unwrap_or_else(|| format!("card {id}")),
            }
        ),
        ActionIntent::EndTurn { .. } => "End turn".into(),
        ActionIntent::ChoosePromotion { archetype, .. } => {
            format!("Promote to {}", archetype.replace('_', " "))
        }
    }
}

#[derive(Clone)]
struct Cell {
    ch: char,
    fg: Color,
    bg: Color,
}
struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}
impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![
                Cell {
                    ch: ' ',
                    fg: TEXT,
                    bg: BG
                };
                width * height
            ],
        }
    }
    fn text(&mut self, x: usize, y: usize, text: &str, fg: Color, bg: Color) {
        if y >= self.height {
            return;
        }
        for (i, ch) in text.chars().take(self.width.saturating_sub(x)).enumerate() {
            self.cells[y * self.width + x + i] = Cell { ch, fg, bg };
        }
    }
    fn plain(&self) -> String {
        self.cells
            .chunks(self.width.max(1))
            .map(|row| row.iter().map(|c| c.ch).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn draw(&self, out: &mut impl Write) -> io::Result<()> {
        for (y, row) in self.cells.chunks(self.width.max(1)).enumerate() {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            let mut colors = None;
            for cell in row {
                if colors != Some((cell.fg, cell.bg)) {
                    queue!(
                        out,
                        SetForegroundColor(cell.fg),
                        SetBackgroundColor(cell.bg)
                    )?;
                    colors = Some((cell.fg, cell.bg));
                }
                queue!(out, Print(cell.ch))?;
            }
        }
        queue!(out, ResetColor)?;
        out.flush()
    }
}
impl Ui {
    /// Plain screen snapshot, also useful for accessibility and layout tests.
    pub fn render_text(&self, width: u16, height: u16) -> String {
        self.screen(width as usize, height as usize).plain()
    }
    fn inspection(&self) -> Vec<String> {
        let mut info = vec![format!(
            "INSPECT {}{}",
            label(self.cursor),
            if self.game.is_magic_tile(self.cursor) {
                " · magic"
            } else if self.game.is_buildable_tile(self.cursor) {
                " · buildable"
            } else {
                ""
            }
        )];
        if let Some(piece) = self.game.at(self.cursor) {
            info.push(format!(
                "P{} {}",
                piece.owner,
                piece.archetype.replace('_', " ")
            ));
            info.push(format!(
                "Damage {}/{} · Strength {}",
                piece.state.damage, piece.stats.tough, piece.stats.strength
            ));
            info.push(if piece.state.acted {
                "Already acted".into()
            } else {
                "Ready to act".into()
            });
            for status in piece.statuses.values() {
                info.push(format!("{:?}: {}", status.kind, status.remaining));
            }
            for (id, cd) in &piece.state.cooldowns {
                info.push(format!("{id}: CD {}", cd.remaining));
            }
        } else {
            info.push("Empty square".into());
        }
        info
    }
    fn screen(&self, w: usize, h: usize) -> Canvas {
        let mut c = Canvas::new(w, h);
        if w < 40 || h < 16 {
            c.text(0, 0, "CCHESS - resize to at least 40 x 16", ACCENT, BG);
            c.text(0, 2, "Q: quit", TEXT, BG);
            return c;
        }
        let turn = self.game.turn();
        let state = if self.game.is_over() {
            format!("GAME OVER · winner {:?}", self.game.winner())
        } else if self.ai_paused {
            "AI PAUSED".into()
        } else if self.game.ai_controller(turn.player).is_some() {
            "AI THINKING".into()
        } else {
            "YOUR TURN".into()
        };
        c.text(
            1,
            0,
            &format!("CCHESS   Turn {} · P{}   {state}", turn.number, turn.player),
            ACCENT,
            BG,
        );
        let p = self.game.player(turn.player).unwrap();
        c.text(
            1,
            1,
            &format!(
                "Mana {}   Actions {}/{}   Hand {}   Deck {}   Events {}",
                p.mana,
                turn.remaining_actions,
                p.actions_per_turn,
                p.cards.hand.len(),
                p.cards.deck.len(),
                self.game.scheduled().len()
            ),
            TEXT,
            BG,
        );
        let seats = self
            .game
            .players()
            .iter()
            .map(|p| {
                format!(
                    "P{}: {} mana [{}]",
                    p.id,
                    p.mana,
                    if self.game.is_eliminated(p.id) {
                        "out"
                    } else if self.game.ai_controller(p.id).is_some() {
                        "AI"
                    } else {
                        "human"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("   ");
        c.text(1, 2, &seats, MUTED, BG);
        let side = if w >= 100 { 34 } else { 0 };
        let cols = ((w - side - 5) / 3)
            .min(self.game.bounds().width as usize)
            .max(1);
        let rows = (h - 10).min(self.game.bounds().height as usize).max(1);
        let ox = (self.cursor.x - cols as i32 / 2)
            .clamp(0, self.game.bounds().width as i32 - cols as i32);
        let oy = (self.cursor.y - rows as i32 / 2)
            .clamp(0, self.game.bounds().height as i32 - rows as i32);
        for x in 0..cols {
            let file = label(Pos::new(ox + x as i32, 0));
            c.text(4 + x * 3, 3, file.trim_end_matches('1'), MUTED, BG);
        }
        let targets = self.targets();
        for row in 0..rows {
            let y = oy + rows as i32 - 1 - row as i32;
            c.text(0, 4 + row, &format!("{:>2}", y + 1), MUTED, BG);
            for col in 0..cols {
                let pos = Pos::new(ox + col as i32, y);
                if !self.game.is_playable(pos) {
                    continue;
                }
                let piece = self.game.at(pos);
                let mut bg = if (pos.x + pos.y) % 2 == 0 {
                    Color::Rgb {
                        r: 37,
                        g: 48,
                        b: 63,
                    }
                } else {
                    Color::Rgb {
                        r: 53,
                        g: 65,
                        b: 80,
                    }
                };
                let mut marker = ' ';
                if targets.contains(&pos) {
                    bg = Color::DarkGreen;
                    marker = '+';
                    if self.moves.iter().any(|a| matches!(a,ActionIntent::Piece{kind:ActionKind::Take | ActionKind::Attack,target,..} if *target==pos)) { bg=Color::DarkRed;marker='x'; }
                }
                if piece.is_some_and(|p| Some(p.id) == self.selected) {
                    bg = Color::DarkYellow;
                    marker = '*';
                }
                if pos == self.cursor {
                    bg = Color::DarkCyan;
                    marker = '>';
                }
                let token =
                    piece
                        .map(|p| glyph(&p.archetype))
                        .unwrap_or(if self.game.is_magic_tile(pos) {
                            "~~"
                        } else if self.game.is_buildable_tile(pos) {
                            " ·"
                        } else {
                            "  "
                        });
                c.text(
                    4 + col * 3,
                    4 + row,
                    &format!("{token}{marker}"),
                    piece.map(|p| owner_color(p.owner)).unwrap_or(MUTED),
                    bg,
                );
            }
        }
        let info = self.inspection();
        if side > 0 {
            let x = w - side;
            for (i, line) in info.iter().take(h - 12).enumerate() {
                c.text(x, 3 + i, line, if i == 0 { ACCENT } else { TEXT }, BG);
            }
            c.text(x, h - 8, "RECENT ACTIONS", ACCENT, BG);
            for (i, line) in self.recent.iter().rev().take(4).enumerate() {
                c.text(x, h - 7 + i, line, MUTED, BG);
            }
        } else {
            for (i, line) in info.iter().take(2).enumerate() {
                c.text(1, h - 6 + i, line, TEXT, BG);
            }
        }
        if let Mode::Target {
            title, destination, ..
        } = &self.mode
        {
            c.text(
                1,
                2,
                &format!(
                    "{title}: choose {} · Tab cycles targets",
                    if *destination {
                        "destination"
                    } else {
                        "target"
                    }
                ),
                ACCENT,
                BG,
            );
        }
        c.text(1, h - 3, &self.notice, ACCENT, BG);
        c.text(
            1,
            h - 2,
            "Arrows/HJKL navigate · Enter select · Tab targets",
            TEXT,
            BG,
        );
        c.text(
            1,
            h - 1,
            "A abilities C cards I inspect T events P pass U undo ? help Q quit",
            MUTED,
            BG,
        );
        let modal = match &self.mode {
            Mode::Menu { title, rows, index } => Some((
                title.clone(),
                if rows.is_empty() {
                    vec!["No choices available. Esc returns to board.".into()]
                } else {
                    let start = index.saturating_sub((h - 9) / 2);
                    rows.iter()
                        .enumerate()
                        .skip(start)
                        .take(h - 9)
                        .map(|(i, r)| {
                            format!("{} {}", if i == *index { ">" } else { " " }, r.label)
                        })
                        .collect()
                },
                "Up/Down choose · Enter select · Esc cancel",
            )),
            Mode::Confirm { lines, .. } => Some((
                "CONFIRM ACTION".into(),
                lines.clone(),
                "Enter confirms · Up/Down scroll · Esc cancels",
            )),
            Mode::Info { title, lines } => Some((
                title.clone(),
                lines.clone(),
                "Up/Down scroll · Enter/Esc returns",
            )),
            Mode::Help => Some((
                "HOW TO PLAY".into(),
                vec![
                    "Arrows or HJKL: move the board cursor".into(),
                    "Enter: select a piece, then a highlighted destination".into(),
                    "Tab: targets (+ / x capture); * marks selected piece".into(),
                    "A: piece abilities   C: cards in your hand".into(),
                    "Abilities and cards preview before you confirm".into(),
                    "P: end turn   U: undo your action and AI responses".into(),
                    "Space: pause AI   I: inspect   T: events   Q: quit".into(),
                    "P0 gold / P1 blue / P2 purple / P3 red".into(),
                    "~~ magic tile   · buildable tile; hover to inspect".into(),
                    "Large boards scroll automatically with the cursor".into(),
                ],
                "Up/Down scroll · Enter/Esc returns",
            )),
            _ => None,
        };
        if let Some((title, lines, footer)) = modal {
            let mw = (w - 4).min(88);
            let mh = (lines.len() + 4).min(h - 4);
            let x = (w - mw) / 2;
            let y = (h - mh) / 2;
            let panel = Color::Rgb {
                r: 31,
                g: 43,
                b: 60,
            };
            for dy in 0..mh {
                c.text(x, y + dy, &" ".repeat(mw), TEXT, panel);
            }
            c.text(
                x + 1,
                y,
                &title.chars().take(mw - 2).collect::<String>(),
                ACCENT,
                panel,
            );
            for (i, line) in lines
                .iter()
                .skip(
                    if matches!(
                        self.mode,
                        Mode::Help | Mode::Info { .. } | Mode::Confirm { .. }
                    ) {
                        self.scroll
                    } else {
                        0
                    },
                )
                .take(mh - 3)
                .enumerate()
            {
                c.text(
                    x + 1,
                    y + 1 + i,
                    &line.chars().take(mw - 2).collect::<String>(),
                    TEXT,
                    panel,
                );
            }
            c.text(
                x + 1,
                y + mh - 1,
                &footer.chars().take(mw - 2).collect::<String>(),
                ACCENT,
                panel,
            );
        }
        c
    }
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), ResetColor, cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
/// Run the interactive terminal client, restoring the terminal on exit or error.
pub fn run(game: Game) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;
    let mut ui = Ui::new(game);
    let mut tick = Instant::now();
    let mut dirty = true;
    loop {
        if dirty {
            let (w, h) = terminal::size()?;
            ui.screen(w as usize, h as usize).draw(&mut out)?;
            dirty = false;
        }
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Input::Key(key) => {
                    if !ui.key(key) {
                        break;
                    }
                    dirty = true;
                }
                Input::Resize(..) => dirty = true,
                _ => {}
            }
        }
        if tick.elapsed() >= Duration::from_millis(250) {
            if !matches!(ui.mode, Mode::Help | Mode::Info { .. })
                && !ui.human_turn()
                && !ui.game.is_over()
                && !ui.ai_paused
            {
                ui.ai_step();
                dirty = true;
            }
            if ui.recent.len() > 100 {
                ui.recent.drain(..ui.recent.len() - 100);
            }
            tick = Instant::now();
        }
    }
    Ok(())
}
