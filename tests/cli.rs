use cchess_engine::GameAi;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn terminal_highlights_actions_and_plays_a_complete_king_capture_game() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"select e2\ne4\nmove f7 f6\nmove d1 h5\nmove a7 a6\ntake h5 e8\npass\nundo\nboard\nlog\nquit\n").unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let out = String::from_utf8(result.stdout).unwrap();
    assert!(out.contains("Legal: Move e3, Move e4"), "{out}");
    assert!(out.contains("Player 0 wins by king removal."), "{out}");
    assert!(out.contains("Cannot act: the game has ended"), "{out}");
    assert!(out.contains("Turn 5: player 0"), "{out}");
    assert!(out.contains("ActionResolved"), "{out}");
}
#[test]
fn asset_validation_runs_without_starting_a_match() {
    let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--validate-assets")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "Validated 36 archetypes and 20 cards."
    );
}
#[test]
fn print_mode_is_noninteractive_and_unknown_options_fail() {
    let out = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--print")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("Turn 1: player 0"));
    assert!(!Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--does-not-exist")
        .output()
        .unwrap()
        .status
        .success());
}

fn demo_session(input: &[u8]) -> String {
    session("--abilities-demo", input)
}
fn session(demo: &str, input: &[u8]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg(demo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn events_demo_exposes_jobs_and_undo_restores_scheduling() {
    let out = session("--events-demo", b"scheduled\nability b2 harvest\nscheduled\nundo\nscheduled\npass\npass\npass\npass\nabilities a3\nlog\nquit\n");
    assert!(out.contains("harvest from mandragora at b2"), "{out}");
    assert!(out.contains("spider at a3"), "{out}");
    assert!(out.contains("PieceTransformed"), "{out}");
    assert!(out.contains("ScheduledResolved"), "{out}");
    assert!(
        !out.contains("ManaGranted"),
        "undo must remove harvest jobs: {out}"
    );
}

#[test]
fn siege_demo_previews_builds_fortifies_and_undo_restores_stats_and_entities() {
    let out = session("--siege-demo", b"preview b2 build b3\nabilities b2\nability b2 build b3\nundo\npreview b2 build b3\nability c1 fortify\nabilities d1\nundo\nabilities d1\nlog\nquit\n");
    assert_eq!(out.matches("Build wall at a3").count(), 2, "{out}");
    assert_eq!(out.matches("Build wall at b3").count(), 2, "{out}");
    assert_eq!(out.matches("Build wall at c3").count(), 2, "{out}");
    assert!(
        out.contains("choose an adjacent square for direction"),
        "{out}"
    );
    assert!(out.contains("Impenetrable 4"), "{out}");
    assert!(out.contains("Impenetrable 3"), "{out}");
    assert_eq!(out.matches("Action 1 resolved.").count(), 2, "{out}");
    assert!(
        !out.contains("StatModified"),
        "undo must remove action log: {out}"
    );
}

#[test]
fn siege_demo_previews_and_undoes_rays_without_spending_preview_actions() {
    let out = session("--siege-demo", b"preview e2 pierce e3\nability e2 pierce e3\nundo\npreview g2 charge g3\nability g2 charge g3\nundo\nabilities g2\nlog\nquit\n");
    assert!(out.contains("Blocked at e8"), "{out}");
    assert!(out.contains("pawn at e4 takes 1 damage"), "{out}");
    assert!(out.contains("Move g2 to g8"), "{out}");
    assert!(out.contains("ram at g2"), "{out}");
    assert_eq!(out.matches("Action 1 resolved.").count(), 2, "{out}");
    assert!(!out.contains("Cannot act"), "{out}");
    assert!(
        !out.contains("TraversalStarted"),
        "undo must remove ray log: {out}"
    );
}

#[test]
fn cards_demo_plays_draws_hastes_and_undo_restores_clock_and_jobs() {
    let out = session("--cards-demo", b"pass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\npass\nhand\nplay forced_march\ncard summon_knight\nplay summon_knight c2\nplay fire\nabilities c3\nplay supply_cache\nscheduled\nplay event_haste event:3\nhand\nclock\nundo\nscheduled\nclock\ndiscard\ncards\ndeck\nquit\n");
    assert!(out.contains("#1 forced_march"), "{out}");
    assert!(out.contains("play 2 c2"), "{out}");
    assert!(out.contains("3 action(s)"), "{out}");
    assert!(
        out.contains("#9 more_mana"),
        "drawn cards remain inspectable: {out}"
    );
    assert_eq!(
        out.matches("from supply_cache card").count(),
        2,
        "undo must restore the job: {out}"
    );
    assert!(out.contains("Clock: 5 movements, 24 turn starts"), "{out}");
    assert!(out.contains("Clock: 4 movements, 24 turn starts"), "{out}");
    assert!(out.contains("Player 0: #3 fire"), "{out}");
    assert!(!out.contains("Cannot act"), "{out}");
}

#[test]
fn starter_deck_validation_and_cards_print_mode_are_available() {
    let result = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args([
            "--validate-deck",
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/decks/starter.ron"),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("Validated deck starter (21 cards)"));
    let result = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args(["--cards-demo", "--print"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("Cards: 8 in hand, 10 in deck"));
}
#[test]
fn terminal_promotes_and_undo_restores_the_pending_choice() {
    let out=demo_session(b"move b7 b8\npass\npromote king\npromote knight\nundo\npromote queen\nabilities b8\nlog\nquit\n");
    assert!(out.contains("Promotion required for b8"), "{out}");
    assert!(
        out.contains("choose the pending pawn promotion first"),
        "{out}"
    );
    assert!(out.contains("invalid promotion choice"), "{out}");
    assert!(out.contains("queen at b8"), "{out}");
    assert!(out.contains("PieceTransformed"), "{out}");
}
#[test]
fn terminal_previews_and_casts_status_abilities_and_shows_cooldowns() {
    let out=demo_session(b"pass\npass\npass\npass\nabilities c3\nability c3 ice\nabilities e5\nmove e5 e4\npass\nabilities c3\nability c3 ice\nlog\nquit\n");
    assert!(out.contains("Ability targets (+)"), "{out}");
    assert!(out.contains("Ice 2"), "{out}");
    assert!(out.contains("cannot act while Ice is active"), "{out}");
    assert!(out.contains("ice is on cooldown for 5"), "{out}");
    assert!(out.contains("StatusApplied"), "{out}");
}
#[test]
fn terminal_immune_king_cannot_teleport_itself() {
    let out = demo_session(b"pass\npass\npass\npass\nability e1 teleport e3\nabilities e1\nquit\n");
    assert!(out.contains("the target rejects this action"), "{out}");
    assert!(out.contains("king at e1"), "{out}");
    assert!(!out.contains("AbilityResolved"), "{out}");
}

#[test]
fn expansion_demo_exercises_magic_portals_and_sacrifice() {
    let out = session(
        "--expansion-demo",
        b"pass\npass\npass\npass\npass\npass\npass\npass\nselect b3\nability d2 portals\nlog\nundo\nability b3 sacrifice\npass\nlog\nquit\n",
    );
    assert!(out.contains('~'), "{out}");
    assert!(out.contains("PieceSpawned"), "{out}");
    assert!(out.contains("PieceRemoved"), "{out}");
    assert!(out.contains("ScheduledResolved"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn catapult_compound_command_supports_preview_apply_and_undo() {
    let out=session("--expansion-demo", b"abilities a1\npreview a1 launch b1 d3\nability a1 launch b1 d3\nlog\nundo\nability b1 draw\nlog\nquit\n");
    assert!(out.contains("choose target and destination"), "{out}");
    assert!(out.contains("Preview (no action spent)"), "{out}");
    assert!(out.contains("TransportStarted"), "{out}");
    assert!(out.contains("CardMoved"), "{out}");
    assert!(out.contains("ability: Some(\"draw\")"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn four_player_terminal_rotates_turns_parses_large_coordinates_and_undoes() {
    let out=session("--four-player",b"select d2\nmove d2 d4\nmove b11 d11\nmove k13 k11\nmove m4 k4\nclock\nundo\nboard\nquit\n");
    assert!(out.contains("0 White, 1 Blue, 2 Black, 3 Red"), "{out}");
    assert!(out.contains("Turn 5: player 0"), "{out}");
    assert!(out.contains("Turn 4: player 3"), "{out}");
    assert!(out.contains("3K"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn mermaid_terminal_selects_and_previews_a_nearby_portal_summon() {
    let out=session("--expansion-demo",b"pass\npass\npass\npass\npass\npass\npass\npass\nability d2 portals\npass\nability c3 summon_witch c4\npass\nabilities c2\npreview c2 hasten event:1\nability c2 hasten event:1\nlog\nquit\n");
    assert!(out.contains("event:1 at c3"), "{out}");
    assert!(out.contains("Event 1: due tick"), "{out}");
    assert!(out.contains("ScheduledResolved"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn ogre_shove_demo_supports_preview_commit_and_undo() {
    let out = session(
        "--expansion-demo",
        b"abilities e2\npreview e2 shove f2\nability e2 shove f2\nlog\nundo\nabilities f2\nquit\n",
    );
    assert!(out.contains("Move f2 to h2"), "{out}");
    assert!(out.contains("TransportStarted"), "{out}");
    assert!(out.contains("golem at f2: damage 0/3"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn oni_and_necromancer_demo_support_preview_undo_and_undead_inspection() {
    let out=session("--expansion-demo",b"pass\npass\npass\npass\npass\npass\npass\npass\npreview h1 seek h2\nability h1 seek h2\nundo\nability b3 sacrifice\npass\npreview d1 revive c3\nability d1 revive c3\nabilities c3\nlog\nquit\n");
    assert!(out.contains("Revive crazy_pawn at c3"), "{out}");
    assert!(out.contains("PieceRevived"), "{out}");
    assert!(out.contains("Tags: {Undead}"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn dragon_demo_exposes_protection_movement_and_undo() {
    let out = session(
        "--expansion-demo",
        b"abilities f1\nmove f1 h2\nundo\nabilities f1\nquit\n",
    );
    assert!(out.contains("dragon at f1: damage 0/3"), "{out}");
    assert!(out.contains("Tags: {Immune, Heroic}"), "{out}");
    assert!(!out.contains("Cannot act:"), "{out}");
}

#[test]
fn human_vs_ai_runs_response_and_undo_restores_before_human_action() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args(["--ai", "first"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"move e2 e4\nundo\nboard\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8(output.stdout).unwrap();
    assert!(out.contains("AI player 1:"), "{out}");
    assert!(out.matches("Turn 1: player 0").count() >= 2, "{out}");
}
#[test]
fn mixed_four_player_controllers_stop_at_next_human_seat() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args([
            "--four-player",
            "--players",
            "human,first,random,human",
            "--ai-seed",
            "7",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"move d2 d4\nquit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let out = String::from_utf8(output.stdout).unwrap();
    assert!(out.contains("AI player 1:"), "{out}");
    assert!(out.contains("AI player 2:"), "{out}");
    assert!(out.contains("Turn 4: player 3"), "{out}");
    assert!(!out.contains("AI player 3:"));
    assert!(!Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args(["--players", "human,random,first", "--print"])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn full_board_cli_starts_human_vs_ai_and_exposes_magic_and_buildable_tiles() {
    for (flags, input, expected) in [
        (
            vec!["--full-board", "--ai", "random"],
            b"terrain\nterrain a8\nmove a4 a6\nquit\n".as_slice(),
            "272 playable, 272 buildable",
        ),
        (
            vec![
                "--full-board",
                "--four-player",
                "--players",
                "human,first,human,random",
            ],
            b"terrain\nterrain a1\nmove e4 e6\nquit\n".as_slice(),
            "512 playable, 512 buildable",
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
            .args(flags)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        let out = String::from_utf8(result.stdout).unwrap();
        assert!(out.contains(expected), "{out}");
        assert!(out.contains("AI player 1:"), "{out}");
        assert!(!out.contains("Cannot act"), "{out}");
    }
    let result = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .args(["--full-board", "--four-player", "--ai", "first", "--print"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(!String::from_utf8(result.stdout)
        .unwrap()
        .contains("AI player"));
}

#[test]
fn automated_random_games_finish_print_once_and_log_replayable_actions() {
    use cchess_engine::{ai::AiPolicy, commands::ResolvedAction, Game, Registry};
    for four in [false, true] {
        let path = std::env::temp_dir().join(format!(
            "cchess-auto-test-{}-{four}.txt",
            std::process::id()
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_cchess-engine"));
        command.args(["--cli", "--auto-play"]).arg(&path).args([
            "--ai-seed",
            "42",
            "--max-actions",
            "1000",
        ]);
        if four {
            command.arg("--four-player");
        }
        let output = command.stdin(Stdio::null()).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.matches("Cards:").count(), 1, "{stdout}");
        assert!(stdout.contains("wins"), "{stdout}");
        assert!(!stdout.contains("AI player") && !stdout.contains("> "));
        let log = std::fs::read_to_string(&path).unwrap();
        assert!(
            log.contains("INITIAL SNAPSHOT (RON)")
                && log.contains("FINAL SNAPSHOT (RON)")
                && log.contains("COMPLETED")
        );
        let registry = Registry::built_in().unwrap();
        let game = if four {
            Game::four_player(registry)
        } else {
            Game::classical(registry)
        }
        .unwrap();
        let seats = game
            .players()
            .iter()
            .map(|p| (p.id, AiPolicy::Random))
            .collect();
        let mut replay = game.with_ai(seats, 42).unwrap();
        let mut count = 0;
        for section in log.split("\nACTION ").skip(1) {
            let (_, body) = section.split_once('\n').unwrap();
            let body = body.split("\nCOMPLETED").next().unwrap().trim();
            let recorded: ResolvedAction = ron::from_str(body).unwrap();
            assert_eq!(replay.choose_ai_action(), Some(recorded.intent.clone()));
            assert_eq!(replay.apply(recorded.intent.clone()).unwrap(), recorded);
            count += 1;
        }
        assert!(count > 0 && replay.is_over());
        let snapshot: cchess_engine::state::Snapshot =
            ron::from_str(log.split("FINAL SNAPSHOT (RON)\n").nth(1).unwrap()).unwrap();
        assert_eq!(replay.snapshot(), snapshot);
        // An existing log must never be silently overwritten.
        let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
            .arg("--auto-play")
            .arg(&path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), log);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn automated_action_limit_preserves_log_without_claiming_a_finished_game() {
    let path = std::env::temp_dir().join(format!("cchess-auto-limit-{}.txt", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--auto-play")
        .arg(&path)
        .args(["--full-board", "--four-player", "--max-actions", "1"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let log = std::fs::read_to_string(&path).unwrap();
    assert_eq!(log.matches("\nACTION ").count(), 1);
    assert!(log.contains("UNFINISHED: action limit reached (1)"));
    assert!(!log.contains("COMPLETED"));
    std::fs::remove_file(path).unwrap();
    for args in [
        vec!["--auto-play"],
        vec!["--max-actions", "1"],
        vec!["--auto-play", "unused.txt", "--tui"],
        vec!["--auto-play", "unused.txt", "--players", "human,mcts"],
    ] {
        assert!(!Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn heuristic_policy_is_accepted_in_both_cli_forms() {
    for args in [
        vec!["--ai", "heuristic", "--print"],
        vec!["--players", "heuristic,human", "--print"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn automated_mcts_and_mixed_policies_log_real_controllers_and_timing() {
    for (label, flags) in [
        ("mcts", vec!["--ai", "mcts"]),
        ("mixed", vec!["--players", "mcts,first"]),
    ] {
        let path =
            std::env::temp_dir().join(format!("cchess-auto-{label}-{}.txt", std::process::id()));
        let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
            .arg("--auto-play")
            .arg(&path)
            .args(flags)
            .args(["--max-actions", "2"])
            .output()
            .unwrap();
        assert!(!output.status.success()); // Bounded run is explicitly unfinished.
        let log = std::fs::read_to_string(&path).unwrap();
        assert!(log.contains("Seats: 0=Mcts"));
        assert!(!log.contains("All seats: Random AI"));
        assert_eq!(log.matches("\nACTION ").count(), 2);
        assert!(log.contains("UNFINISHED: action limit reached (2)"));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("Timing:")
                && stderr.contains("AI selection")
                && stderr.contains("single-threaded")
        );
        std::fs::remove_file(path).unwrap();
    }
}
