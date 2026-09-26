# Basic AI opponents

Four policies share the normal legal-action generator, validation and
resolver. Random and First have no evaluation function or search. Heuristic adds
one-action lookahead; none of the policies coordinates teams. Four-player matches remain FFA regardless of seat types.

- **Random:** uniformly chooses among all distinct legal card plays, captures,
  attacks, abilities, moves and required promotion choices. Each target or
  transport destination is a distinct action. Pass is selected only when no
  actionable choice exists, so it cannot randomly abandon a playable turn.
- **First:** checks categories in order: cards → Take → Attack → abilities →
  Move → remaining required choices/pass. It returns as soon as a category has
  a legal action; it does not generate or rank lower categories. Mandatory
  promotion is handled immediately because other actions are illegal then.

- **Heuristic:** resolves each candidate on a cloned game and scores the resulting
  position. It prioritizes wins/survival, material (including custom piece stats
  and abilities), remaining health, statuses, mana, hand/on-board card counts,
  central placement, promotion progress, and ordinary capture threats from the
  next acting seat. All opponents count against the AI in FFA. It also considers
  ending the turn when legal, even if other actions remain. Equal scores retain
  candidate order; the live game and its RNG are never changed by selection.

- **Mcts:** seeded Monte Carlo tree search with UCT selection, progressive
  widening, tactical rollout ordering, and a heuristic cutoff evaluation. The
  default budget is 128 iterations and up to 8 rollout actions per iteration.
  Each node selects for its actual acting seat, including extra-action turns
  and four-player FFA. Search and simulation run on the caller's single thread.
  See [MCTS implementation and benchmarks](MCTS.md).

```sh
cargo run -- --ai heuristic
cargo run --release -- --ai mcts
cargo run -- --four-player --players human,heuristic,heuristic,heuristic
```

The implementation and tunable weights live in `AI/src/heuristic.rs`.
`cchess_ai::heuristic::evaluate(&game, player)` exposes the position score.
This is a hand-tuned one-action heuristic, not a deep search: it does not plan
multi-action combinations or model every future ability/scheduled effect.
Threat scoring estimates pressure, not guaranteed damage or a forced capture.
It scores card counts rather than opponent card identities, but simulations use
the actual engine state and RNG, so this is not an information-safe policy for
competitive hidden-hand play. Simulation copies omit committed history, but resolving every candidate is
still more expensive than First/Random, particularly on full boards.

All policies re-evaluate the board after every action, continuing through zero-action
cards and extra movement budgets until the turn transfers or the game ends.
First-policy ties use hand order, then entity-ID order and existing rule/target
order. It does not estimate whether an action is strategically useful.

For complete classical + starting armies and magic/buildable terrain, add
`--full-board`; see the [full-board guide](FULL_BOARD.md). It combines with every
AI seat option and with `--four-player`.

## Playing

Run from the repository root:

```sh
# Human seat 0 against one random opponent.
cargo run --manifest-path Cargo.toml -- --ai random

# Human seat 0 against three First opponents, all playing FFA.
cargo run --manifest-path Cargo.toml -- --four-player --ai first

# Two humans and two different AIs, still FFA (not teams).
cargo run --manifest-path Cargo.toml -- --four-player --players human,random,human,first

# Explicit seats and a repeatable random seed.
cargo run --manifest-path Cargo.toml -- --players first,human --ai-seed 123
```

`--players` takes one comma-separated controller per seat in roster order:
`human`, `random`, `first`, `heuristic`, or `mcts`. Unconfigured games remain human-controlled.
`--ai` assigns all seats except seat 0 to the named policy. Use either `--ai`
or `--players`. The seed defaults to 42. These options also work with demos;
`--print` prints setup without running AI.

The CLI runs AI seats automatically until a human seat or game-over is reached.
It prints each AI action. `undo` restores the previous human-command snapshot,
including any subsequent AI responses. To keep all-AI games, eliminated-human
matches or unusual custom content responsive, it pauses after 256 consecutive
AI actions; enter `ai` to resume, or inspect the board, undo or quit.
A resolver error pauses the AI and reports the error instead of hiding it or
spending resources on a failed action.

## Engine API and reproducibility

`Game::with_ai(seats, seed)` configures controllers before play. Missing seats
are human. `choose_ai_action()` is read-only; `play_ai_step()` applies one normal
intent; `play_ai_turn(max_actions)` continues only the current turn up to the
caller's work budget. Reaching that budget pauses without forcing a pass or
losing remaining actions. Human seats and finished games produce no AI action.

Controller policy and seed are serialized in the snapshot. Random selection
uses a separate deterministic stream derived from that seed, seat and committed
action index. Inspecting choices or previews does not consume randomness or
change the deck/Crazy Pawn RNG. Ordinary logged intents replay through
`Game::apply`, reproducing snapshots including controller configuration.

## Quiet automated games and action logs

Run every seat as AI, without prompts or intermediate board output. The default
is Random; `--ai POLICY` sets every seat, and `--players` permits mixed AI seats:

```sh
# MCTS, classical board; omit the action limit for an unbounded match.
cargo run --release -- --auto-play mcts.txt --ai mcts --max-actions 1000

# Two players, classical board
cargo run --manifest-path Cargo.toml -- --auto-play game-2p.txt
# Four players, cross-shaped FFA board
cargo run --manifest-path Cargo.toml -- --four-player --auto-play game-4p.txt
# Full armies, buildable tiles and magic tiles
cargo run --manifest-path Cargo.toml -- --full-board --four-player --auto-play full-4p.txt --ai-seed 123
```

`--auto-play LOG_FILE` runs until the engine declares victory or a draw, prints
only the final board/state to stdout, and reports completion and the log path to
stderr. It bypasses the TUI and the interactive 256-action pause. The file must
not already exist; use a different name for each run. Its parent directory must
exist. The default AI seed is 42.

The UTF-8 text log contains the initial snapshot, seed and board dimensions,
numbered actions with their turn numbers, every resolved intent and resulting
event in pretty-printed RON, the final board and final snapshot. Each action is
flushed as it is recorded. Coordinates inside RON use zero-based `x`/`y` values.
The log records moves, attacks, abilities, cards, promotion, passes and triggered
effects, including actions that were interrupted during resolution.

Optionally use `--max-actions 10000` to bound an experiment. Hitting this limit
writes an **UNFINISHED** footer and current state and exits unsuccessfully; it
does not invent a winner or a draw. Without that option there is no action limit;
random games are not guaranteed to terminate. Engine errors also preserve the
recorded actions and produce an unfinished result.

This mode supports all board presets. `--players` must assign an AI to every
seat; human seats are rejected. `--ai` and `--players` are mutually exclusive.
It cannot be combined with `--tui`, `--print` or asset/deck validation. `--cli`
is accepted but unnecessary. Stderr reports auto-play elapsed time, total AI
selection time, milliseconds per action, and actions per second.
