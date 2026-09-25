# Full CChess board

Start a complete game against an AI:

```sh
cargo run --manifest-path engine/Cargo.toml -- --full-board --ai random
cargo run --manifest-path engine/Cargo.toml -- --full-board --ai first
```

Two-player mode preserves the original `src/board.rs` CChess layout: a **16×17**
board, **46 pieces per side**, and magic squares **a8, a10, p8, p10**. It includes
all six classical archetypes and the starting army's Super Pawn, Crazy Pawn,
Shield Bearer, Builder, Cannon, Catapult, Ship, Tesla Tower, Archer, Ballista,
Ram, Warlock, Paladin and Magician. Walls are constructed or summoned during
play; Demonic reinforcements arrive through Portals. The legacy crate is not
needed to run this setup.

All playable squares are buildable, matching the original board. `~` marks an
empty magic square. `terrain` lists magic squares and playable/buildable counts;
`terrain a8` inspects a square even when occupied. Pieces, cards and both AIs use
the same terrain-aware legality checks.

For four-player FFA:

```sh
# One human and three AIs.
cargo run --manifest-path engine/Cargo.toml -- --full-board --four-player --ai first

# Two humans and two AIs; every seat is an opponent, not a team.
cargo run --manifest-path engine/Cargo.toml -- --full-board --four-player --players human,random,human,first --ai-seed 42

# Inspect the initial position without running AI.
cargo run --manifest-path engine/Cargo.toml -- --full-board --four-player --print
```

The full FFA setup uses a **24×24 cross** with four **4×4 corners removed**:
512 playable cells and 184 pieces. Each original 16-file army occupies one
four-rank arm, rotated to face inward. Eight empty magic tiles sit near the
central arena. This is a new four-player adaptation of the original two-player
layout; the smaller `--four-player` classical setup remains a 14×14 cross.

Every player starts with 1 mana, the normal income cadence, an independent
seeded starter deck and three opening cards. AI settings, terrain, board shape
and pieces are included in snapshots and deterministic replay. All ordinary
commands, previews and undo work. `--full-board` cannot be combined with demos.

API: `Game::full_board(registry, four_player)` creates the setup; follow it with
`with_ai(...)` for any combination of human, Random and First seats.
