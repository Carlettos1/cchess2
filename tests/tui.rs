mod support;
use cchess_engine::{ai::AiPolicy, tui::Ui, *};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
fn key(ui: &mut Ui, code: KeyCode) {
    assert!(ui.key(KeyEvent::new(code, KeyModifiers::NONE)));
}
#[test]
fn keyboard_movement_and_undo_restore_ai_response() {
    let g = Game::classical(Registry::built_in().unwrap())
        .unwrap()
        .with_ai(vec![(1, AiPolicy::First)], 42)
        .unwrap();
    let before = g.snapshot();
    let mut ui = Ui::new(g);
    ui.cursor = Pos::new(4, 1);
    key(&mut ui, KeyCode::Enter);
    key(&mut ui, KeyCode::Up);
    key(&mut ui, KeyCode::Enter);
    assert_eq!(ui.game.at(Pos::new(4, 2)).unwrap().archetype, "pawn");
    ui.ai_step();
    assert!(ui.game.log().len() >= 2);
    key(&mut ui, KeyCode::Char('u'));
    assert_eq!(ui.game.snapshot(), before);
}
#[test]
fn card_preview_cancel_and_confirm_are_atomic() {
    let mut ui = Ui::new(support::game(
        &[("rook", 0, 0, 0), ("pawn", 1, 0, 3)],
        &["mana"],
        &["mana"],
        1,
    ));
    let before = ui.game.snapshot();
    key(&mut ui, KeyCode::Char('c'));
    assert!(ui.render_text(100, 30).contains("HAND"));
    key(&mut ui, KeyCode::Enter);
    assert!(ui.render_text(100, 30).contains("CONFIRM ACTION"));
    assert_eq!(ui.game.snapshot(), before);
    key(&mut ui, KeyCode::Esc);
    assert_eq!(ui.game.snapshot(), before);
    key(&mut ui, KeyCode::Char('c'));
    key(&mut ui, KeyCode::Enter);
    key(&mut ui, KeyCode::Enter);
    assert_eq!(ui.game.log().len(), 1);
    assert!(ui.game.player(0).unwrap().cards.hand.is_empty());
}
#[test]
fn transport_selects_piece_then_destination_then_confirmation() {
    let mut ui = Ui::new(support::game(
        &[("catapult", 0, 0, 0), ("pawn", 0, 1, 0), ("pawn", 1, 7, 7)],
        &["mana"],
        &["mana"],
        0,
    ));
    let before = ui.game.snapshot();
    ui.cursor = Pos::new(0, 0);
    key(&mut ui, KeyCode::Char('a'));
    key(&mut ui, KeyCode::Enter);
    ui.cursor = Pos::new(1, 0);
    key(&mut ui, KeyCode::Enter);
    assert!(ui.render_text(100, 30).contains("choose destination"));
    key(&mut ui, KeyCode::Tab);
    let destination = ui.cursor;
    key(&mut ui, KeyCode::Enter);
    assert!(ui.render_text(100, 30).contains("CONFIRM ACTION"));
    assert_eq!(ui.game.snapshot(), before);
    key(&mut ui, KeyCode::Enter);
    assert_eq!(ui.game.at(destination).unwrap().archetype, "pawn");
}
#[test]
fn promotion_menu_completes_a_move() {
    let mut ui = Ui::new(support::game(
        &[("pawn", 0, 0, 6), ("pawn", 1, 7, 6)],
        &["mana"],
        &["mana"],
        0,
    ));
    ui.cursor = Pos::new(0, 6);
    key(&mut ui, KeyCode::Enter);
    key(&mut ui, KeyCode::Up);
    key(&mut ui, KeyCode::Enter);
    assert!(ui.render_text(100, 30).contains("PROMOTION"));
    key(&mut ui, KeyCode::Enter);
    key(&mut ui, KeyCode::Enter);
    assert!(ui.game.pending_promotion().is_none());
    assert_ne!(ui.game.at(Pos::new(0, 7)).unwrap().archetype, "pawn");
}
#[test]
fn responsive_layout_and_ai_pause_cover_full_cross_board() {
    for four in [false, true] {
        let g = Game::full_board(Registry::built_in().unwrap(), four)
            .unwrap()
            .with_ai(vec![(0, AiPolicy::Random)], 7)
            .unwrap();
        let mut ui = Ui::new(g);
        key(&mut ui, KeyCode::Char(' '));
        let before = ui.game.snapshot();
        ui.ai_step();
        assert_eq!(ui.game.snapshot(), before);
        for (w, h) in [(0, 0), (30, 10), (40, 16), (80, 24), (120, 40)] {
            let text = ui.render_text(w, h);
            if w > 0 {
                assert_eq!(text.lines().count(), h as usize);
                assert!(text.lines().all(|l| l.chars().count() == w as usize));
            }
        }
        ui.cursor = if four {
            Pos::new(19, 23)
        } else {
            Pos::new(15, 16)
        };
        assert!(ui
            .render_text(80, 24)
            .contains(if four { "INSPECT t24" } else { "INSPECT p17" }));
        key(&mut ui, KeyCode::Char('?'));
        assert!(ui.render_text(80, 24).contains("HOW TO PLAY"));
        assert!(!ui.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)));
    }
}
#[test]
fn forced_tui_requires_terminal_and_cli_still_accepts_pipes() {
    use std::process::{Command, Stdio};
    let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--tui")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    let output = Command::new(env!("CARGO_BIN_EXE_cchess-engine"))
        .arg("--cli")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
}
