# CChess desktop GUI — design and implementation plan

Status: proposed design, with a minimal vector asset kit and illustrative mockup.
The desktop application is not implemented by this change.

## Product direction

Build a complete local desktop game with Tauri 2, React, TypeScript, Vite and
an SVG board. Keep rules and AI in the existing Rust crates. The first release
supports every currently implemented mechanic, all four board presets, mixed
human/AI seats, hotseat play, saving, loading and replay. Online multiplayer,
mobile distribution, a content editor and unimplemented rule systems are later
projects. Packaging first targets the current Linux development environment;
Windows and macOS builds require their own build and smoke-test runners.

Working visual direction: **Arcane Table**. A quiet charcoal tabletop, warm
stone squares, ivory typography, restrained brass accents and colored player
tokens. Fantasy character comes from the board and pieces; controls stay plain
and readable. Avoid decorative textures and ornamental fonts in the first release.

## Screen design

The included `match-concept.svg` is an illustrative composition, not an engine
position or a playable prototype. It demonstrates selection and hierarchy.

- Top bar: match menu, turn/round, current seat, mana, remaining actions, AI state.
- Center: board with coordinates, camera controls, last action and target overlays.
- Right inspector: selected piece, owner, health/damage, tags, statuses, abilities,
  exact costs, cooldowns, selection steps and preview consequences.
- Bottom: current viewer's hand; each card shows name, type, costs and an availability
  reason. On-board cards are a separate expandable shelf, not mixed into the hand.
- Secondary drawer: chronological action log, scheduled work, discard/consumed zones,
  rules reference and piece catalogue. These do not permanently shrink the board.
- End turn remains visible. Undo, AI pause, save and settings live in a compact toolbar.

At 1280×800, reserve approximately 300 px for the inspector and 150 px for the hand.
At 1024×768, use a narrower inspector and compact hand. Below that, switch the
inspector to a drawer and hand to a list. Preserve a usable board rather than
shrinking text. The application should allow resizing; 1024×768 is the initial
desktop QA target, not a gameplay restriction.

The board supports 8×8, 16×17, 14×14 cross and 24×24 cross layouts. Use the engine's
playable-cell mask, magic cells and build restrictions. Never infer cross corners
from an image. Render in layers: squares → terrain → last action → legal targets
→ pieces → status/health marks → preview ghosts → focus/selection.
Default fit-to-board; wheel/buttons zoom, drag empty background to pan, Home fits,
and a control centers the selected piece. On large boards, fit mode is an overview;
zoom for details. Keep token labels upright during rotation. Screen/world conversion
must respect north-positive engine coordinates and preserve coordinate labels.

Player identity uses both color and a seat marker: P0 gold/circle, P1 blue/diamond,
P2 violet/square, P3 coral/triangle. Board tokens use the existing unique two-letter
TUI codes; inspector and tooltips always give the full name. The provided tokens
are neutral source art; the application adds owner rings and seat markers.

## Interaction model

Click a piece to inspect it. Click a friendly actionable piece to see legal moves.
An action palette distinguishes Move, Take and Attack when a destination is shared;
never guess the action from the destination alone. Click an ability or card to enter
its targeting mode. Escape steps back without changing the game. Invalid input shows
the engine's reason and retains the selection.

Use an explicit UI state machine:

`Idle → PieceSelected → ActionSelected → ChoosingTarget(s) → Preview → Submitting`

Promotion, hotseat handoff, AI thinking and game over are separate blocking states.
Submissions disable duplicate activation. Clear stale selections after every revision.
Simple moves can submit on destination click; complex actions always preview first.
Provide a preference to confirm all actions. Drag/drop is optional polish; click and
keyboard interaction must provide complete functionality first.

| Engine intent / target | Required interface |
| --- | --- |
| Piece: Move / Take / Attack | Distinct markers and action label; source/destination selection |
| Ability: no target | Show affected set in preview, then confirm |
| Ability: square | Highlight legal choices; show affected area from engine preview |
| Directional ability | Eight/cardinal direction choices as appropriate; do not invent empty-square requirements |
| Transport | Choose passenger, then only destinations associated with that passenger |
| ScheduledAbility | Select eligible scheduled job, identify portal/source and clock |
| PlayCard: None / Square | Select card, choose square if needed, preview costs/effects |
| PlayCard: Scheduled | Select an eligible event in the timeline and highlight its source |
| PlayCard: Card | Select an eligible opposing on-board card |
| ChoosePromotion | Mandatory modal generated from legal promotion actions; no dismissal to gameplay |
| EndTurn | Explicit action; preview scheduled consequences before confirmation |

Availability is not merely an empty list of legal targets. Show why a piece/card
cannot act: wrong seat, acted, insufficient resources, missing unlock, cooldown,
protection, status restriction or no target. Add a structured core query if current
public queries cannot supply these facts; do not reimplement predicates in TypeScript.

Previews display costs, affected pieces, movement paths, damage/removals, summons,
status changes and turn transitions. Derive facts from `Game::preview`/resolved events;
where events lack presentation detail, extend a Rust preview projection using an
isolated clone. Never derive consequences by running a second rules engine in JS.
Clocks explicitly say “2 owner turns” or the appropriate clock unit, never seconds.

## Rust integration and ownership

Proposed workspace additions:

```text
session/                     cchess-session; game lifecycle and client projections
desktop/src-tauri/           Tauri binary, commands, platform file access
desktop/src/bridge/          generated wire types and typed invocation wrapper
desktop/src/features/       setup, match, board, inspector, cards, history, replay
desktop/src/ui/              shared controls and theme
assets/gui/                 source presentation assets; no gameplay RON changes
docs/gui/                   design, mockup, acceptance matrix
```

`cchess-session` depends on `cchess-core` and `cchess-ai`. Tauri depends on session.
The TUI may adopt shared lifecycle/history code incrementally; it must keep working
during extraction. React owns transient UI state only. A single Rust session owner
serializes mutations; no renderer or AI worker mutates the live `Game` directly.

Proposed wire operations (new APIs, not existing methods):

| Operation | Contract |
| --- | --- |
| new_match(config) | Validate preset, seat roster, policies and seed; return a new session ID and view |
| get_view(viewer) | Project public state and authorized private hand; include revision |
| get_choices(selection, revision) | Return legal intents/choice IDs, target stages and availability reasons |
| preview(choice, revision) | Read-only projection of consequences; reject stale choice |
| submit(choice, revision, request_id) | Validate caller/seat and intent; commit once; return view plus events |
| undo(revision) | Restore history checkpoint; invalidate AI and stale choices |
| set_ai_paused(value) | Pause scheduling and invalidate outstanding job generation |
| save / load / replay | Versioned, validated persistence; never expose filesystem writes to arbitrary JS paths |

Use decimal strings for Rust u64 identifiers, seeds and counters on the wire to avoid
JavaScript integer precision loss. Generate/export TypeScript DTO types from Rust,
then check schema drift in CI. Return structured error codes plus readable messages.
Keep raw internal `Snapshot` out of frontend state: it contains private cards and RNG.
Each response includes `(session_id, revision)`; request IDs make retrying a submission
idempotent. Invalid requests leave both state and revision unchanged. A stale preview
must be recalculated before the user confirms it again.

Start with a complete projected view after each action: these boards are small enough
to measure before optimizing deltas. Use the accompanying event list for presentation.
Animation may interpolate between old/new views, but completion or skipping always
lands on the authoritative new view. Disable gameplay input during a resolving
animation; save/quit remains usable. Reduced-motion mode applies the result immediately.

## AI, undo and hidden information

Run AI search on a cloned position in a bounded worker. Tag work with session ID,
revision and job generation. Accept a result only if all still match and that seat
is still AI-controlled. Undo, restart, load and pause invalidate job generations.
Pause must suppress results even if the board revision did not change. Add cooperative
cancellation/search budgets where needed; ignoring stale results alone must not allow
unbounded abandoned searches. Never hold the session lock throughout MCTS.

Expose existing Random, First, Heuristic and MCTS policies. Custom MCTS strength
settings require wiring validated search budgets into the current default search API;
they are not already provided by `AiController`. Show thinking, pause and resume states.
Keep AI-vs-AI playback paced independently of search speed and logical clocks.

Single-player undo returns to the previous human decision, including subsequent AI
responses. Keep atomic checkpoints so a promotion can also return to its pending
choice as the engine/TUI currently allow. Offer action-step undo in analysis mode.
Bound retained history by count and memory; document that load does not restore the
entire in-memory undo stack unless explicitly persisted.

Hotseat begins with private hands: use a handoff curtain between human seats and require
the incoming player to reveal their hand. Never flash it behind an overlay. Public
zone visibility follows an explicitly documented rules policy. Closed deck order and
other private hands stay out of views and logs. Analysis mode can explicitly reveal all
information. Local saves contain full state and are not a secrecy boundary.

## Saving and replay

The current serialized Snapshot is a foundation, not a complete save/load API.
Add a validated core restoration constructor that rebuilds occupancy and resolver
state, validates IDs, references, turn/promotion state, zones, scheduled work and
registry compatibility before replacing a live match.

Save envelope: format version, engine/rules version, content hash, match config,
full snapshot and optional replay history. Include AI configuration, RNG, timers,
statuses, deaths/revival claims and pending promotion. Bundle the exact supported
content with the app and reject incompatible saves clearly until migrations exist.
Write through a temporary file and atomic replacement; failed loads preserve the
current match. Add autosave at committed decision boundaries and recovery on launch.

Replay stores initial state/config plus accepted intents and the content/version
identity. Verify replay determinism against resulting state; checkpoints can accelerate
scrubbing later. Replay is read-only; “continue from here” explicitly creates a new
session. Hidden-hand replay defaults to the original viewer policy.

## Minimal asset kit

The included kit is deliberately usable without external art, fonts or downloads:

- 36 neutral SVG piece tokens, one per registered archetype, using unique TUI labels.
- One fallback token for future archetypes; catalogue coverage still fails CI when an
  expected archetype lacks an explicit mapping.
- One card back and three shared card faces: Summon, Utility and OnBoard. Real names,
  costs and rules text are rendered as HTML from engine content, never baked into art.
- Reuse the summoned piece token on summon cards; use text labels for utility effects.
- Theme tokens for surfaces, text, four owners and action states.
- CSS/SVG primitives for move dot, capture brackets, attack crosshair, direction arrow,
  preview ghost, magic diamond and blocked-tile hatch. These belong in board code.
- Text badges for Fire, Ice, Deactivate, Invulnerable, Undead, acted and cooldown.
  Show at most two on-board marks plus an overflow count; inspector lists all effects.
- A simple source app icon. Convert to platform packaging sizes during desktop setup.
- System sans-serif font; no music, portraits, raster textures or mandatory audio in v1.

All provided art is original geometric source generated by the included script; no
third-party asset licenses are introduced. It inherits the repository's licensing
policy; establish that policy before public distribution if it is still unspecified.
Owner tint and seat markers must be applied by the component, preferably by inline SVG
or a wrapper; CSS cannot reliably recolor `currentColor` inside an external `<img>`.
At fit-to-board scale on a 24×24 board, show compact labels and selection only; reveal
fine status details when zoomed or inspected. Test legibility at 24, 40 and 64 px.

## Delivery sequence and acceptance gates

| Stage | Work | Exit gate |
| --- | --- | --- |
| 0 — baseline | Record current workspace test result; catalogue all presets/intents; define visibility policy | Coverage matrix linked to engine fixtures; no unknown target forms |
| 1 — session | Introduce session owner, view/choice/preview DTOs, revisions, history and restoration API | Rust tests prove rejected/stale commands do not mutate state; TUI behavior preserved |
| 2 — shell and board | Tauri/Vite/React scaffold, bundled content/assets, SVG transforms, setup, inspector | Native window launches offline; all four layouts select correct world squares at any camera rotation |
| 3 — complete human play | All intent/target flows, hand/zones, preview, promotion, outcomes and handoff | Entire interaction matrix playable without CLI; one fixture per branch passes |
| 4 — AI and lifecycle | Worker scheduling, cancellation, pause, undo, restart, save/load, autosave, replay | Mixed seats and AI-only matches finish; stale results cannot commit after lifecycle changes |
| 5 — polish | Event animations, keyboard flow, reduced motion, catalogue, settings, help | No information lost when animations skipped; full game operable by keyboard |
| 6 — release | Native packaging, CI, asset/type coverage, representative performance profiling | Clean-machine offline install/load/play/save smoke tests pass on each claimed platform |

Dependencies: 1 precedes gameplay integration; 2 can use DTO fixtures until 1 is ready;
3 precedes final AI UX; save/load validation is a prerequisite for stage 4 completion.
Do not call a movement-only board feature-complete. Asset polish must not postpone
transport, scheduled targeting, card targeting or promotion.

## Verification and definition of done

- Run `cargo test --workspace` before integration and at release; maintain CLI/TUI tests.
- Session integration tests cover duplicate/stale requests, actor validation, preview
  purity, RNG preservation, hidden-hand projections and AI cancellation generations.
- Save tests cover pending promotion, scheduled summon, eliminated players, revival,
  invalid references, incompatible content and failed-load rollback.
- Frontend tests cover the targeting state machine, coordinate transforms, focus
  restoration and wire errors. Browser automation uses a deterministic bridge fixture;
  separate native smoke tests verify real IPC, packaging and file dialogs.
- Gameplay scenarios include all four board presets; Move/Take/Attack disambiguation;
  no-target, square and directional abilities; passenger/destination transport; all
  four CardChoice forms; scheduled haste/cancellation; promotion; elimination/draw;
  area/collision chains; on-board unlock consumption; undo after AI.
- Verify large boards at 1024×768 and 1280×800, high DPI, zoom, rotation and narrow mode.
  Never rely on color alone. Restore focus after dialogs; expose full piece/cell/action
  labels and a keyboard-accessible target list alongside the graphical board.
- Performance targets to measure, not promises: normal local command feedback under
  100 ms excluding search/heavy resolution; no UI freeze during MCTS; smooth board
  pan/zoom on the largest preset. Profile before adding memoization or render layers.
- Finish with an installed offline application that can create, complete, save, load
  and replay a match covering all current rules, with every archetype represented.

## Architectural references

- Existing engine seams: `core/src/commands.rs`, `core/src/rules.rs`,
  `core/src/abilities.rs`, `core/src/cards.rs`, `core/src/state.rs`.
- Existing interaction reference: `tui/src/lib.rs`; AI integration: `AI/src/lib.rs`.
- Tauri architecture: https://tauri.app/concept/architecture/
- Tauri process model: https://tauri.app/concept/process-model/

Choose dependency versions when implementation begins, validate the native prerequisites,
and commit the resolved lockfiles. This plan does not pin untested package versions.
