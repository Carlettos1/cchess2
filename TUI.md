# Terminal interface

An interactive terminal opens the fullscreen interface automatically:

```sh
cargo run --manifest-path engine/Cargo.toml -- --full-board --ai random
cargo run --manifest-path engine/Cargo.toml -- --full-board --four-player --players human,first,random,first
```

All existing board presets and AI flags work. Use `--cli` for the original text
commands, `--tui` to explicitly request fullscreen, or `--print` for a board
snapshot. Piped input automatically uses the text interface.

| Key | Action |
| --- | --- |
| Arrows / HJKL | Navigate the board; large boards scroll with the cursor |
| Enter | Select your piece, then move to a highlighted square |
| Tab | Cycle legal destinations or targets |
| A | Open the selected piece's abilities (or the piece under the cursor) |
| C | Open your hand, including costs and unavailable cards |
| I | Inspect the hovered piece, statuses and cooldowns |
| T | Inspect scheduled events and their due clock ticks |
| P | Preview ending your turn |
| U | Undo your last action and subsequent AI responses |
| Space | Pause or resume AI |
| ? | Help |
| Esc | Cancel selection or preview |
| Q / Ctrl-C | Quit |

Menus use Up/Down and Enter. Abilities and cards highlight legal targets and show
a confirmation preview before spending anything. Transport selects its passenger
first and destination second. Promotion opens a mandatory choice menu. Ordinary
movement executes when you press Enter on a legal destination; U undoes it.
Up/Down scroll longer previews, help and inspection screens.

The cursor has a `>` marker, selected pieces have `*`, and legal destinations have
`+` (or red `x` for captures and attacks). Two-letter piece labels are distinct; hover and press I for the full name.
Player colors are gold, blue, purple and red for P0–P3. `~~` marks empty magic
tiles and `·` marks empty buildable tiles. Missing cross-board corners are blank.
The header shows the current player, mana, remaining actions, hand and deck sizes.
At 100 columns or wider a side panel adds piece details and recent actions.

The minimum terminal size is 40×16; 100×34 or larger is recommended. The board
viewport follows the cursor, so the full 24×24 FFA board works in smaller windows.
AI acts at most once every 250 ms, pauses while help/inspection is open, and has a
256-action safety pause that Space resumes. UI timing does not change engine
clocks or seeded randomness. The terminal is restored when quitting or on errors.
