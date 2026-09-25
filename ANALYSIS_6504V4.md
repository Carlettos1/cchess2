# Four-player full-board analysis: seed 650

The supplied [6504v4 log](6504v4) contains **3,099 committed actions**, ending in
**Player 0's victory**. Replaying the exact scenario takes approximately **0.79
seconds in release mode** and **7.92 seconds in debug mode** on this machine.
The reported seven seconds is therefore consistent with a debug build, though
the log itself does not record the build profile or runtime.

No gameplay code or original log was changed during this analysis.

## Verified input and reproduction

Original command:

```sh
--four-player --full-board --auto-play 6504v4 --ai-seed 650
```

The original file is `engine/6504v4`, not in the repository root. It records a
24×24 cross board with four Random AI seats and 184 starting pieces.

| Property | Value |
| --- | --- |
| Actions | 3,099 |
| Final turn counter | 1,605 |
| Winner | Player 0 |
| Remaining pieces | 27, all owned by Player 0 |
| Recorded action events | 28,042 |
| File size | 11,629,153 bytes, approximately 11.09 MiB |
| Final serialized snapshot | 6,780,066 bytes, approximately 58.3% of the file |

Original file SHA-256:

```text
c3b1fabb02f4eaaf5c2448650be64a21a909f71e71617b46470c6c46713c9544
```

Three release runs and one debug run reproduced the **entire file byte-for-byte**.
An instrumented replay additionally checked every AI-selected intent and every
resolved action/event against the original records.

Use a fresh filename because auto-play deliberately refuses to overwrite logs:

```sh
cargo build --release --manifest-path engine/Cargo.toml --offline
engine/target/release/cchess-engine \
  --four-player --full-board --auto-play 6504v4-release.txt --ai-seed 650
```

Run from the repository root. Compilation was excluded from the timings below.

## Runtime measurements

Same environment as the [optimization report](OPTIMIZATION.md): AMD Ryzen 7 5700G,
Linux x86-64, Rust `1.98.0-nightly (4c9d2bfe4 2026-07-01)`. Runs were sequential,
with logs written under `engine/target` on the workspace ext4 filesystem. CPU
frequency and affinity were not fixed. Writes used the OS page cache, not durable
per-action disk sync.

| Build | End-to-end time |
| --- | ---: |
| Release, run 1 | 0.804 s |
| Release, run 2 | 0.788 s |
| Release, run 3 | 0.793 s |
| **Release median** | **0.793 s** |
| Debug, single run | 7.920 s |

That is approximately 3,910 actions/second in release versus 391 in debug: a
roughly **10× build-mode difference**, not a new optimization applied to this run.
The seven-second observation does not by itself show that the earlier optimizations
failed. Both tested builds already contain those optimizations.

Compared with the earlier optimized two-player seed-42 benchmark:

- This game has 3.72× as many actions: 3,099 versus 833.
- It takes about 5.7× as long in release: 0.793 versus 0.139 seconds.
- Average cost is about 0.256 ms/action versus 0.167 ms/action, roughly 1.5× higher.

The larger board, different pieces and ability targets, four players, and more
live/dead state make this a different workload. These ratios do not isolate the
cost of adding players; they are comparisons between these two particular games.

## What happens during the game

### Action mix

| Selected action | Count | Approximate share |
| --- | ---: | ---: |
| Ordinary movement | 2,493 | 80.4% |
| Ability | 331 | 10.7% |
| Take attempt | 138 | 4.5% |
| Card play | 74 | 2.4% |
| Attack | 35 | 1.1% |
| Transport ability | 27 | 0.9% |
| Promotion choice | 1 | <0.1% |
| Explicit end turn | 0 | 0% |

These are selected intents, not necessarily successful captures or unique pieces
moved. Turns also end automatically when the action budget is exhausted.

Royal eliminations occurred at:

| Action | Turn before resolution | Eliminated player |
| --- | ---: | ---: |
| 904 | 514 | P2 |
| 2,379 | 1,245 | P1 |
| 3,099 | 1,605 | P3; P0 wins |

There were 122 spawned pieces and 15 revivals, so the game did not simply become
steadily smaller. Live pieces fell to 39 by action 1,500, then rose to 60 by action
2,000. The full accounting is `184 + 122 + 15 - 294 = 27` remaining pieces.
The 294 removal events include administrative elimination cleanup, not just
combat kills.

Other relevant observations:

- All four players played Lasting Momentum and finished with an allowance of two
  movements per turn, including the eliminated players' retained player records.
- The maximum observed scheduled queue was only **three jobs**.
- The maximum combined hand size after sampled action resolutions was 70 cards.
- The longest consecutive stretch without a piece-removal event was 137 actions.
  This does not mean that those actions had no damage or other effects.
- Dragons made 433 ordinary moves, kings 338 and ships 202. Together that is about
  39% of ordinary movement selections.
- Portal abilities were selected 103 times. Spawning and revival help explain why
  the middle of the game becomes more expensive again.

The long game is consistent with uniform Random AI, which does not prioritize
winning captures. Changing the policy would alter the benchmark and seeded trace;
it would not be a semantics-preserving engine optimization.

## Where time is spent

A temporary optimized harness replayed the original log and separately timed
selection, application, serialization and writing. It retained the normal engine
rules and checked replay equality. Its output went to `/tmp` tmpfs. It also
collected analysis data outside the timed regions, so it is not identical to the
normal CLI benchmark and its phase totals should not be substituted for wall time.

| Phase | Instrumented time |
| --- | ---: |
| AI selection, including legal-action generation | 576.8 ms |
| Apply selected actions | 197.9 ms |
| Serialize action records | 27.9 ms |
| Write and flush action records | 10.4 ms |
| Clone final snapshot | 2.9 ms |
| Serialize final snapshot | 18.3 ms |

Selection accounts for approximately **69%** of this measured phase total, applying
for **24%**, and serialization/writing for most of the remainder. Selection is still
the main target for optimization. This is a phase profile, not a sampled instruction
or allocation profile; peak process memory was not measured.

The first 500 actions spent 146.1 ms selecting and 38.2 ms applying. Actions
1,001–1,500 spent 45.5 and 25.5 ms; actions 2,001–2,500 spent 112.0 and 35.7 ms.
Application cost is not increasing monotonically with action history. The middle
increase coincides with changing board population and gameplay, so there is no
evidence here to reverse the earlier decision to remove history copying.

### Which generators deserve attention

A separate instrumented replay timed calls to `legal_actions(piece)` and
`legal_card_plays(card)` before applying each recorded action. It excludes the
Random selector's deduplication and index draw. Extra timer calls and different
cache behavior mean its total must not be added to the phase table above.

| Generator | Calls | Candidate intents returned across all calls | Time |
| --- | ---: | ---: | ---: |
| Oni | 2,574 | 55,418 | 41.8 ms |
| Ogre | 1,673 | 17,300 | 34.8 ms |
| Super Pawn | 2,677 | 8,540 | 34.0 ms |
| Second Wind card | 822 | 169 | 31.9 ms |
| Paladin | 823 | 24,074 | 31.8 ms |
| Portal | 1,131 | 33,077 | 31.7 ms |
| Archer | 1,260 | 13,436 | 28.6 ms |
| Crazy Pawn | 1,197 | 3,182 | 26.9 ms |

Candidate totals include choices considered repeatedly in different states, not
executed actions. No single generator dominates: Oni was about 6.9% of this
separate generator timing total. Prefer improvements to shared targeting code
over an Oni-specific rewrite based solely on its first-place ranking.

## Recommended next single-threaded work

### 1. Stop scanning the whole board for small ranges

In [`RuntimePattern::targets_on_board`](src/geometry.rs), `Range` currently loops
over every board position, then tests its Manhattan or square distance from the
origin. A radius-2 effect on this board still starts with 576 coordinates, even
though its local bounding box contains at most 25 positions and its Manhattan
radius contains at most 13.

This path is used by patterns such as Ogre movement and Super Pawn's cleanse
area. The generator timings make this a strong next candidate, but they do not
isolate the exact percentage spent inside `Range`.

Restrict iteration to the clipped local bounding box, apply the same distance and
playability tests, and preserve x-then-y output order. Excluded corners and edge
clipping must retain their current behavior. Validate all range shapes against the
existing implementation before replacing it. This is a small, shared improvement
with a clearer scope than introducing broad caches.

### 2. Specialize board-origin, occupied-square targeting

[Second Wind](assets/cards/second_wind.ron) uses `Board + SelfOnly` with
`Friendly`, `Occupied` and `HasActed` filters. The generic resolver builds all
board origins and expands each even though only existing friendly pieces can
qualify. It was queried 822 times and returned only 169 candidate intents in total.

For this exact target shape, enumerate piece positions, apply the same filters,
and sort/deduplicate using the existing canonical order. This avoids many empty
squares. Do not generalize the shortcut to targets with `Empty` filters or other
patterns: summon and area semantics differ. Preserve immunity checks in the card
validator.

### 3. Avoid allocating a tag set for single-tag questions

[`Game::has_tag`](src/state.rs) constructs `effective_tags(piece)` and then asks
whether one tag is present. Under the current implementation in
[`revival.rs`](src/revival.rs), an equivalent single-tag test is:

```text
(base tags contain tag OR added tags contain tag)
AND suppressed tags do not contain tag
```

Use that direct membership check while retaining `effective_tags` for callers that
need the full set. This affects many target/protection checks, including Oni's
Heroic seeking. Add equivalence tests for revived pieces and suppressed Immune,
Heroic and Biological tags. Allocation savings need a fresh benchmark; the current
measurements do not provide a separate `has_tag` time.

### 4. Then profile allocations and repeated eligibility checks

Pattern expansion and target resolution still use temporary vectors and tree sets.
Consider reusable buffers or board-sized membership storage only after measuring
what remains following the targeted changes above. Preserve first-seen action order,
position order and the single random-index draw.

Shared eligibility is checked early now, but some checks are repeated for each
candidate. A carefully scoped per-generation context could reuse the already
validated source, ability and cost. Do not remove submitted-action validation or
revalidation after movement-start clock effects.

### Lower priorities for this scenario

- A scheduling heap is not compelling with a maximum queue of three jobs.
- Log writing is not the main cost. The final snapshot repeats the action history
  and accounts for 58.3% of file bytes, so a future versioned checkpoint format
  could reduce disk output. That is a format/API change, not a reason to omit the
  required complete action stream.
- A full make/unmake transaction system is still more invasive than the targeting
  changes above. Investigate remaining live-state copying only if it remains
  significant after those changes.
- Parallelism is unnecessary for these proposals. Keep the benchmark and game
  simulation on one thread.

## Acceptance criteria for the next patch

Use this completed game as an additional fixed benchmark alongside two-player seed
42. Run release builds with logging enabled and new filenames. Before and after
changes, compare the entire log to the supplied file, including its SHA-256 above.

Require identical 3,099 actions, 28,042 action events, P0 victory and final snapshot.
Measure repeated wall times rather than only the best run. Test target equivalence
at edges and excluded corners, empty and occupied boards, revival/tag suppression,
clock-start invalidation and rollback. A change that shortens the game by changing
Random AI's candidate order does not count as this optimization.
