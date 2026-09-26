# Single-threaded optimization plan

For the new MCTS policy, simulation-state optimizations, and current release
timings, see [MCTS implementation and benchmarks](MCTS.md). The Random-policy
measurements below are historical and refer to a different workload.

The prioritized optimizations are now implemented. Selection, resolution and logging
still run on one thread. Rules, deterministic ordering, atomic rollback, snapshot
format and complete readable logs are preserved.

A follow-up [analysis of the supplied four-player seed-650 game](ANALYSIS_6504V4.md)
includes complete-game timings, action statistics and the next targeting priorities.

## Implemented results

The primary `--full-board --auto-play` benchmark (seed 42, release build) improved
from a **0.850-second median to 0.139 seconds**, approximately **6.1× faster**.
Five optimized runs took 0.135–0.140 seconds on the same machine and workspace
filesystem as the baseline below. Each completed the same 833 actions; the entire
3,072,408-byte log has the same SHA-256 as the original.

| Workload | Original | Optimized | Result |
| --- | ---: | ---: | --- |
| Two players, seed 42 | 0.836 s | 0.138 s | Completed, 833 actions |
| Two players, seed 7 | 0.789 s | 0.145 s | Completed, 838 actions |
| Two players, seed 123 | 1.254 s | 0.200 s | Completed, 1,173 actions |
| Four players, seed 42, 2,000-action prefix | 3.523 s | 0.498 s | **Unfinished in both builds**, same state at limit |

These paired timings are single runs, separate from the repeated seed-42 median.
For every row, the original executable and optimized executable produced
byte-for-byte identical logs, including all events and final snapshots. The
four-player prefix is a scaling/compatibility check, not a completed-game result.

Implemented changes:

- Shared eligibility checks reject unavailable piece actions, abilities and cards
  before expanding targets. Enumeration reuses resolved target lists; transport
  checks its passenger once before expanding destinations. Full validation of
  submitted actions and revalidation after movement-start effects remain intact.
- A derived occupancy table replaces scans in `Game::at`. Insertion, removal and
  movement update the table centrally; transactions copy and commit it atomically.
  It is not serialized. Snapshot restoration must rebuild it. Debug builds check
  its consistency after commits, and tests compare it against piece positions.
- Transactions use `Snapshot::clone_for_resolution` to copy live fields without
  the historical log. A successful commit transfers the original history and
  appends the new action. Failure leaves the original game untouched. This avoids
  a public snapshot/API redesign while removing repeated history copies.
- AI deduplication uses a membership tree with borrowed keys, retaining first-seen
  order in the output vector. Target filters share a single piece lookup per
  candidate. No RNG consumption or canonical ordering changed.

The same temporary phase harness now measures approximately 92.3 ms selecting,
28.4 ms applying, 6.7 ms serializing actions and 2.7 ms writing/flushing, plus
5.0 ms serializing the final snapshot. These are single instrumented-run timings,
not independent speedup guarantees for each change. Selection remains the largest
phase, but apply no longer rises steadily over the sampled 200-action blocks.

Validation includes 296 passing tests, clean all-target clippy, reference action
list comparisons across full boards and demos, and a frozen pre-optimization
full-game trace fingerprint. Resolution-limit rollback is also checked with
previously committed history present. The original plan below records the
reasoning and remaining options; it should not be read as a claim that every
optional experiment or profiler counter has been implemented.

## Benchmark and measured baseline (before optimization)

The primary workload is the complete, two-player, full-army Random-AI game:

```sh
cargo build --release --manifest-path engine/Cargo.toml --offline
engine/target/release/cchess-engine --full-board --auto-play game.txt --ai-seed 42
```

The log filename must not already exist. Build before timing; do not include
compilation in the measurement. Keep logging enabled: it is part of this workload.
No `--max-actions` is used in a completion benchmark. An action-limit stop or error
is an unfinished run, never a fast completed result.

Measured on 2026-09-23, AMD Ryzen 7 5700G, Linux x86-64, Rust
`1.98.0-nightly (4c9d2bfe4 2026-07-01)`, using the existing Cargo release profile.
Game executions were sequential. No CPU pinning or frequency control was applied.
The working tree contains implementation beyond commit `639ff6a`; that commit
alone does not identify this baseline. Use the recorded output hash as an
additional behavior check.

| Measurement | Result |
| --- | --- |
| Board / starting army | 16×17, 92 pieces, two Random AI seats |
| Seed / result | 42 / completed after 833 actions |
| Release, logs on workspace ext4, 3 runs | 0.843, 0.850, 0.855 seconds |
| Release median, workspace ext4 | **0.850 seconds**, approximately 980 actions/s |
| Release, logs on `/tmp` tmpfs, 5 runs | 0.831–0.874 seconds; median 0.850 seconds |
| Debug, workspace ext4, 1 run | 16.068 seconds |
| Complete text log size | 3,072,408 bytes |
| Final snapshot text size | 1,789,025 bytes; includes the history again |

All these runs produced identical log bytes, including debug versus release:

```text
SHA-256: 1ea485007b8247bc796c70dce40b6ce29f649adf8d0739bf9d3fc45382824d24
```

Release was approximately 19× faster than debug in this comparison. This is a
build-mode difference, not an implemented algorithmic improvement. Always evaluate
engine optimizations in release mode. The small sample is a local baseline, not
a portable latency guarantee. The ext4 writes used the operating system's page
cache; the application flushes its buffer but does not request durable disk sync.

### Where the measured time goes

A temporary, optimized harness used the same full-board setup, controllers and
seed. It timed `choose_ai_action()`, `apply()`, pretty-RON serialization and
buffered writes/flushes separately for every action. It completed the same 833
actions and recorded 7,611 events. Output went to `/tmp` tmpfs. It omitted CLI
startup, the initial snapshot and some text formatting, so these numbers are
phase measurements, not a second end-to-end benchmark.

| Phase | Total measured time | Interpretation |
| --- | ---: | --- |
| Random AI choice, including legal-action enumeration | 679.7 ms | About 81% of the measured phase total |
| Apply selected actions | 142.9 ms | About 17%; includes validation, cloning, clocks and effects |
| Serialize individual actions | 10.4 ms | About 1.2% |
| Write and flush individual actions | 3.0 ms | Small on tmpfs; storage dependent |
| Clone final snapshot | 0.18 ms | One-time cost |
| Serialize final snapshot | 5.0 ms | One-time cost |

The first 200 actions spent 291.4 ms choosing and 18.4 ms applying. Actions
601–800 spent 75.1 ms choosing and 48.8 ms applying. Selection became cheaper as
the game evolved, while application became more expensive. Growing history is a
plausible contributor to the latter, supported by the copying described below;
these timings alone do not isolate its cost from changes in game state.

This was coarse phase instrumentation, not a sampled function profile or allocation
profile. No measured percentage can yet be assigned to occupancy lookup, target
resolution, deduplication or cloning individually. Peak memory was not measured.
The instrumentation was temporary and is not a new shipped benchmark command.

## What the baseline code did

| Location | Baseline behavior | Optimization opportunity |
| --- | --- | --- |
| [`ai.rs`](src/ai.rs), `ai_legal_actions` | Enumerates all hand-card choices and all current-player piece actions; deduplicates using `Vec::contains` | Reduce repeated enumeration work and quadratic deduplication |
| [`rules.rs`](src/rules.rs), `legal_actions`, `validate_piece` | Resolves a rule's target list, then validates each candidate; validation resolves the target list again | Resolve once per rule and board state |
| [`abilities.rs`](src/abilities.rs), `legal_abilities` | Generates targets and transport destination combinations before validating each intent | Check common eligibility before expanding choices; share target work |
| [`cards.rs`](src/cards.rs), `legal_card_plays` | Generates card targets before rejecting unplayable candidates through validation | Check ownership, zone, costs and common conditions first |
| [`state.rs`](src/state.rs), `Game::at` | Scans all pieces for a position | Add a direct occupancy index |
| [`targeting.rs`](src/targeting.rs), `resolve` | Builds origins, resolves patterns, scans occupancy, filters, collects a `BTreeSet`, then a vector | Reuse occupancy, avoid repeated filtering and temporary allocations |
| [`rules.rs`](src/rules.rs), `apply` | Clones `Game` to resolve transactionally | Clone mutable live state without copying prior action history |
| [`state.rs`](src/state.rs), `Snapshot` | Contains the entire `Vec<ResolvedAction>` | History grows with the game and is copied by each transaction |
| [`main.rs`](src/main.rs), `automated_game` | Serializes and flushes every action, then serializes a final snapshot containing history | Later reduce temporary strings or duplicate output, if measured worthwhile |

`Game` already shares the immutable registry through `Arc<Registry>`; cloning
compiled content is not the issue. `Arc` here does not imply parallel execution.
`preview()` clones a game and calls `apply()`, which clones again, but this is not
on the automated AI path. Optimize it for UI responsiveness separately, rather
than crediting it with improving this benchmark.

## Implementation rationale and remaining options

### 1. Establish profiling and behavior checks

Keep the current implementation as the reference. Add optional timing/counter
instrumentation for:

- Action choice, validation, transactional copy, effect resolution and serialization.
- Target resolutions, occupancy lookups, generated candidates, accepted actions
  and duplicate actions.
- Live piece count, hand size, scheduled work count, retained event count and
  action index, sampled in blocks of 100–200 actions.
- Allocations and peak resident memory, using a profiler available on the host.

Measure instrumented and uninstrumented binaries separately. A counter on every
lookup can distort the hot path. A function/allocation profile should determine
whether repeated targeting or board scans offer the larger next improvement.

Keep a golden seed-42 trace and compare actions, events and final snapshots after
each change. Do not optimize by changing the Random policy to First or by sampling
pieces instead of actions: either changes the requested workload and game behavior.

### 2. Reject unavailable actions before expanding targets — implemented

This was the first code change: small scope, directly on the dominant path.
Extract shared eligibility checks from the existing validators, then call them
before target enumeration. For example, a card with insufficient mana needs no
board-wide target search; an unavailable ability needs no transport combinations.
Ordinary actions by an already-acted piece can be rejected early.

Keep eligibility specific to the action or ability. Do not skip every ability of
an acted piece unless its actual rules prohibit it. Ice permits attacks,
deactivation permits movement, and special ability conditions may differ. Cards,
promotion and pass behavior must retain their existing semantics.

Once eligible, resolve targets once per rule. Use an internal validation helper
that accepts the already-resolved target information for candidate enumeration.
Continue to run full validation on externally submitted intents and on commits.

Crucially, preserve the **second validation after movement-start clock effects**
in `apply()`. Those effects can invalidate an action that was legal when selected.
A cache from before a clock tick is not evidence that the action remains legal.

Acceptance: exact ordered legal-action lists on reference positions, identical
seeded games and improved selection time. Share validation logic rather than
maintaining a separate, approximate set of AI rules.

### 3. Add a direct board occupancy index — implemented

The original `Game::at` cost a scan of up to all live pieces. It is called repeatedly
by ray traversal, target filters, movement checks, abilities and effects. A dense
board-sized table of `Option<EntityId>` gives constant-time occupancy queries;
looking up the resulting piece can still use the existing entity map initially.

Retain stable entity IDs and the piece map. Maintain the index centrally for moves,
swaps, transport, spawning, captures, deaths, revival and elimination. Treat it as
derived runtime state, rebuilding it on loading snapshots if it is not serialized.
Include its updates in rollback behavior. Excluded cross-board corners must remain
unplayable even though their occupancy entry is empty.

Add an invariant comparing every index entry with the authoritative piece positions
after actions and after failed resolutions. This optimization should not change
iteration order: current positions sort by **x, then y**, which is not the usual
row-major array traversal order.

After measuring lookup gains, consider a reusable board-sized target bitmap instead
of allocating a tree for every query. Emit positions in the existing canonical
order, not whichever order is most convenient for the bitmap implementation.

### 4. Stop cloning historical actions during transactions — implemented

Keep the existing atomic resolution model initially. Split internal storage into
mutable live state and append-only history. Clone only the live state into the
working transaction; append the resulting action to history after successful
resolution. Failed resolutions must leave live state, RNG, counters and history
unchanged.

Originally, the nth action copied the earlier log as part of `Game::clone()`. For a game
with roughly constant event size per action, the cumulative history-copy component
is quadratic in the number of actions. Separating history addresses that growth
without immediately introducing a complex make/unmake system.

Preserve the public `log()` view, undo, replay, snapshot equality and serialized
snapshot contract. A snapshot can still include history for compatibility, while
the internal transaction does not. A borrowed serialization view can later avoid
cloning just to save. Death records, scheduled work and RNG are live state, not
history that can casually be excluded from the transaction.

Do not simply wrap the whole history vector in `Rc`/`Arc` and mutate it using
copy-on-write: appending while both transaction and original own it can still copy
the entire vector. Share immutable chunks or keep the append-only log outside the
transactional copy.

The RNG decision and action IDs currently depend on `log().len()`. If a later
streaming mode drops retained history, introduce a serialized committed-action
counter and use it consistently for seeds and IDs. Merely clearing the vector
would break replay. Keeping history outside the copied state does not itself
require dropping any history.

### 5. Reduce candidate allocations and deduplication — implemented in part

After the previous steps, re-profile `ai_legal_actions`. Its repeated
`unique.contains()` can require quadratic comparisons in candidate count.
Use a membership set while appending first-seen actions to the existing vector,
or prove and enforce uniqueness in the generators. Define keys with all action
fields, including card identity, source, target, transport destination and
scheduled-event identity.

Never iterate an unordered set to choose the random action. Preserve the original
ordered vector and the existing random-index draw. Reordering or sorting the
choices can change the entire seeded game even if the legal set is unchanged.

Reusable scratch vectors and direct enum/interned identifiers can reduce allocation
and string copying. Start with the measured hot allocations; a wholesale entity
storage or ECS rewrite is not justified by the current evidence.

Reservoir sampling is a possible single-pass, uniform selection strategy, but the
usual implementation changes RNG consumption and seed-to-action mapping. It also
still needs every legal choice and duplicate handling. Defer it unless a versioned
change to deterministic selection is explicitly acceptable.

### 6. Revisit logging and smaller costs only after measurement

Keep the complete readable log. Serialization and write/flush work were small in
this benchmark, so neither a logging thread nor removing the log is the right first
step. Possible later changes are:

- Serialize directly into the buffered writer rather than allocating a complete
  formatted string for each action, if the serializer interface supports it.
- Make per-action versus periodic flushing an explicit option. Periodic flushing
  changes how much recent output may be lost on abrupt termination; ordinary
  `flush` is not a durable disk sync in either mode.
- Avoid repeating all actions inside the final snapshot by defining a versioned
  live-state checkpoint format. Keep initial state and the complete action stream;
  update readers and replay tests before changing the existing log contract.
- Reuse temporary target buffers, make simple tag membership checks without building
  a full tag set, and index scheduled work by clock/owner if profiling supports it.
  Preserve simultaneous-event priorities and stable ordering.

Try thin LTO, a lower codegen-unit count or host-specific code generation only as
separate measured build experiments. Report portability/build-time tradeoffs. They
should follow structural work, not substitute for it.

## What to defer

- **Parallelism:** no worker pool, parallel AI search, async logger or concurrent
  effect resolution is needed for this plan. Run benchmark games sequentially too.
- **Full make/unmake journals:** potentially useful if live-state copying remains
  costly after history separation, but every status, card, clock, revival and RNG
  mutation would need correct reversal. Retain clone-and-commit until measurements
  justify that complexity.
- **Broad persistent caches:** invalidate on all relevant state changes, including
  mid-action clock effects. Prefer per-query or board-revision-scoped reuse first.
- **Rule changes:** do not cap mana, remove extra movements, cut candidate actions,
  change game-ending conditions or reduce event detail to improve a timing result.
- **UI rendering work:** the automated mode already bypasses the TUI and prints only
  its last state. Rendering is not the recurring cost in this benchmark.

## Repeatable measurement procedure

Build once, then run from the repository root. This small driver creates fresh logs
on the workspace filesystem and reports wall time, action count, size and hash:

```sh
cargo build --release --manifest-path engine/Cargo.toml --offline
python3 - <<'PY'
import hashlib, pathlib, statistics, subprocess, tempfile, time
root = pathlib.Path(tempfile.mkdtemp(prefix="cchess-bench-", dir="engine/target"))
times, hashes = [], []
for run in range(6):
    path = root / f"game-{run}.txt"
    start = time.perf_counter()
    result = subprocess.run([
        "engine/target/release/cchess-engine", "--full-board",
        "--auto-play", str(path), "--ai-seed", "42",
    ], capture_output=True, check=True, timeout=120)
    elapsed = time.perf_counter() - start
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    actions = data.count(b"\nACTION ")
    assert b"\nCOMPLETED" in data
    print(run, round(elapsed, 4), actions, len(data), digest)
    if run:  # Exclude a warm-up from the summary.
        times.append(elapsed)
        hashes.append(digest)
assert len(set(hashes)) == 1
print("median seconds:", statistics.median(times), "logs:", root)
PY
```

A timeout is a failed measurement, not a completed sample. This driver leaves its
logs for inspection. Do not compare `/dev/null` with the normal workload: it omits
the requested artifact and is incompatible with exclusive file creation.

For each optimization:

1. Run the same seed and repeat enough times that the improvement exceeds normal
   run-to-run variation. Record wall time, CPU time, actions/s, memory and output
   bytes; report which measurements were actually collected.
2. Compare ordered intents, complete events, final state and RNG/clock counters.
   For changes that preserve formatting, the full log hash should also match.
3. Replay the trace through both reference and optimized builds. Test rejected
   actions and resolver-limit rollback, not only successful games.
4. Add fixed seeds such as 7 and 123 as separate workloads. Different seeds may
   produce different game lengths; compare the same seed across builds, never
   treat a shorter random game as proof of a faster engine.
5. Run `--full-board --four-player --auto-play ...` as the secondary scaling case.
   Use separate reference traces. Do not mix its timing into the two-player median.
6. Use long fixed-action prefixes to study history growth, but label unfinished
   prefixes separately from full-game completion results.
7. Run `cargo test --manifest-path engine/Cargo.toml --offline` and
   `cargo clippy --manifest-path engine/Cargo.toml --offline --all-targets -- -D warnings`.

For changed indexes and caches, explicitly cover immunity to allied/global/card
effects, ice versus deactivation, status stacking, promotion, transport, spawning,
revival, scheduled work, owner-turn timing, permanent extra movements and four-way
elimination. Optimization must preserve those rules and event order.

## Expected outcome and decision points

The immediate practical improvement is using the existing release build. Those implementation milestones now preserve the same 833-action reference game.
The next changes should follow fresh profiles rather than assume the original
phase proportions still apply.

The pre-optimization measured split put an approximate ceiling of only 1.2× on improving the whole
run by eliminating application time alone. Halving the roughly 81% selection share
would, under the same workload and other costs, suggest about 1.7× overall speedup.
These are budgeting estimates, not promised optimization results.

Prioritize selection work for this benchmark's current runtime, and history
separation for long-game scaling. Re-profile after those changes: the dominant
cost will move. Keep the engine on one thread throughout.
