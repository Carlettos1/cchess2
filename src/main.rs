use cchess_engine::cards::{CardChoice, CardZone, DeckDef};
use cchess_engine::GameAi;
use cchess_engine::{ActionIntent, ActionKind, Event, Game, Pos, Registry};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    path::Path,
};

fn load_directory(path: &Path) -> Result<Registry, String> {
    let mut files = std::fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    files.retain(|p| p.extension().is_some_and(|e| e == "ron"));
    files.sort();
    let sources = files
        .into_iter()
        .map(|p| {
            let name = p.display().to_string();
            std::fs::read_to_string(&p)
                .map(|text| (name.clone(), text))
                .map_err(|e| format!("{name}: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let registry = Registry::load(
        sources
            .iter()
            .map(|(name, text)| (name.as_str(), text.as_str())),
    )
    .map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let cards_path = path.join("cards");
    if cards_path.is_dir() {
        let mut files = std::fs::read_dir(&cards_path)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|f| f.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        files.retain(|p| p.extension().is_some_and(|ext| ext == "ron"));
        files.sort();
        let texts = files
            .into_iter()
            .map(|p| std::fs::read_to_string(&p).map(|s| (p.display().to_string(), s)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        registry
            .with_cards(
                texts
                    .iter()
                    .map(|(name, text)| (name.as_str(), text.as_str())),
            )
            .map_err(|errors| {
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
    } else {
        Ok(registry)
    }
}
fn square(s: &str) -> Result<Pos, String> {
    let split = s
        .bytes()
        .position(|b| b.is_ascii_digit())
        .ok_or("use a coordinate such as a1 or n14")?;
    let (file, rank) = s.split_at(split);
    if file.is_empty() || file.len() > 2 || !file.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err("use a coordinate such as a1 or n14".into());
    }
    let x = file
        .bytes()
        .fold(0u32, |n, b| n * 26 + u32::from(b - b'a') + 1);
    let y = rank.parse::<u32>().map_err(|_| "invalid rank")?;
    if !(1..=64).contains(&x) || !(1..=64).contains(&y) {
        return Err("coordinate outside supported board dimensions".into());
    }
    Ok(Pos::new(x as i32 - 1, y as i32 - 1))
}
fn file_label(x: i32) -> String {
    let mut n = x + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(char::from(b'a' + (n % 26) as u8));
        n /= 26;
    }
    letters.into_iter().rev().collect()
}
fn label(p: Pos) -> String {
    format!("{}{}", file_label(p.x), p.y + 1)
}
fn player_label(game: &Game, player: u8) -> &'static str {
    if game.players().len() == 2 {
        if player == game.players()[0].id {
            "White"
        } else {
            "Black"
        }
    } else {
        match game.player(player).unwrap().forward {
            cchess_engine::Direction::N => "White",
            cchess_engine::Direction::E => "Blue",
            cchess_engine::Direction::S => "Black",
            cchess_engine::Direction::W => "Red",
        }
    }
}
fn display(game: &Game, selected: Option<Pos>) -> String {
    render(game, selected, None)
}
fn render(
    game: &Game,
    selected: Option<Pos>,
    ability_targets: Option<&BTreeMap<Pos, char>>,
) -> String {
    let mut targets = BTreeMap::new();
    if let Some(ability_targets) = ability_targets {
        targets = ability_targets.clone();
    } else if let Some(p) = selected.and_then(|p| game.at(p)) {
        for action in game.legal_actions(p.id) {
            if let ActionIntent::Piece { kind, target, .. } = action {
                targets.insert(
                    target,
                    match kind {
                        ActionKind::Move => '.',
                        ActionKind::Take => 'x',
                        ActionKind::Attack => '!',
                        ActionKind::Ability | ActionKind::PlayCard => '+',
                    },
                );
            }
        }
    }
    let mut out = String::new();
    let four = game.players().len() == 4;
    let rank_width = game.bounds().height.to_string().len();
    for y in (0..i32::from(game.bounds().height)).rev() {
        out.push_str(&format!("{:>rank_width$} ", y + 1));
        for x in 0..i32::from(game.bounds().width) {
            let pos = Pos::new(x, y);
            if !game.is_playable(pos) {
                out.push_str(if four { "    " } else { "   " });
                continue;
            }
            let symbol = game
                .at(pos)
                .map(|p| {
                    let ch = game.registry().get(&p.archetype).unwrap().definition.symbol;
                    if four || p.owner == 0 {
                        ch
                    } else {
                        ch.to_ascii_lowercase()
                    }
                })
                .unwrap_or(if game.is_magic_tile(pos) { '~' } else { '-' });
            let marker = if selected == Some(pos) {
                '*'
            } else {
                *targets.get(&pos).unwrap_or(&' ')
            };
            if four {
                let owner = game.at(pos).map_or(' ', |p| char::from(b'0' + p.owner));
                out.push_str(&format!("{owner}{symbol}{marker} "));
            } else {
                out.push_str(&format!("{symbol}{marker} "));
            }
        }
        out.push('\n');
    }
    out.push_str(&" ".repeat(rank_width + 1));
    for x in 0..i32::from(game.bounds().width) {
        out.push_str(&format!(
            "{:<width$}",
            file_label(x),
            width = if four { 4 } else { 3 }
        ));
    }
    out.push('\n');
    if four {
        out.push_str(&format!(
            "Players: {}\n",
            game.players()
                .iter()
                .map(|p| format!(
                    "{} {}{}",
                    p.id,
                    player_label(game, p.id),
                    if game.is_eliminated(p.id) {
                        " (eliminated)"
                    } else {
                        ""
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(winner) = game.winner() {
        out.push_str(&format!("Player {winner} wins by king removal.\n"));
    } else if game.is_draw() {
        out.push_str("Draw: no surviving players.\n");
    } else {
        out.push_str(&format!(
            "Turn {}: player {} ({}), {} action(s), {} mana\n",
            game.turn().number,
            game.turn().player,
            player_label(game, game.turn().player),
            game.turn().remaining_actions,
            game.player(game.turn().player).unwrap().mana
        ));
    }
    if let Some(pending) = game.pending_promotion() {
        out.push_str(&format!(
            "Promotion required for {}: promote <{}>\n",
            label(game.piece(pending.piece).unwrap().position),
            pending.choices.join("|")
        ));
    }
    for piece in game.pieces().filter(|p| !p.statuses.is_empty()) {
        out.push_str(&format!(
            "{}: {}\n",
            label(piece.position),
            piece
                .statuses
                .values()
                .map(|s| format!("{:?} {}", s.kind, s.remaining))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !game.scheduled().is_empty() {
        out.push_str(&format!(
            "Scheduled effects: {} (use scheduled to inspect)\n",
            game.scheduled().len()
        ));
    }
    let p = game.player(game.turn().player).unwrap();
    if !p.cards.hand.is_empty() || !p.cards.deck.is_empty() {
        out.push_str(&format!(
            "Cards: {} in hand, {} in deck (use hand)\n",
            p.cards.hand.len(),
            p.cards.deck.len()
        ));
    }
    out
}
fn show_abilities(game: &Game, pos: Pos) -> Result<(), String> {
    let piece = game.at(pos).ok_or("no piece at that square")?;
    println!(
        "{} at {}: damage {}/{}, acted {}, statuses {:?}",
        piece.archetype,
        label(pos),
        piece.state.damage,
        piece.stats.tough,
        piece.state.acted,
        piece
            .statuses
            .values()
            .map(|s| format!("{:?}({})", s.kind, s.remaining))
            .collect::<Vec<_>>()
    );
    println!(
        "Strength {}, Impenetrable {}",
        piece.stats.strength, piece.stats.impenetrable
    );
    println!("Tags: {:?}", game.effective_tags(piece));
    let legal = game.legal_abilities(piece.id);
    let mut marked = BTreeMap::new();
    for definition in &game.registry().get(&piece.archetype).unwrap().abilities {
        let def = &definition.definition;
        let actions: Vec<_> = legal
            .iter()
            .filter(|a| matches!(a,ActionIntent::Ability{ability,..} | ActionIntent::Transport{ability,..} | ActionIntent::ScheduledAbility{ability,..} if ability==&def.id))
            .collect();
        let mut targets = Vec::new();
        for action in &actions {
            for p in game.ability_targets(action).map_err(|e| e.to_string())? {
                marked.insert(p, '+');
                targets.push(
                    if let ActionIntent::ScheduledAbility { scheduled, .. } = action {
                        format!("event:{scheduled} at {}", label(p))
                    } else {
                        label(p)
                    },
                );
            }
        }
        targets.sort();
        targets.dedup();
        println!(
            "{}: {} action(s), {} mana; cooldown {}, used {}; {}",
            def.id,
            def.cost.actions,
            game.effective_ability_cost(piece.id, def.cost).mana,
            piece
                .state
                .cooldowns
                .get(&def.id)
                .map_or(0, |c| c.remaining),
            piece.state.used_abilities.contains(&def.id),
            if actions.is_empty() {
                "unavailable".into()
            } else {
                format!(
                    "targets {}{}",
                    targets.join(", "),
                    if def.selection == cchess_engine::content::Selection::All {
                        " (all; omit target)"
                    } else if def.selection == cchess_engine::content::Selection::ScheduledSummon {
                        " (choose event:ID from a nearby portal; use scheduled to inspect)"
                    } else if def.selection == cchess_engine::content::Selection::Transport {
                        " (choose target and destination: ability <source> <id> <target> <destination>)"
                    } else if def.selection == cchess_engine::content::Selection::Direction {
                        " (choose an adjacent square for direction; use preview to inspect)"
                    } else {
                        ""
                    }
                )
            }
        );
    }
    if !marked.is_empty() {
        println!("{}", render(game, Some(pos), Some(&marked)));
        println!(
            "Ability targets (+): {}",
            marked
                .keys()
                .map(|p| label(*p))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}
fn hand_card(game: &Game, name: &str) -> Result<u64, String> {
    let ids = &game.player(game.turn().player).unwrap().cards.hand;
    ids.iter()
        .copied()
        .find(|id| {
            name.trim_start_matches('#').parse::<u64>().ok() == Some(*id)
                || game
                    .card_instance(*id)
                    .is_some_and(|c| c.definition == name)
        })
        .ok_or_else(|| "that card is not in your hand".into())
}
fn card_choice_label(choice: &CardChoice) -> String {
    match choice {
        CardChoice::None => String::new(),
        CardChoice::Square(pos) => format!(" {}", label(*pos)),
        CardChoice::Scheduled(id) => format!(" event:{id}"),
        CardChoice::Card(id) => format!(" card:{id}"),
    }
}
const HELP:&str="Commands: select e2 (or e2), then e4 to play a highlighted action;\nmove e2 e4; take e4 d5; attack a1 a4; abilities e1; ability e1 teleport e3;\nability b1 muster; promote queen; pass; undo; log; scheduled; board; help; quit.\nMarkers: * selected, . move, x take, ! attack. Uppercase White, lowercase Black.\nRules: one action per turn, king removal wins, pawn double moves remain available.\nUse --abilities-demo for custom pieces and on-board Fire/Ice/Invulnerability cards.\nUse --events-demo for Mandragora, Tesla, spiders, Witch and Archer.\nUse --siege-demo for Shield Bearer, Builder, Wall, Ballista and Ram.\nDirectional abilities: ability b2 build b3; preview e2 pierce e3.\nCards: hand; card <name|ID>; play <name|ID> [square|event:ID|card:ID]; cards; discard; deck; clock.\nUse --cards-demo for ordered decks, utility cards, unlocks and clock events.\nUse --expansion-demo for Crazy Pawn, Cannon, Ship, Warlock, Catapult, Imp, Basilisk, Gargoyle, Mermaid, Golem, Ogre, Oni, Necromancer and Dragon; ~ marks magic tiles.\nTransport: ability a1 launch b1 d3 (target, then destination); preview uses the same arguments.\nUse --four-player for a 14x14 cross-shaped free-for-all; cells show player ID then piece. Last survivor wins.\nMermaid: ability c2 hasten event:1; abilities c2 lists eligible summons.\nOgre: preview e2 shove f2; ability e2 shove f2 pushes directly away.\nOni: ability h1 seek h2. Necromancer: ability d1 revive c3 (needs a friendly death).\nFull army setup: --full-board; combine with --four-player for a 24x24 cross. Terrain: terrain [square]; ~ marks magic; ordinary squares are buildable.\nAI: --ai random|first|heuristic|mcts controls all opponents; --players human,random,first,human assigns seats; --ai-seed N controls reproducibility. Four-player games remain FFA.\nNo check/checkmate, castling or en passant.";

fn ai_policy(name: &str) -> Result<cchess_engine::ai::AiPolicy, String> {
    match name {
        "random" => Ok(cchess_engine::ai::AiPolicy::Random),
        "first" => Ok(cchess_engine::ai::AiPolicy::First),
        "heuristic" => Ok(cchess_engine::ai::AiPolicy::Heuristic),
        "mcts" => Ok(cchess_engine::ai::AiPolicy::Mcts),
        _ => Err("AI policy must be random, first, heuristic or mcts".into()),
    }
}

fn automated_game(
    mut game: Game,
    path: &Path,
    seed: u64,
    limit: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let mut selection_time = std::time::Duration::ZERO;
    // Create before playing so an invalid path never wastes a simulation or overwrites a log.
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut log = io::BufWriter::new(file);
    let seats = if game.players().iter().all(|p| {
        game.ai_controller(p.id)
            .is_some_and(|c| c.policy == cchess_engine::ai::AiPolicy::Random)
    }) {
        "All seats: Random AI".to_string()
    } else {
        format!(
            "Seats: {}",
            game.players()
                .iter()
                .map(|p| format!("{}={:?}", p.id, game.ai_controller(p.id).unwrap().policy))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    writeln!(
        log,
        "CCHESS AUTOMATED GAME\n{seats}\nAI seed: {seed}\nPlayers: {}\nBoard: {} x {}",
        game.players().len(),
        game.bounds().width,
        game.bounds().height
    )?;
    writeln!(
        log,
        "\nINITIAL SNAPSHOT (RON)\n{}",
        ron::ser::to_string_pretty(&game.snapshot(), ron::ser::PrettyConfig::default())?
    )?;
    log.flush()?;
    let mut count = 0;
    let result: Result<(), String> = (|| {
        while !game.is_over() {
            if limit.is_some_and(|n| count >= n) {
                return Err(format!("action limit reached ({count}); game unfinished"));
            }
            let turn = game.turn().number;
            let selection_started = std::time::Instant::now();
            let intent = game.choose_ai_action();
            selection_time += selection_started.elapsed();
            let action = game
                .apply(intent.ok_or("AI returned no action before game end")?)
                .map_err(|e| e.to_string())?;
            count += 1;
            writeln!(
                log,
                "\nACTION {count} · TURN {turn}\n{}",
                ron::ser::to_string_pretty(&action, ron::ser::PrettyConfig::default())
                    .map_err(|e| e.to_string())?
            )
            .map_err(|e| e.to_string())?;
            log.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    let status = match &result {
        Ok(()) => "COMPLETED".to_string(),
        Err(e) => format!("UNFINISHED: {e}"),
    };
    writeln!(
        log,
        "\n{status} · {count} actions\nFINAL STATE\n{}",
        display(&game, None)
    )?;
    writeln!(
        log,
        "FINAL SNAPSHOT (RON)\n{}",
        ron::ser::to_string_pretty(&game.snapshot(), ron::ser::PrettyConfig::default())?
    )?;
    log.flush()?;
    print!("{}", display(&game, None));
    eprintln!("{status} · {count} actions · log: {}", path.display());
    let elapsed = started.elapsed().as_secs_f64();
    eprintln!("Timing: {elapsed:.6}s total, {:.6}s AI selection, {:.3}ms/action, {:.2} actions/s (single-threaded)",
        selection_time.as_secs_f64(), elapsed * 1000.0 / count.max(1) as f64,
        count as f64 / elapsed.max(f64::EPSILON));
    result.map_err(Into::into)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw: Vec<_> = std::env::args().skip(1).collect();
    let mut args = Vec::new();
    let mut full_board = false;
    let mut auto_log = None;
    let mut max_actions = None;
    let mut terminal_ui = None;
    let mut players = None;
    let mut all_opponents = None;
    let mut seed = 42_u64;
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--auto-play" => {
                let path = raw
                    .get(i + 1)
                    .ok_or("--auto-play requires a log file path")?;
                if auto_log.replace(path.clone()).is_some() {
                    return Err("duplicate --auto-play".into());
                }
                i += 2;
            }
            "--max-actions" => {
                let n: usize = raw
                    .get(i + 1)
                    .ok_or("--max-actions requires a positive integer")?
                    .parse()
                    .map_err(|_| "--max-actions requires a positive integer")?;
                if n == 0 || max_actions.replace(n).is_some() {
                    return Err("--max-actions must be positive and supplied once".into());
                }
                i += 2;
            }
            "--tui" | "--cli" => {
                if terminal_ui.replace(raw[i] == "--tui").is_some() {
                    return Err("choose only one of --tui or --cli".into());
                }
                i += 1;
            }
            "--full-board" => {
                if full_board {
                    return Err("duplicate --full-board".into());
                }
                full_board = true;
                i += 1;
            }
            "--players" | "--ai" | "--ai-seed" => {
                let value = raw.get(i + 1).ok_or("missing AI option value")?;
                match raw[i].as_str() {
                    "--players" => {
                        if players.replace(value.clone()).is_some() {
                            return Err("duplicate --players".into());
                        }
                    }
                    "--ai" => {
                        if all_opponents.replace(ai_policy(value)?).is_some() {
                            return Err("duplicate --ai".into());
                        }
                    }
                    _ => {
                        seed = value
                            .parse()
                            .map_err(|_| "AI seed must be an unsigned integer")?
                    }
                }
                i += 2;
            }
            _ => {
                args.push(raw[i].clone());
                i += 1;
            }
        }
    }
    if max_actions.is_some() && auto_log.is_none() {
        return Err("--max-actions requires --auto-play".into());
    }
    if auto_log.is_some()
        && (terminal_ui == Some(true)
            || args
                .iter()
                .any(|a| a == "--print" || a.starts_with("--validate")))
    {
        return Err("--auto-play cannot combine with --tui, --print or validation".into());
    }
    if players.is_some() && all_opponents.is_some() {
        return Err("use --ai or --players, not both".into());
    }
    match args.as_slice() {
        [flag] if flag == "--help" || flag == "-h" => {
            println!("cchess-engine [--full-board | --abilities-demo | --events-demo | --siege-demo | --cards-demo | --expansion-demo | --four-player] [--print] [--tui | --cli] [--auto-play LOG_FILE [--ai POLICY | --players POLICIES] [--max-actions N]] | --validate-assets [DIRECTORY] | --validate-deck FILE\n{HELP}");
            return Ok(());
        }
        [flag] if flag == "--validate-assets" => {
            let registry = load_directory(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))?;
            println!(
                "Validated {} archetypes and {} cards.",
                registry.iter().count(),
                registry.cards().count()
            );
            return Ok(());
        }
        [flag, path] if flag == "--validate-assets" => {
            let registry = load_directory(Path::new(path))?;
            println!(
                "Validated {} archetypes and {} cards.",
                registry.iter().count(),
                registry.cards().count()
            );
            return Ok(());
        }
        [flag, path] if flag == "--validate-deck" => {
            let registry = Registry::built_in().map_err(|e| format!("{e:?}"))?;
            let deck: DeckDef = ron::from_str(&std::fs::read_to_string(path)?)?;
            registry.validate_deck(&deck)?;
            println!("Validated deck {} ({} cards).", deck.id, deck.cards.len());
            return Ok(());
        }
        [] => {}
        [flag]
            if matches!(
                flag.as_str(),
                "--print"
                    | "--abilities-demo"
                    | "--events-demo"
                    | "--siege-demo"
                    | "--cards-demo"
                    | "--expansion-demo"
                    | "--four-player"
            ) => {}
        [a, b]
            if (a == "--print"
                && matches!(
                    b.as_str(),
                    "--abilities-demo"
                        | "--events-demo"
                        | "--siege-demo"
                        | "--cards-demo"
                        | "--expansion-demo"
                        | "--four-player"
                ))
                || (b == "--print"
                    && matches!(
                        a.as_str(),
                        "--abilities-demo"
                            | "--events-demo"
                            | "--siege-demo"
                            | "--cards-demo"
                            | "--expansion-demo"
                            | "--four-player"
                    )) => {}
        _ => return Err("unknown arguments; use --help".into()),
    }
    let registry = Registry::built_in().map_err(|e| format!("built-in content errors: {e:?}"))?;
    if full_board && args.iter().any(|a| a.ends_with("-demo")) {
        return Err("--full-board cannot be combined with a demo".into());
    }
    let mut game = if full_board {
        Game::full_board(registry, args.iter().any(|a| a == "--four-player"))?
    } else if args.iter().any(|a| a == "--four-player") {
        Game::four_player(registry)?
    } else if args.iter().any(|a| a == "--expansion-demo") {
        Game::expansion_demo(registry)?
    } else if args.iter().any(|a| a == "--cards-demo") {
        Game::cards_demo(registry)?
    } else if args.iter().any(|a| a == "--siege-demo") {
        Game::siege_demo(registry)?
    } else if args.iter().any(|a| a == "--events-demo") {
        Game::events_demo(registry)?
    } else if args.iter().any(|a| a == "--abilities-demo") {
        Game::abilities_demo(registry)?
    } else {
        Game::classical(registry)?
    };
    let mut seats = Vec::new();
    if let Some(spec) = players {
        let names: Vec<_> = spec.split(',').collect();
        if names.len() != game.players().len() {
            return Err("--players must list one controller per seat".into());
        }
        for (player, name) in game.players().iter().zip(names) {
            if name == "human" && auto_log.is_some() {
                return Err("--auto-play requires AI controllers for every seat".into());
            }
            if name != "human" {
                seats.push((player.id, ai_policy(name)?));
            }
        }
    } else if auto_log.is_some() {
        let policy = all_opponents.unwrap_or(cchess_engine::ai::AiPolicy::Random);
        seats.extend(game.players().iter().map(|p| (p.id, policy)));
    } else if let Some(policy) = all_opponents {
        seats.extend(game.players().iter().skip(1).map(|p| (p.id, policy)));
    }
    game = game.with_ai(seats, seed)?;
    if let Some(path) = auto_log {
        return automated_game(game, Path::new(&path), seed, max_actions);
    }
    if args.iter().any(|a| a == "--print") {
        print!("{}", display(&game, None));
        return Ok(());
    }
    use std::io::IsTerminal;
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    if terminal_ui.unwrap_or(interactive) {
        if !interactive {
            return Err("--tui requires an interactive terminal; use --cli for piped input".into());
        }
        return cchess_engine::tui::run(game).map_err(Into::into);
    }
    println!("{HELP}\n{}", display(&game, None));
    let mut history = Vec::new();
    let mut selected = None;
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        let mut ai_actions = 0;
        while !game.is_over()
            && game.ai_controller(game.turn().player).is_some()
            && ai_actions < 256
        {
            let player = game.turn().player;
            match game.play_ai_step() {
                Ok(Some(action)) => {
                    println!("AI player {player}: {:?}", action.intent);
                    ai_actions += 1;
                }
                Ok(None) => break,
                Err(e) => {
                    println!("AI paused: {e}");
                    break;
                }
            }
        }
        if ai_actions > 0 {
            selected = None;
            println!("{}", display(&game, None));
        }
        if ai_actions == 256 {
            println!("AI paused after 256 actions. Enter ai to continue, undo or quit.");
        }
        print!("> ");
        io::stdout().flush()?;
        let Some(line) = lines.next() else {
            break;
        };
        let line = line?;
        let words: Vec<_> = line.split_whitespace().collect();
        let result: Result<Option<ActionIntent>, String> = (|| match words.as_slice() {
            [] => Ok(None),
            ["quit" | "q"] => Ok(None),
            ["ai"] => Ok(None),
            ["help"] => {
                println!("{HELP}");
                Ok(None)
            }
            ["terrain"] => {
                let playable: Vec<_> = game
                    .bounds()
                    .positions()
                    .filter(|p| game.is_playable(*p))
                    .collect();
                let magic: Vec<_> = playable
                    .iter()
                    .filter(|p| game.is_magic_tile(**p))
                    .map(|p| label(*p))
                    .collect();
                println!(
                    "Terrain: {} playable, {} buildable. Magic: {}",
                    playable.len(),
                    playable
                        .iter()
                        .filter(|p| game.is_buildable_tile(**p))
                        .count(),
                    magic.join(", ")
                );
                Ok(None)
            }
            ["terrain", square_name] => {
                let p = square(square_name)?;
                println!(
                    "{}: playable {}, buildable {}, magic {}",
                    label(p),
                    game.is_playable(p),
                    game.is_buildable_tile(p),
                    game.is_magic_tile(p)
                );
                Ok(None)
            }
            ["board"] => {
                println!("{}", display(&game, selected));
                Ok(None)
            }
            ["scheduled"] => {
                for job in game.scheduled() {
                    let source = match job.cause.source {
                        cchess_engine::state::ActionSource::Piece(id) => game
                            .piece(id)
                            .map(|p| format!("{} at {}", p.archetype, label(p.position)))
                            .unwrap_or_else(|| "removed piece".into()),
                        cchess_engine::state::ActionSource::Card(id) => game
                            .card_instance(id)
                            .map(|c| format!("{} card", c.definition))
                            .unwrap_or_else(|| "removed card".into()),
                        cchess_engine::state::ActionSource::System => "system".into(),
                    };
                    println!(
                        "Event {}: player {}: {} from {}, in {} {:?} tick(s)",
                        job.id,
                        job.owner,
                        job.cause
                            .ability
                            .as_deref()
                            .or(job.cause.trigger.as_deref())
                            .unwrap_or("effect"),
                        source,
                        job.due_tick
                            .saturating_sub(game.clock_value(job.owner, job.clock)),
                        job.clock
                    );
                }
                if game.scheduled().is_empty() {
                    println!("No scheduled effects.");
                }
                Ok(None)
            }
            ["log"] => {
                println!(
                    "{}",
                    ron::ser::to_string_pretty(game.log(), ron::ser::PrettyConfig::default())
                        .map_err(|e| e.to_string())?
                );
                Ok(None)
            }
            ["undo"] => {
                game = history.pop().ok_or("nothing to undo")?;
                selected = None;
                println!("{}", display(&game, None));
                Ok(None)
            }
            ["clock"] => {
                println!(
                    "Clock: {} movements, {} turn starts, {} round starts",
                    game.clock().movements,
                    game.clock().turns,
                    game.clock().rounds
                );
                for owner in [0, 1] {
                    println!(
                        "Player {owner}: {} owner turn starts",
                        game.owner_turn(owner).unwrap()
                    );
                }
                Ok(None)
            }
            ["hand"] | ["discard"] => {
                let zone = if words[0] == "hand" {
                    CardZone::Hand
                } else {
                    CardZone::Discard
                };
                let player = game.player(game.turn().player).unwrap();
                for id in player.cards.get(zone) {
                    let c = game.card_instance(*id).unwrap();
                    let def = &game.registry().card(&c.definition).unwrap().definition;
                    println!(
                        "#{id} {}: {:?}, {} action(s), {} mana",
                        c.definition, def.kind, def.cost.actions, def.cost.mana
                    );
                }
                if player.cards.get(zone).is_empty() {
                    println!("No cards in {zone:?}.");
                }
                Ok(None)
            }
            ["cards"] => {
                for c in game
                    .card_instances()
                    .filter(|c| c.zone == CardZone::OnBoard)
                {
                    println!("Player {}: #{} {}", c.owner, c.id, c.definition);
                }
                Ok(None)
            }
            ["deck"] => {
                let p = game.player(game.turn().player).unwrap();
                println!(
                    "Deck: {} undrawn; hand: {}; discard: {}; consumed: {}",
                    p.cards.deck.len(),
                    p.cards.hand.len(),
                    p.cards.discard.len(),
                    p.cards.consumed.len()
                );
                Ok(None)
            }
            ["card", name] => {
                let id = hand_card(&game, name)?;
                let choices = game.legal_card_plays(id);
                println!(
                    "{}: {} legal play(s)",
                    game.card_instance(id).unwrap().definition,
                    choices.len()
                );
                for choice in choices {
                    if let ActionIntent::PlayCard { target, .. } = choice {
                        println!("play {id}{}", card_choice_label(&target));
                    }
                }
                Ok(None)
            }
            ["play", name] | ["play", name, _] => {
                let card = hand_card(&game, name)?;
                let target = if let Some(arg) = words.get(2) {
                    if let Some(id) = arg.strip_prefix("event:") {
                        CardChoice::Scheduled(id.parse().map_err(|_| "invalid event ID")?)
                    } else if let Some(id) = arg.strip_prefix("card:") {
                        CardChoice::Card(id.parse().map_err(|_| "invalid card ID")?)
                    } else {
                        CardChoice::Square(square(arg)?)
                    }
                } else {
                    CardChoice::None
                };
                Ok(Some(ActionIntent::PlayCard {
                    player: game.turn().player,
                    card,
                    target,
                }))
            }
            ["abilities", p] => {
                show_abilities(&game, square(p)?)?;
                Ok(None)
            }
            ["preview", from, id]
            | ["preview", from, id, _]
            | ["preview", from, id, _, _]
            | ["ability", from, id]
            | ["ability", from, id, _]
            | ["ability", from, id, _, _] => {
                let piece = game.at(square(from)?).ok_or("no source piece")?;
                let definition = game
                    .registry()
                    .get(&piece.archetype)
                    .unwrap()
                    .abilities
                    .iter()
                    .find(|a| a.definition.id == *id)
                    .ok_or("unknown ability")?;
                let scheduled = if definition.definition.selection
                    == cchess_engine::content::Selection::ScheduledSummon
                {
                    if words.len() != 4 {
                        return Err("this ability requires event:ID".into());
                    }
                    Some(
                        words[3]
                            .strip_prefix("event:")
                            .ok_or("use event:ID")?
                            .parse::<u64>()
                            .map_err(|_| "invalid event ID")?,
                    )
                } else {
                    None
                };
                let target = if scheduled.is_some() {
                    None
                } else if words.len() >= 4 {
                    Some(square(words[3])?)
                } else if definition.definition.selection == cchess_engine::content::Selection::All
                {
                    None
                } else if definition.definition.target.pattern
                    == cchess_engine::geometry::Pattern::SelfOnly
                {
                    Some(piece.position)
                } else {
                    return Err("this ability requires a target square".into());
                };
                let intent = if let Some(scheduled) = scheduled {
                    ActionIntent::ScheduledAbility {
                        player: game.turn().player,
                        source: piece.id,
                        ability: (*id).into(),
                        scheduled,
                    }
                } else if words.len() == 5 {
                    ActionIntent::Transport {
                        player: game.turn().player,
                        source: piece.id,
                        ability: (*id).into(),
                        target: target.unwrap(),
                        destination: square(words[4])?,
                    }
                } else {
                    ActionIntent::Ability {
                        player: game.turn().player,
                        source: piece.id,
                        ability: (*id).into(),
                        target,
                    }
                };
                if words[0] == "preview" {
                    let result = game.preview(&intent).map_err(|e| e.to_string())?;
                    let mut positions: BTreeMap<_, _> =
                        game.pieces().map(|p| (p.id, p.position)).collect();
                    println!("Preview (no action spent):");
                    for event in result.events {
                        match event {
                            Event::AbilityResolved { .. } => break,
                            Event::PieceRevived {
                                archetype,
                                position,
                                ..
                            } => println!("Revive {archetype} at {}", label(position)),
                            Event::ScheduledChanged {
                                id, before, after, ..
                            } => println!("Event {id}: due tick {before} -> {after}"),
                            Event::TraversalVisited { position, .. } => {
                                println!("Ray visits {}", label(position))
                            }
                            Event::TraversalStopped { position, .. } => {
                                println!("Blocked at {}", label(position))
                            }
                            Event::PieceSpawned {
                                archetype,
                                position,
                                ..
                            } => println!("Build {archetype} at {}", label(position)),
                            Event::DamageDealt { target, amount, .. } => {
                                if let Some(p) = game.piece(target) {
                                    println!(
                                        "{} at {} takes {amount} damage",
                                        p.archetype,
                                        label(positions[&target])
                                    );
                                }
                            }
                            Event::PieceMoved { piece, from, to } => {
                                positions.insert(piece, to);
                                println!("Move {} to {}", label(from), label(to))
                            }
                            Event::StatModified {
                                piece,
                                stat,
                                before,
                                after,
                                ..
                            } => {
                                if let Some(p) = game.piece(piece) {
                                    println!(
                                        "{} at {}: {stat:?} {before} -> {after}",
                                        p.archetype,
                                        label(p.position)
                                    );
                                }
                            }
                            _ => (),
                        }
                    }
                    Ok(None)
                } else {
                    Ok(Some(intent))
                }
            }
            ["promote", archetype] => {
                let pending = game.pending_promotion().ok_or("no pending promotion")?;
                Ok(Some(ActionIntent::ChoosePromotion {
                    player: game.turn().player,
                    token: pending.token,
                    archetype: (*archetype).into(),
                }))
            }
            ["pass"] => Ok(Some(ActionIntent::EndTurn {
                player: game.turn().player,
            })),
            ["select", p] => {
                let pos = square(p)?;
                let piece = game.at(pos).ok_or("no piece at that square")?;
                if piece.owner != game.turn().player {
                    return Err("select the current player's piece".into());
                }
                selected = Some(pos);
                println!("{}", display(&game, selected));
                let targets = game
                    .legal_actions(piece.id)
                    .iter()
                    .filter_map(|a| {
                        if let ActionIntent::Piece { kind, target, .. } = a {
                            Some(format!("{kind:?} {}", label(*target)))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                println!(
                    "Legal: {}",
                    if targets.is_empty() {
                        "none".into()
                    } else {
                        targets.join(", ")
                    }
                );
                Ok(None)
            }
            [kind, from, to] if matches!(*kind, "move" | "take" | "attack") => {
                let source = game.at(square(from)?).ok_or("no source piece")?.id;
                let kind = match *kind {
                    "move" => ActionKind::Move,
                    "take" => ActionKind::Take,
                    _ => ActionKind::Attack,
                };
                Ok(Some(ActionIntent::Piece {
                    player: game.turn().player,
                    source,
                    kind,
                    target: square(to)?,
                }))
            }
            [p] => {
                let pos = square(p)?;
                if let Some(source) = selected.and_then(|p| game.at(p)) {
                    let legal = game.legal_actions(source.id);
                    let intent = legal
                        .into_iter()
                        .find(|a| matches!(a,ActionIntent::Piece{target,..} if *target==pos));
                    if intent.is_some() {
                        return Ok(intent);
                    }
                }
                let piece = game
                    .at(pos)
                    .ok_or("select a friendly piece or a highlighted destination")?;
                if piece.owner != game.turn().player {
                    return Err("select the current player's piece".into());
                }
                selected = Some(pos);
                println!("{}", display(&game, selected));
                Ok(None)
            }
            _ => Err("unknown command; use help".into()),
        })();
        if matches!(words.as_slice(), ["quit" | "q"]) {
            break;
        }
        match result {
            Ok(Some(intent)) => {
                if game.ai_controller(game.turn().player).is_some() && !game.is_over() {
                    println!("This seat is controlled by AI; use ai, undo or quit.");
                    continue;
                }
                let before = game.clone();
                match game.apply(intent) {
                    Ok(action) => {
                        if history.len() == 100 {
                            history.remove(0);
                        }
                        history.push(before);
                        selected = None;
                        println!("Action {} resolved.\n{}", action.id, display(&game, None));
                    }
                    Err(e) => println!("Cannot act: {e}"),
                }
            }
            Ok(None) => {}
            Err(e) => println!("{e}"),
        }
    }
    Ok(())
}
