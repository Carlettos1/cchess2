# Single-threaded Monte Carlo tree search

Select the policy with `--ai mcts`, or mix it with other controllers using
`--players`. Interactive `--ai` controls opponents; in auto-play it controls
**every seat**. Auto-play defaults to Random when no policy is supplied.

```sh
cargo run --release -- --ai mcts
cargo run --release -- --auto-play mcts-game.txt --ai mcts --ai-seed 42
cargo run --release -- --auto-play mixed-game.txt --players mcts,heuristic --max-actions 1000
cargo run --release -- --four-player --auto-play four-game.txt --ai mcts --max-actions 1000
```

Log paths must not exist. Reaching `--max-actions` writes an **UNFINISHED**
footer and exits unsuccessfully; it does not declare a winner or a draw.

## Search

`AI/src/mcts.rs` implements an arena of tree nodes with:

- UCT selection using each node's actual acting player's reward. Repeated
  actions by the same seat and four-player FFA require no alternating-sign
  assumption. Rewards use roster indexes, not player IDs.
- Progressive widening: at most approximately `2 * sqrt(visits + 1)` children
  are opened at a node before revisiting existing children. Large card and
  transport action sets therefore do not consume every iteration at the root.
- Seeded action ordering, with captures and promotions prioritized. End-turn
  is a candidate whenever legal, except while a promotion must be resolved.
- Bounded rollouts that usually prefer tactical actions, with 25% random
  exploration. Rules, cards, abilities, triggers and turn transfer all use the
  ordinary transactional resolver. Failed resolutions are discarded safely.
- A heuristic cutoff reward based on score change from the root position:
  `0.5 + 0.49 * delta / (1000 + abs(delta))`. Every seat has its own reward.
  Wins receive 1, losses/eliminations 0, and draws 0.5. A known immediate root
  win is selected without completing the remaining budget.
- Backpropagation of all seats' rewards and final selection of the most visited
  root child, breaking visit ties by mean reward and then expansion order.

The default is **128 iterations**, with at most **8 rollout actions** after each
selected/expanded node. Tree descent is additional work; this is an iteration
budget, not a millisecond deadline. Search returns counters for iterations,
nodes, rollout actions, failed resolutions and maximum tree depth.

For callers experimenting with budgets:

```rust
use cchess_ai::mcts::{search, SearchConfig};

let result = search(&game, SearchConfig {
    iterations: 512,
    rollout_depth: 12,
});
if let Some(action) = result.action {
    game.apply(action)?;
}
```

Zero iterations or a terminal position yields no action. Repeating a search with
identical state, seed and settings yields the same choice. Search uses a separate
RNG and never mutates the live game. No threads, worker pools, async search tasks,
or parallel rollout libraries are used. The engine also remains single-threaded.

This is full-state MCTS: simulated card draws and other random outcomes use the
actual game RNG. It is **not an information-set search** and can benefit from
hidden state. The fixed heuristic and short rollouts are approximations; this
work establishes correctness, reproducibility and execution cost, not a measured
playing-strength advantage over every existing policy.

## Engine optimizations

`Game::simulation()` creates an isolated `Simulation` that shares compiled
content and copies live state without committed history. It uses the same
resolver and rollback semantics, preserves action IDs and RNG state, and does
not accumulate speculative traces. `Game::action_count()` keeps AI seeds and
future event IDs correct despite the omitted history. Simulation snapshots are
not complete save/replay files; only the live game supplies those.

Both MCTS and Heuristic use this path, as does `Game::preview`. Leaf evaluation
computes material and immediate threats once for all seats. Ordinary move
enumeration reuses eligibility and random-direction checks across targets;
submitted actions are still validated fully. Geometry and target unions use
sorted, deduplicated vectors, retaining the original canonical order. Local
range queries scan their bounding rectangle instead of the whole board.

## Reproducing measurements

Compile before timing so compilation does not contaminate the results:

```sh
cargo build --release --offline -j 1
python3 scripts/benchmark_mcts.py --repetitions 3
```

The script runs sequential `cargo run --release --offline -j 1 -- --auto-play`
commands and retains stdout, stderr, complete game logs, hashes and JSON results.
An optional `--baseline-executable PATH` enables alternating baseline/optimized
runs. No concurrent benchmark processes are launched. On Linux it samples the
engine's `/proc` thread count. Runtime telemetry separates AI selection time
from total auto-play time. The latter includes logging and final serialization,
but excludes initial asset loading and game setup. JSON also records command
wall time (including Cargo startup). Short capped runs are explicitly identified
as unfinished, never reported as complete games.

## Measured results (2026-09-25)

AMD Ryzen 7 5700G, Linux x86-64, Rust 1.98.0-nightly
(`4c9d2bfe4 2026-07-01`), release profile, seed 42, 128 iterations and
8 rollout actions. Logs were enabled on `/tmp` tmpfs. Three alternating,
sequential runs per version/workload; medians below exclude compilation and
Cargo startup. No CPU pinning or frequency control was applied.

The baseline is the first MCTS implementation in this task, before the engine
optimizations described above; it is not the Random or Heuristic policy.

| Workload | Outcome | Before | Optimized | Less elapsed time |
| --- | --- | ---: | ---: | ---: |
| Two-player classical | Completed, 40 actions | 2.553 s | 2.124 s | 16.8% |
| Two-player full board | Unfinished, 20-action cap | 5.651 s | 4.351 s | 23.0% |
| Four-player classical | Unfinished, 20-action cap | 2.496 s | 2.042 s | 18.2% |

All six logs for each workload (three before, three after) have the same
SHA-256, including full action traces and final snapshots. Every sampled
optimized engine process had **one thread**. The bounded full-board and
four-player samples measure prefix cost only; they are not full-game timings.

[Raw timing samples, log hashes and binary fingerprints](benchmarks/mcts-2026-09-25.json).
The initial baseline executable and detailed measurement logs were retained in
`/tmp/cchess-mcts-bench/` for this session; the script reproduces current-build
measurements without depending on them.
