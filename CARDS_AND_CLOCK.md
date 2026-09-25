# Cards, decks and logical clock events

**Current rules:** [settled gameplay decisions](SETTLED_RULES.md) supersede earlier provisional timing, immunity, economy and board-shape notes below.

The replacement engine implements the design's minimum card milestone. It has
20 versioned card definitions, a starter deck, unique card instances, shared
piece/card effects, a trigger card and a controlled ability-cost modifier.
The legacy card and Time implementations remain untouched.

## Playing

The default game deals three cards from each player's shuffled 21-card starter
deck. Its fixed seed is 42. Each subsequent turn start draws one card, including
Black's first turn; White begins with the opening hand. Deck rules and seed are
configurable through `Game::with_decks` before play.

`--cards-demo` uses ordered 18-card decks, eight opening cards and one starting
mana. Before the sequence below, pass both sides until player 0 has at least
7 mana (24 passes from setup). This also advances draws and egg jobs.

```text
hand
play forced_march
card summon_knight
play summon_knight c2
play fire
play supply_cache
scheduled
play event_haste event:3
hand
clock
undo
scheduled
```

The demo begins with two egg-hatching jobs, so Supply Cache creates event 3.
For other positions, use the IDs printed by `scheduled`.

| Command | Purpose |
| --- | --- |
| `hand` | Current player's card IDs, names and costs |
| `card <name-or-ID>` | Enumerate legal plays and targets |
| `play <name-or-ID> [square]` | Play an untargeted card or select a board square |
| `play event_haste event:<ID>` | Select friendly scheduled work |
| `play dispel card:<ID>` | Remove an opposing on-board card |
| `cards` | Inspect both players' active cards |
| `deck` / `discard` | Zone counts / discarded cards |
| `scheduled` | Pending jobs, IDs, source, owner and remaining clock ticks |
| `clock` | Movement, turn, round and owner-turn start counters |
| `undo` | Restore cards, RNG, costs, clocks, jobs, triggers and board state |

A name selects the first matching instance in hand; use its ID to distinguish
duplicates. The terminal is a local two-player interface. Debug traces and
`Game::preview` expose complete deterministic results, including draws; they are
not a hidden-information network protocol.

## Definitions and card lifetimes

Card assets live in `assets/cards/`; deck lists live in `assets/decks/`.
Definitions remain immutable. Each instance has a separate card ID, definition
ID, owner, zone and trigger-use records. Piece and card ID spaces are distinct;
`ActionSource::Piece`, `Card` and `System` retain unambiguous attribution.

| Kind | Resolution and destination |
| --- | --- |
| Summon | Pay costs, enter Discard, spawn at the selected empty square |
| Utility | Pay costs, enter Discard, execute shared effects |
| OnBoard | Pay costs and enter the persistent OnBoard zone |

Every card occupies exactly one ordered zone: Deck, Hand, Discard, OnBoard or
Consumed. Dispel moves an opposing active card to Discard. Paladin consumes the
matching Attack Demonic, Invulnerability or Revive instance into Consumed. Runtime unlock queries use active
instances; `Player.board_cards` remains a derived compatibility view and setup
seed list for the older demos.

Only the current player may play cards from their hand. Promotion pauses card
plays. Card costs are data-defined; supplied Summons cost one action, Utilities
and OnBoard cards cost zero actions. All pay their stated mana. Extra actions
affect the current turn only, and do not clear piece exhaustion. Second Wind
clears only the selected piece's acted flag, not its cooldowns, once-use records
or statuses. Summoned pieces start acted; arriving pawns still require promotion.

An owner may have at most one active copy of each OnBoard definition. Duplicate
copies may exist in a deck/hand, but playing another while one is active fails
without costs. Different owners may each activate a copy. Draws stop when the
deck is empty and log the shortfall; there is no fatigue loss, automatic discard
recycling, hand limit or voluntary discard action in this version.

## Supplied cards

| Card | Kind | Actions / mana | Effect |
| --- | --- | --- | --- |
| Summon Knight | Summon | 1 / 2 | Spawn Knight in any unique empty FullStep square adjacent to any friendly piece |
| Summon Dragon | Summon | 1 / 6 | Spawn a full-health, acted Dragon adjacent to any friendly piece; one copy in the starter deck |
| Summon Wall | Summon | 1 / 1 | Same placement rule, spawning Wall |
| Forced March | Utility | 0 / 1 | Gain two actions this turn |
| Second Wind | Utility | 0 / 1 | Reset one acted friendly piece anywhere on the board |
| Event Haste | Utility | 0 / 1 | Reduce one friendly job's remaining delay by two units of its own clock |
| Cancel Event | Utility | 0 / 0 | Cancel one friendly pending job |
| Fire / Ice | OnBoard | 0 / 2 each | Unlock the matching Magician ability |
| Invulnerability | OnBoard | 0 / 3 | Unlock Paladin's consumable ward |
| Attack Demonic / Revive | OnBoard | 0 / 3 and 0 / 4 | Unlock the matching consumable Paladin branch |
| Blood Pact | OnBoard | 0 / 2 | Gain one mana whenever any Demonic piece is removed, in addition to its owner's intrinsic reward |
| Royal Decree | OnBoard | 0 / 2 | Friendly Heroic abilities cost one less mana, clamped at zero |
| More Mana | OnBoard | 0 / 2 | Gain one mana at each subsequent owner turn start |
| Supply Cache | Utility | 0 / 1 | Draw two cards after two owner turn starts |
| Insight | Utility | 0 / 1 | Draw two cards immediately |
| Mana | Utility | 0 / 0 | Gain two mana |
| Dispel | Utility | 0 / 1 | Move one opposing on-board card to Discard |

Mana starts at 1 with +1 every two personal turns in normal matches. Card costs
and deck composition remain tunable balance values.
Incoming card effects follow the same protection checks as abilities: Immune and
Invulnerable prevent harmful and helpful effects on the protected piece; Heroic
alone does not. Summoning adjacent to a protected friendly piece remains allowed,
because the selected target is an empty square. Acquiring Immune cleanses existing Fire; acquiring Invulnerable alone does not.

Royal Decree is a deliberately constrained modifier: the registry stores a
source-tag selector and mana discount, collected from current friendly OnBoard
instances in card-ID/definition order. Legal previews and execution compute the
same effective cost and record applied discounts. Immune casters receive no
card-supplied discounts. Removing the card immediately
removes its discount without altering the ability definition. This does not yet
implement arbitrary augments or temporary modifier lifetimes.

## Clock contract

All clocks are logical and serialized; wall-clock time is irrelevant.

| Clock | Tick boundary |
| --- | --- |
| `Movements` | Start of an accepted primary piece action, ability or card play; passes and promotion choices do not tick it |
| `Turns` | Start of each incoming global turn |
| `Rounds` | Start of the incoming turn when the active roster wraps |
| `OwnerTurns` | A particular player's turn starts; the initial active player begins at ordinal 1, all other players at 0 |

`Schedule` retains its existing owner-turn meaning. `ScheduleOn` selects any
clock explicitly, with a positive delay. A deadline is the current value plus
the delay. A one-movement delay waits for the next movement start; a
one-global-turn delay waits for the next incoming player. Ordinary owner-turn
delay 1 means the caster owner's next turn start.

Clock events and due work resolve before the next input is accepted. Due jobs
are selected as a batch for that boundary and sorted by deadline then job ID.
Each job's direct effects finish before its completion marker; resulting
lifecycle reactions then drain before the next job starts. Jobs already in the
due batch cannot be retargeted by haste/cancellation. Newly scheduled positive
delays wait for a later tick.

Event Haste selects only pending friendly work and clamps its deadline to the
current value of that job's clock. If that makes it ready, its effects and
completion marker enter the front of the current queue and resolve during the
card play. Cancellation removes pending work without running completion effects.
Neither operation rewrites the originating source, owner or captured targets.

At owner turn start the incoming player and acted state are established,
`TurnStarted` is emitted and turn/round counters advance before status/cooldown
expiration or scheduled work can eliminate a player. Clock effects then resolve,
cards are drawn, due owner work resolves, and owner card triggers run.
Mana income follows the two-personal-turn cadence. Promotions pause input, and
resuming them does not repeat movement or start-of-turn processing. A valid
selected action invalidated by movement-start work is interrupted without costs;
the clock and due effects still commit. See [settled rules](SETTLED_RULES.md).

## Triggers, determinism and limits

Card triggers match logical clock ticks or piece removal with a tag, optionally
restricted to friendly pieces. Candidates sort by priority, card instance ID,
then definition order. Limits are once per event, action or global turn. Piece
Spawned/Removed hooks and card reactions enqueue through the same bounded effect
resolver, though their matcher definitions remain separate. A full arbitrary
event/condition matcher and parent-event graph are still future work.

Jobs preserve their source action/card/piece, owner, original action kind,
selected entity identity, selected square and optional scheduled/card target.
Player effects survive source death or card discard/removal. Missing entity
targets are skipped instead of retargeting replacement occupants; incoming
protection is checked when the effect actually runs. A scheduled action grant
outside its owner's active turn is skipped rather than stored for a future turn.

Deck shuffling uses a serialized SplitMix64 generator and Fisher–Yates with
rejection sampling. Seeds, RNG state and resulting deck-order events support
identical replay and undo. Draws and shuffle swaps consume resolution budget.
The 1,024-work-item and 1,024-pending-job limits cover cards, clocks, triggers and
piece effects. Any failure rolls back the entire action, including partially
drawn cards, RNG changes, paid costs, trigger limits and advanced clocks.

Validation checks card/deck versions, IDs, references, compatible targets,
effect nesting, clocks/delays and supported modifiers. Decks allow 1–256 cards;
opening hands and per-turn draws allow 0–256. Validate embedded content with
`--validate-assets`, or a deck file with `--validate-deck FILE`. `with_cards` and
`with_decks` also support programmatic custom setups. No snapshot importer or
schema migration API is provided yet; snapshot serialization and replay from
the same initial setup are tested.

In four-player mode each player has independent card zones and owner-turn counters.
Elimination discards that player's remaining hand/deck/on-board cards, cancels their
pending work and suppresses their queued effects. Owner-start elimination skips
that player's action. Round boundaries follow wraparound of the original roster,
skipping eliminated seats. See the engine README for the complete mode rules.

## Lasting Momentum

The default starter deck includes one `lasting_momentum` card per player. It costs
2 mana and zero actions. Playing it permanently adds 1 to that player’s movements
per turn, including 1 additional movement in the current turn. Copies stack; the
card goes to the discard pile, and its bonus persists independently of the card.
It does not reset a piece’s acted state. The allowance is serialized with the
player and reproduced by action replay.
