# CChess desktop

The desktop and browser development client both execute the real Rust engine through
`cchess-session`. React renders projected state and selects opaque, revision-bound
choices. It does not calculate moves, spend resources or resolve effects.

```sh
cd desktop
npm install
npm run dev        # Vite + automatically launched Rust bridge; http://localhost:1420
npm test
npm run build
npm run tauri dev  # native window; requires Tauri system prerequisites
npm run tauri -- build --no-bundle  # build the native release executable
```

Rust and Cargo are required for browser development too. Restart the development
server after Rust changes. A static `dist/` server alone cannot run the game;
production gameplay runs in the Tauri application. Fixtures are used only in tests.

## Playing

- New match selects classical/full/cross boards, engine demos and a controller for
  every seat: Human, Random, First, Heuristic or MCTS.
- Select a piece to see engine-legal squares and exact action choices. Multiple
  actions can share a square: use the action list to disambiguate Move/Take/Attack
  and abilities. The inspector shows costs, effective tags and availability reasons.
- Select an ability to restrict targets. Transport selects a passenger before its
  destination. Scheduled effects and on-board-card targets are explicit choices
  in the action list; the drawers identify their event/card IDs.
- Cards display actual costs/effects and can be previewed/played. No-target cards
  have a direct action choice. Mandatory promotion appears in the inspector and
  prevents other gameplay until resolved.
- Preview runs on an isolated engine copy. Confirm commits the exact selected
  choice. Stale choices and duplicate submissions cannot change the new position.
- AI runs off the session lock. Pause/undo/restart invalidates pending results.
  Undo restores the previous human decision together with subsequent AI responses,
  and pauses AI. Resume it with the play button when appropriate.
- Human hotseat turns hide the hand behind a handoff screen. This is a local
  convenience, not a security boundary for someone with access to the computer.
- Rotate keeps world coordinates consistent; zoom changes the scrollable board size.
  Drag the empty viewport to pan, or use its scrollbars. Fit and Center selected work.
- Settings controls ordinary-move confirmation. Other actions always preview.

## Persistence

Save writes one local slot atomically. Load validates and restores it, then pauses AI.
Native storage is `cchess-save.json` inside Tauri's application data directory.
Browser development storage is `desktop/.local/cchess-save.json` (ignored by Git).
The browser and native clients have separate local slots. Save again to replace a slot.
Settings → Import save file accepts the same JSON format from another client.

The save contains the initial preset/controllers, version/content fingerprint and
lossless RON action transcript. Loading replays accepted actions through the engine;
it restores cards, RNG, scheduled work and pending promotion without accepting
arbitrary snapshots. Invalid saves leave the current match untouched. Save files
contain the full match history and are not intended to conceal private information.
The in-memory undo stack is not restored by loading.

## Checks

`cargo test --workspace` includes session regressions for actual opening moves,
preview purity, resource/turn changes, card costs, promotion, surviving captures,
AI undo, stale search results and persistence/failed-load rollback. Frontend tests
cover typed preview/commit, staged transport, promotion visibility, unselected cells,
complete piece labels and rectangular-board coordinate transforms.

### Watching a replay

Choose **Watch replay** to review the current match from its initial position. For a saved match, load or import it first, then choose Watch replay. Drag the timeline, click an action label, or use first/previous/next/final controls. Arrow keys and Home/End work in the timeline; playback supports 0.5×, 1× and 2× speed.

The solid board is the position after the displayed number of actions. Dashed translucent pieces and paths preview the **next** action. Affected squares are outlined; red `!` marks damage and `×` marks removal. The inspector describes non-spatial effects too. The overlay follows board rotation and zoom. Replay is read-only, and exiting returns to the untouched live match with AI paused.

Replay uses engine resolution and bounded history-free checkpoints. Rapid scrubbing is serialized and coalesced; the timeline only transfers nearby labels, even for long games. No additional raster assets are required.
