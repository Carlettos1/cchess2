# Desktop implementation review

Reviewed 2026-09-25 against the current uncommitted `desktop/` tree.
This is a diagnostic review; application behavior has not been changed.

## Conclusion

The desktop application is a visual prototype. It does not interface with the Rust
game. This explains the unexpected gameplay: its legal moves and commits are a
second, incomplete implementation inside React. The existing Rust engine tests pass,
but none of those rules execute when a user clicks this GUI.

The native Rust binary only starts a Tauri window. It has no core/AI dependencies,
managed game session or registered game commands. The frontend always constructs
`DemoBridge`, including when packaged as a native application.

## Findings, in repair order

### P1 — Connect the desktop bridge to the engine

Locations: `desktop/src/bridge/index.ts:17`, `desktop/src-tauri/src/main.rs:4`,
`desktop/src-tauri/Cargo.toml:11`.

`GameBridge` has only `newMatch` and `getView`, both returning fixtures. There is no
API for legal choices, preview, submit, end turn or AI execution. Introduce a Rust
session owner and real IPC implementation before treating this client as playable.
Keep fixtures explicitly selectable for UI development; never silently use them
as a native gameplay backend.

### P1 — Replace frontend move generation and commits

Locations: `desktop/src/App.tsx:30`, `desktop/src/App.tsx:55`.

Every archetype gets the same four orthogonal single-square destinations. At the
opening position, b1 Knight incorrectly has no moves and e2 Pawn offers only e3,
not e4. These were reproduced through the running browser interface.

Confirm removes the destination occupant unconditionally and changes the selected
piece's position. It does not resolve Tough, protection, Attack-vs-Take, costs,
promotion, death effects or victory. It increments a renderer revision but never
updates the bridge's stored view. Selected action identity is reconstructed by
parsing an English notice string, rather than keeping a typed pending choice.

Use core `legal_actions`, `preview` and `apply` through the session. Return the
authoritative updated view plus events, and retain a typed choice/revision across
preview and commit. Do not repair each archetype's movement separately in JavaScript.

### P1 — Wire turn progression and active-looking controls

Locations: `desktop/src/App.tsx:62`, `desktop/src/App.tsx:100`,
`desktop/src/App.tsx:111`, `desktop/src/features/inspector/Inspector.tsx:13`.

After confirming e2→e3, the UI still shows P0, Turn 12, Actions 1. More friendly
pieces can act in the same nominal one-action turn. End turn, Undo, Save, Settings
and Center selected have no click handlers. The AI button only toggles its icon;
there is no AI scheduling. Enabled ability buttons have no handlers either.

End turn, Undo and Save were clicked in the running UI with no state change.
Connect these operations to real commands; until implemented, disable them with
an explicit availability explanation. Turn owner, resources and AI status must be
derived from the returned session view, including four-player seats.

### P1 — Complete card and continuation flows

Locations: `desktop/src/App.tsx:66`, `desktop/src/bridge/types.ts:5`.

Choosing a card clears the board targets and displays “Choose a legal target”, but
does not retain the card or generate targets. There is no card preview/commit path.
The application also lacks promotion, transport passenger/destination selection,
scheduled-job selection and opposing on-board-card selection. These are existing
engine actions, not optional polish.

Extend the bridge and interaction state machine for all ActionIntent/CardChoice
variants. Store the selected action/card and stage explicitly. No-target cards must
proceed directly to preview rather than instructing the user to pick a square.

### P2 — Replace misleading fixture data with content projections

Location: `desktop/src/bridge/fixture.ts:13` through `makeDemoMatch`.

- A new match begins at fictional Turn 12 with made-up history and resources.
- Magic cells are assigned using `(x + y) % 13`, not engine terrain.
- Full boards reuse small classical armies with two extra pieces. Four-player
  rosters/seat orientations do not match the engine presets.
- Forced March is shown as free and resetting a piece's acted state. Its RON
  definition costs 1 mana and grants 2 actions; Second Wind resets acted state.
- Summon Knight is shown costing 1 mana; its definition costs 2 mana and 1 action.
- Fire's same card instance is in both the hand and on-board arrays.
- King teleport says insufficient mana even though P0 has 3 mana and its shown cost
  is 2. Dragon's displayed tags omit its actual Immune/Heroic protections.

Build gameplay views from the registry and current engine state. Keep illustrative
fixture content isolated and visibly labeled as such in development mode.

### P2 — Empty squares appear selected without a selection

Location: `desktop/src/features/board/Board.tsx:36`.

`selected === piece?.id` is true when both values are undefined. After reloading
the classical board without clicking anything, the DOM contains 32 selected cells:
every empty square. Guard on an actual selected ID and occupied cell.

### P2 — Board rotation leaves coordinate labels behind

Locations: `desktop/src/features/board/Board.tsx:30` and `:48`.

Only the square grid rotates; file/rank labels remain in the original orientation.
After one clockwise rotation, actual a1 is at the top left beside rank label 8.
This was verified against rendered cell/label positions in the browser.

Compute cell and edge-label positions from one camera transform. At quarter turns,
swap the displayed width/height for rectangular boards, preserve upright labels,
and test screen-to-world selection at all four orientations.

### P2 — Piece and unavailable-card rendering reduces legibility

Locations: `desktop/src/features/board/Board.tsx:42`,
`desktop/src/styles.css:43`, `desktop/src/styles.css:44`.

Piece SVGs already contain abbreviations, but the board draws a second abbreviation
over the image; the browser shows doubled text and rings. Boards wider than 16
also truncate the foreground label to one letter, making King/Knight and other
archetypes ambiguous. Render one complete unique abbreviation per token.

The global `.unavailable` rule sets `display:flex` for ability-reason text. It also
matches the entire unavailable Fire card, laying its title, description and reason
out sideways and clipping them. This malformed card is visible in the running UI.
Scope the reason style to the inspector or give it a distinct class.

## Verification performed

Passed:

- `cargo test --workspace` — core, AI, CLI and TUI tests.
- `npm test` in `desktop/` — 5 existing fixture tests.
- `npm run build` in `desktop/` — TypeScript and production frontend build.
- `cargo check --offline --manifest-path desktop/src-tauri/Cargo.toml` — native
  shell compiles. This is not evidence of functioning native gameplay.

Browser checks used the actual Vite application: initial board, Knight selection,
Pawn selection, preview/confirm, unchanged turn/resources, inert End turn/Undo/Save,
reload selection count, rotated coordinates and rendered token/card legibility.
Native window interaction and release packaging were not tested.

The five desktop tests only cover preset cell masks and the shape of fixture player
objects. They do not exercise an action, a turn transition, an AI response or IPC.
Passing builds therefore do not contradict these findings.

## Required regression gates

1. Opening Knight exposes a3/c3 and Pawn exposes e3/e4 through the real engine.
2. Preview is pure; commit spends resources and transfers the turn exactly once.
3. Take against a surviving Tough defender leaves the attacker in place; Attack
   never relocates; protected targets remain protected.
4. Promotion blocks other actions until resolved; transport and all card targets
   can be completed from the UI.
5. Undo restores a human decision and subsequent AI responses; stale AI results
   cannot apply after undo, restart, load or pause.
6. No selection produces zero selected cells; a piece selection produces one.
7. All rotation/rectangular-board combinations agree between labels and clicked
   world coordinates; full piece labels remain distinct on large boards.
8. Every advertised enabled control produces its advertised result. Test real
   native command wiring separately from frontend fixture/component tests.

Repair the Rust session/IPC and real action lifecycle first, then the missing action
flows, lifecycle tools and UI defects. The existing implementation plan still applies;
the current implementation has completed only part of its shell/layout stage.

## Repair follow-up — 2026-09-26

The reviewed prototype has been replaced with a live engine client. `cchess-session`
now owns authoritative state and serves native Tauri commands and the Vite development
bridge. React no longer generates moves or edits game pieces directly. Revisions
protect preview/submit and AI generation checks protect pause/undo/restart.

The reported action/turn/card/ability gaps now use engine choices, including promotion,
transport and non-square targets. Setup uses actual presets and configurable seats.
Save/load uses an atomic local slot with validated deterministic replay. Availability
and card descriptions are projected from engine rules and content. Empty-cell selection,
rotated coordinates, duplicated labels and the unavailable-card CSS collision are fixed.

See `desktop/README.md` for the current controls, storage locations and test coverage.
The findings above document the old implementation and remain as regression scenarios.

Final validation: 327 Rust workspace tests and 14 frontend tests passed, and the
native release executable built with `tauri build --no-bundle`. Live browser checks
used the same Rust session: opening knight targets, pawn double move, preview/commit,
AI turn, grouped undo, Forced March resource/hand changes, mandatory promotion, and
saving/loading the pending promotion. Native-window interaction and installer
packaging were not exercised; this build check produced the executable only.
