# Playable replacement engine

This repository is a Cargo workspace:

- `core` (`cchess-core`): game state, rules, content, and serialized AI seat configuration.
- `AI` (`cchess-ai`): deterministic action selection and the `GameAi` extension trait.
- `tui` (`cchess-tui`): terminal interface, depending on core and AI.
- The root package (`cchess-engine`): CLI entry point and public facade.

Run `cargo run` from this directory to play, and `cargo test --workspace` to test
all members. Use `cargo check -p cchess-core`, `cargo check -p cchess-ai`, or
`cargo check -p cchess-tui` to check a member independently. Shared game assets
remain in `assets/`. Clients using AI methods such as `choose_ai_action` must
import `cchess_ai::GameAi` (or `cchess_engine::GameAi` through the facade).

[Basic AI opponents and mixed human/AI seats](AI.md) support Random and First policies.

[Single-threaded optimization plan and measured auto-play baseline](OPTIMIZATION.md).

**Current rules:** [settled gameplay decisions](SETTLED_RULES.md) supersede earlier provisional timing, immunity, economy and board-shape notes below.

This crate implements the minimum playable and minimum card milestones, plus
promotion, abilities, statuses and logical clock events from
[`cchess_design_and_implementation.md`](../cchess_design_and_implementation.md).
It builds independently of the legacy root crate; legacy source and compilation
issues have deliberately not been repaired. See [the gap report](DESIGN_GAP.md)
for what the full design still requires.

```sh
cargo run --manifest-path Cargo.toml
cargo run --manifest-path Cargo.toml -- --abilities-demo
cargo run --manifest-path Cargo.toml -- --events-demo
cargo run --manifest-path Cargo.toml -- --siege-demo
cargo run --manifest-path Cargo.toml -- --cards-demo
cargo run --manifest-path Cargo.toml -- --expansion-demo
cargo run --manifest-path Cargo.toml -- --four-player
cargo test --manifest-path Cargo.toml
cargo run --manifest-path Cargo.toml -- --validate-assets
cargo run --manifest-path Cargo.toml -- --print
```

## Playing

Interactive terminals now open the [fullscreen TUI](TUI.md): arrows to navigate,
Enter to select and move, A for abilities, C for cards, and ? for help. All board
and AI presets are supported. Use `--cli` for the text commands below; piped input
also selects text mode automatically.

Type `select e2` (or `e2`), then a highlighted destination such as `e4`.
Explicit commands include `move e2 e4`, `take e4 d5`, `pass`, `undo`, `log`, and
`quit`. Markers are `*` selected, `.` move, `x` take, and `!` attack. Uppercase
pieces belong to White (player 0), lowercase pieces to Black (player 1).

`abilities e1` lists that piece's abilities, action/mana costs, remaining
cooldowns, use state and legal targets; the board marks ability targets with `+`.
Use `ability e1 teleport e3` for a selected-target ability, or
`ability b1 muster` for a self-directed ability. An area ability such as
`ability c3 fire` takes no target and affects all eligible targets shown by the
preview. Illegal actions print their failure reason without changing the match.

On reaching its final rank a pawn pauses resolution for
`promote queen`, `promote rook`, `promote bishop`, or `promote knight`.
Other actions and passing are blocked until that choice is resolved.
Undo restores a complete pre-command snapshot, including mana, cooldowns,
statuses, card zones, RNG, clocks, scheduled jobs, pending promotion and logs,
up to 100 snapshots. Undoing a promotion
choice returns to the pending choice; another undo returns before the move.

The default classical setup starts each player with 1 mana and grants 1 every two
personal turns, at starts 3, 5, 7, and so on. Each has a shuffled 21-card starter deck (seed 42),
three opening cards and one draw at subsequent turn starts. `--abilities-demo` starts with 1 mana each and seeds
Fire, Ice and Invulnerability on-board cards for each player. The demo has four
additional archetypes and advanced pawns ready to promote. These are provisional
setup/balance choices. These seeded unlocks are real on-board card instances.

Example demo commands, starting a fresh match:

```text
abilities c3
ability c3 ice
pass
abilities c3
```

For promotion, start a fresh demo and run `move b7 b8`, then `promote queen`.

`--events-demo` adds Mandragora, Tesla Tower, Spider, Spider Egg, Witch and Archer
in a mirrored setup with 1 mana each. Try `ability b2 harvest`, then `scheduled`
and pass turns to watch payouts and eggs hatch. `scheduled` lists pending jobs
and their remaining ticks in the appropriate clock; `undo` restores them too.

`--siege-demo` contains Shield Bearer, Builder, Wall, Ballista and Ram. An
adjacent cardinal square chooses a build facing or ray direction, even when the
square contains a protected piece. `abilities <square>` lists available directions
and shows Strength and Impenetrable. Preview executes on a copy and spends nothing:

```text
preview b2 build b3
ability b2 build b3
undo
preview e2 pierce e3
preview g2 charge g3
ability c1 fortify
```

`--cards-demo` starts with an ordered deck, eight cards in hand and one mana.
Use `hand`, `card summon_knight`, `play forced_march`, `play summon_knight c2`,
`cards`, `scheduled`, and `clock`. Card names or IDs can be used; scheduled targets
use `event:ID` and opposing on-board card targets use `card:ID`.

See [Cards, decks and logical clock events](CARDS_AND_CLOCK.md) for all seventeen
cards, timing, seed/deck configuration, adopted rules and complete examples.

## Implemented abilities

All definitions are RON data; runtime resolution has no archetype-specific dispatch.
Durations and cooldowns below count the affected piece's or caster's owner turns.
Costs in the table are base costs; Royal Decree discounts Heroic abilities live.

| Piece | Ability | Actions / mana | Cooldown / restriction |
| --- | --- | --- | --- |
| Pawn | Mandatory promotion choice after reaching final rank | No extra cost | Queen, rook, bishop or knight |
| Knight | Spawn pawns in valid relative left/right squares | 1 / 1 | 10 turns; at least one empty square |
| Bishop | Move to one empty orthogonal adjacent square | 1 / 0 | 2 turns |
| Queen | Move by knight leap to an empty square | 1 / 0 | 5 turns |
| King | Teleport to an empty square within Chebyshev distance 5 | 1 / 2 | Once per instance |
| Super Pawn | Cleanse all statuses from one friendly piece within square radius 2 | 1 / 0 | 2 turns; target must have a status |
| Succubus | Deactivate one enemy Biological piece within taxicab distance 3 for 3 turns | 1 / 1 | 2 turns |
| Magician | Apply Fire 5 or Ice 3 to eligible enemies within square radius 4 | 1 / 2 per cast | 6 turns per ability; corresponding on-board card required |
| Paladin | Give a friendly piece Invulnerable 5 | 1 / 1 | Requires and consumes Invulnerability card |
| Mandragora | Gain 1 mana next owner turn and 2 the following owner turn | 1 / 0 | 3 turns |
| Tesla Tower | After 2 owner turns, Deactivate enemy Structures for 6 turns | 1 / 2 | 6 turns |
| Spider | Lay an egg one relative square forward | 1 / 1 | 3 turns; square must be empty |
| Spider Egg | Automatically become a Spider after 2 owner turn starts | Passive | Cannot move or capture |
| Witch | Damage every eligible adjacent piece, including allies | 1 / 1 | 2 turns |
| Archer | Ranged Attack within taxicab radius 6; move within taxicab radius 2 | 1 / 0 | Attack does not move; no Take |
| Shield Bearer | Give every eligible adjacent ally +1 Impenetrable | 1 / 0 | 3 turns; excludes self |
| Builder | Choose a cardinal facing and build Walls at F, FL and FR | 1 / 1 | 3 turns; all three squares must be empty and on-board |
| Wall | Immobile Structure, Impenetrable 3, Tough 1 | — | No actions or abilities |
| Ballista | Cardinal penetrating ray, Strength 2; damage pass targets only | 1 / 1 | 2 turns; stays in place |
| Ram | Cardinal penetrating ray, Strength 4; relocate and collide | 1 / 1 | 3 turns; blocker and self take 1 damage when stopped |

Witch movement combines orthogonal rays up to 5 with adjacent squares through
`Any` pattern composition. Every current Demonic archetype grants its owner 1
mana on removal via a RON lifecycle trigger. New costs, delays and cooldowns are
provisional balance choices.

All catalogue ability branches are implemented. Magician Fire and Ice are
confirmed separate casts with independent costs and cooldowns. Shared immunity
can block self-directed abilities, including King teleport.

All demos now start with 1 mana. Before trying expensive example sequences,
pass both players through enough turns to accumulate the required mana.
Four-player construction requires a cross: 14×14 for classical armies or 24×24
for full armies. See [the full-board guide](FULL_BOARD.md).

## Adopted core rules

- Square orthogonal lattice, signed coordinates, north increases y. Rectangular
  bounds are 1–64 cells per dimension. Normal setups are 8×8 for two players
  and a 14×14 cross with removed 3×3 corners for four-player FFA.
- Removing the opponent's last Royal piece wins, including Fire damage. The king
  is Royal. There is no check/checkmate, castling, en passant or automatic draw.
- One action per turn by default. Positive action costs and mana costs are data
  driven. Exhaustion transfers the turn, unless a promotion choice is pending.
  A player may pass. Acted state resets at the start of the owner's next turn.
- Tough defaults to 1. Damage starts at 0 and persists; Damage >= Tough removes
  the piece. Take damages first and moves only after removal. Attack does not
  move. A surviving defender still consumes the attacker's action and costs.
- Pawns may advance one or two squares on any turn with a clear path and capture
  only one square diagonally forward. Move/Take/Attack cannot target their source.
- Rays include the first occupied square, stop there, and never jump blockers
  before minimum range. Filters determine eligibility. EndOfBoard ignores blockers.
- Promotion is mandatory and free as a continuation of arrival. It preserves
  entity ID, owner, position, statuses and acted state; it adopts new base stats,
  resets damage to zero and clears old cooldown/use records. It emits no death.
  Multiple spawned pawns needing promotion are handled in entity-ID order.
  Winning king removal takes precedence over promotion.
- Spawned pieces start acted and cannot act until their owner's next turn.
- Cooldown N becomes ready at the Nth future owner turn start.
  The activation turn is not counted. One-use state and cooldowns follow entity
  identity through movement. Promotion initializes the new archetype's abilities.
- Validate everything before costs. Apply to a snapshot and commit once. Invalid
  target, cost, cooldown, card or promotion input leaves all state/logs unchanged.

## Status contract

| Status | Behavior |
| --- | --- |
| Deactivate | Allows Move only; blocks Take, Attack and abilities |
| Ice | Allows Attack only; blocks Move, Take and abilities |
| Fire | Does not prevent acting; deals one damage once on natural expiration |
| Invulnerable | Rejects incoming piece actions/abilities from other entities |

Statuses count down at **the affected owner's turn start**. Duration 1 expires
at the next such start. Timers advance even if a player passes. Ice and
Deactivate coexist; together they block all piece actions.

Each kind has at most one instance. Positive effects retain the higher remaining
duration; negative effects retain the lower. Equal values preserve attribution.
Cleansing removes statuses without expiration effects, cancelling pending Fire.

Heroic rejects Attack but permits Take and Ability. Immune cleanses statuses and
rejects spell/ability/status effects from any source, including self, allies and
global selectors. Ordinary Take and Attack retain their tag rules. Invulnerable
blocks incoming actions; self-directed actions and ordinary movement remain
allowed unless another protection/restriction blocks them. Gaining Immune through
promotion or transformation cancels pending Fire.

Status maintenance is not a new player action: it does not spend resources or
consult acted state. Expirations are removed in entity-ID/status-kind order, then
Fire damage goes through the shared damage/death layer. Source entity, player,
originating action and ability remain recorded even after the caster dies. Royal
removal during expiration settles victory before another action is accepted. Lifecycle
reactions use the shared effect queue.

## Siege mechanics and adopted choices

Shield Bearer moves one or two squares forward without jumping and captures one
square diagonally forward. Builder leaps exactly two diagonally for Move and
captures one diagonally. Ballista and Ram move one orthogonal square and have no
Take or ordinary Attack. Wall has no movement. These use the main design's stats,
including Wall Impenetrable 3 and Shield Bearer Transportable 1, rather than the
conflicting legacy asset values.

`ModifyStat` changes mutable Strength or Impenetrable by a signed amount, clamped
to 0–u32::MAX, and emits the before/after values with source attribution. Shield
Bearer affects adjacent friendly pieces excluding itself; Immune/Invulnerable
allies reject the buff. Zero becomes one, repeated buffs add, and no timer removes
the increase. Promotion/transformation resets stats to the new base definition.
An activation with no eligible ally fails without spending resources.

`SpawnOriented` uses a selected cardinal facing, independent of the owner's
forward direction. Placement is all-or-nothing, validated before costs and
rechecked during execution. New walls have distinct IDs, belong to the builder's
owner, start acted, and use the same spawn lifecycle hooks as other summons.

`Traverse` visits a chosen ray from nearest square to the board edge, excluding
the source. It compares each occupant's current Impenetrable against the caster's
Strength captured at activation, before applying damage. Defense >= Strength
stops the ray; lower defense does not. Callbacks run in authored order:

- `on_pass`: damage each encountered piece by 1, including allies. Empty squares
  have no damage target. Protected targets ignore damage but do not stop the ray
  unless their defense is sufficient. Heroic alone does not prevent Ability damage.
- `on_stop`: Ballista does nothing. Ram first relocates to the last empty square
  before the blocker, then damages the blocker and itself by 1. Destroying the
  blocker does not resume the ray or let Ram enter that square. An adjacent
  blocker leaves Ram at its original square. With default Tough 1, Ram dies on
  collision; its self-damage still occurs if the blocker rejects damage.
- `on_end`: runs after either a stop or the board edge. A surviving Ram relocates
  to the last empty square; Ballista stays put. Removed pass targets free landing
  squares. Surviving low-defense occupants never become landing squares, but the
  ray continues beyond them, so final relocation can pass over them.

Both abilities may target an empty ray and consume their normal costs. Traversal
steps and callbacks count against the shared resolution limit. Damage/removal
logs occur immediately, lifecycle reactions queue after the traversal, and source
removal ends further ray visits. Death rewards and victory use the shared rules.
The selected direction is submitted in one action; this does not introduce a
persistent multi-stage Choice system. Radius, friendly-fire, placement, timing
and balance choices above are explicit provisional decisions where the design
leaves details open.

Traversal callbacks currently support target/source damage and source relocation;
they are not arbitrary effect lists. General transport/pushing, collision chains,
revival, timed stat modifiers and health/Transportable modification remain future
work. The public `Game::preview` resolves the entire action on a copy (including
turn transfer), producing the same trace as committing it. The terminal preview
shows the direct ability effects up to `AbilityResolved`.

## Queued effects and delayed work

`Sequence`, `ForEach`, `Schedule`, `AddMana` and `TransformSource` compose the new
pieces without archetype-specific Rust handlers. Piece-local `Spawned` and
`Removed` triggers run once per lifecycle event; initial placements also receive
Spawned hooks, recorded in `setup_events`. Transformation emits neither hook.

Effects resolve in definition order. Sequence/ForEach children resolve before
remaining siblings; lifecycle reactions append to the queue. Existing `Schedule`
delays count owner turn starts. `ScheduleOn` additionally supports movement,
global turn and round clocks. After acted reset, mana income and the configured
card draw, due owner jobs run in (due tick, job ID) order, before `TurnStarted`
and promotion checks. Owner-start card triggers follow `TurnStarted`. The due batch
finishes before victory is checked. Status timers still use owner turn ends.

Jobs preserve owner, source attribution and selected entity identity. They never
retarget a replacement occupant. Nested `ForEach` resolves live targets when the
job runs and rechecks protection. Mandragora payouts and Tesla's board-wide query
survive caster removal; source-dependent effects skip missing sources. Egg
hatching also skips a source that has already changed archetype. Hatching keeps
identity, owner, position and statuses, resets damage/ability state, and marks
the Spider acted until its next owner turn. Ice/Deactivate do not pause jobs.

Resolution is capped at 1,024 work items per action and 1,024 pending jobs per
match. Exceeding either limit rolls back the entire action, including resources,
IDs, logs and queue changes. Effects carry action/ability/trigger/job provenance.
Card reactions to clocks and tagged piece removal have deterministic priorities
and per-event/action/turn limits. Arbitrary event matching and a complete causal
event graph remain future work. See the card/clock guide for clock boundaries,
haste/cancellation and promotion continuations during turn transfer.

## Architecture and verification

`geometry`, `content`, `state`, `targeting`, `rules`, `effects`, `abilities`,
`statuses`, `events`, `siege`, `cards`, `clock` and `commands` are independent of the terminal adapter. Immutable
archetypes are shared; instances contain IDs, mutable state and statuses.

The registry normalizes all 14 deterministic input patterns plus `Any` unions and validates
versioned RON, duplicate identifiers, cross-references, ranges, filters, target
counts, health/costs, positive status durations, promotion choices and compatible
effect targets. Diagnostics include file, definition and field path. Validate an
external directory with `--validate-assets PATH`. Gameplay embeds assets, so
rebuild after edits.

Abilities support selected or area targets, sequences, nested selectors and
delays, numeric stat changes, oriented spawning and immediate directional
traversal. Validation recursively checks effects and archetype references; ordinary movement
remains a single immediate effect and removed-piece hooks permit player mana,
draw and shuffle effects and their sequences/delays. General Choice, Conditional, arbitrary traversal callbacks,
revival and effect references remain absent.

Tests cover geometry, ownership, costs, turns, damage, status interactions,
cooldowns, immunity, promotion continuations, source attribution, arbitrary
archetypes, diagnostics, terminal gameplay and undo, and deterministic replay.
Siege tests cover blocker thresholds, immunity, stat stacking, placement rotations,
collisions, traversal limits, previews and shared death hooks.
Targets sort by coordinates; IDs/logs use deterministic order. Deck shuffling uses
a serialized seeded RNG and records the resulting order. Snapshots, including
card instances/zones, RNG, clocks, trigger limits, statuses, choices and scheduled work,
round-trip through RON. There is no public save/load importer or replay file
runner yet; replay tests resubmit intents from the same setup and compare the
entire trace and final state.

## Crazy Pawn, Cannon, Ship and Warlock

Run `--expansion-demo` for a seeded board with these pieces plus Catapult and Imp, starter decks,
and magic squares marked `~`. Try `select b3`, `ability b3 sacrifice`,
`ability d2 portals`, `abilities f3` and `scheduled`; `undo` restores the RNG too.
Once a portal can act, `abilities c3` lists its summon choices and valid squares.

- **Crazy Pawn:** Move/Take use a ray of up to two squares. `random_ray` selects
  uniformly among directions with a legal Move or Take, in canonical order,
  using the serialized match RNG. Both actions share that direction; the player
  chooses distance or capture along it. Queries and previews peek a copied RNG;
  successful actions consume the sample and emit `RandomDirectionChosen`.
  Other RNG-consuming actions can change the next direction. There is no
  reroll command. Sacrifice removes the pawn without damage and draws two cards
  at its next owner's turn start, even though the pawn is gone.
- **Cannon:** orthogonal one-square Move; Attack within square radius five,
  without line-of-sight blocking or movement. No Take. Attack protections apply.
- **Ship:** eight-direction ray Move up to three; adjacent Take. Broadside deals
  one damage to eligible adjacent pieces, including allies, except relative
  forward/back. Provisional cooldown: 12 owner turns; mana cost zero.
- **Warlock:** diagonal ray Move up to two; Demonic, Immune, Transportable 5.
  Portals cost three mana with a five-owner-turn cooldown, spawning on every
  empty, buildable, adjacent magic square. Each portal starts acted.
- **Portal:** immobile Demonic piece. Choose `summon_imp`, `summon_mandragora`,
  `summon_succubus`, `summon_witch`, `summon_basilisk`, `summon_gargoyle`,
  `summon_mermaid`, `summon_ogre`, `summon_golem`, `summon_oni` or
  `summon_necromancer` and an adjacent empty buildable square.
  Provisional delays are one (Imp/Mandragora), two (Succubus/Mermaid), three
  (Witch/Basilisk/Gargoyle/Ogre), and four (Golem/Oni/Necromancer) owner turn starts;
  each costs one action and zero mana. `NoPendingWork` locks all summon choices
  until completion/cancellation. Haste uses the existing event system. Occupied
  destinations at resolution skip spawning and release the lock; jobs survive
  source death and summons start acted. Warlock and Portal grant their owner
  one mana on removal, matching other implemented Demonic pieces.

`Game::with_terrain(magic, unbuildable)` configures serialized terrain before
play; ordinary squares are buildable but not magic. Terrain is currently static.
`Game::with_seed(seed)` seeds randomness before play; `with_decks` also sets that
same RNG, so use the final desired seed after deck configuration if overriding it.
These rules keep randomness outside the deterministic geometry evaluator.
Portal's choices cover implemented Demonic units; additional catalogue summons
and a general multi-stage choice system remain future work.

## Catapult and Imp

Both are available in `--expansion-demo`: White's Catapult starts at a1 and Imp
at b1. Try `abilities a1`, `preview a1 launch b1 d3`, then
`ability a1 launch b1 d3`. Use `undo` to restore the position, then
`ability b1 draw` to test Imp.

Catapult has orthogonal Step movement, no Take/Attack, and Strength 3. Its launch
selects one adjacent friendly or enemy piece with `Transportable <= Strength`,
then an in-bounds destination one to three squares from that piece along any
of the eight ray directions. It costs one action, zero mana and has a provisional
two-owner-turn cooldown. Direction is absolute and independent of owner facing.

Transport follows the path and stops before its first occupied square. A collision
deals one ability damage to both the carried piece and blocker, with normal
Immune/Invulnerable protection. Protected blockers still stop movement. The carried
piece reaches the last empty square before damage; destroying the blocker does not
extend the movement. It can collide with the launcher or allies. Off-board, zero,
non-ray and excessive-range destinations are rejected atomically. Terrain does not
block transport; buildability is a placement filter, not a movement restriction.

Forced movement preserves identity, ownership, statuses, cooldowns and acted state.
An inactive piece can be carried, but an inactive Catapult cannot launch. Friendly
pawn arrival on the final rank requests promotion through the existing continuation;
enemy promotion is checked when that owner next starts a turn. Collisions share
normal death hooks and victory. Both target and destination are serialized in
`ActionIntent::Transport`; previews, replay and undo use the same resolution.
The range of three, one collision damage and cooldown are provisional balance
choices because the design does not specify them.

Imp moves by knight leap and has no Take/Attack. `draw` costs one action and zero
mana, draws one card immediately, and has a provisional one-owner-turn cooldown.
An empty deck emits DrawSkipped without reshuffling. Imp is Demonic with
Transportable 1 and grants its owner one mana on removal. Portal's `summon_imp`
uses a one-owner-turn delay and the existing pending-summon lock.

## Four-player mode

Run `cargo run --manifest-path Cargo.toml -- --four-player` for local
four-player free-for-all. `--four-player --print` prints the initial setup.
The board is a 14×14 cross (a1–n14 envelope), with all four 3×3 corners removed. Each side
starts with the classical eight-piece back rank and eight pawns, centered along
its edge, plus an independent seeded starter deck and three opening cards.
Custom four-player positions use `Game::new` with four distinct players and
14×14 or 24×24 bounds; other four-player dimensions and placements in corners are rejected.
`Game::four_player` supplies the standard setup. The same piece/card registry and
all implemented abilities work with either player count.

| Player | Name / starting side | Forward | Example opening |
| --- | --- | --- | --- |
| 0 | White / bottom | North | `move d2 d4` |
| 1 | Blue / left | East | `move b11 d11` |
| 2 | Black / top | South | `move k13 k11` |
| 3 | Red / right | West | `move m4 k4` |

Turns follow that roster cyclically, skipping eliminated players. Cells show an
owner ID followed by a piece symbol (`2K` is player 2's king); target markers
remain unchanged. `select`, abilities, cards, preview, promotion, `log` and `undo`
use the same commands as two-player mode. Promotion occurs at the opposite board
edge in each player's forward direction.

Removing a player's last Royal eliminates that player. Their remaining pieces
leave the board without death triggers/rewards; their hand, deck and on-board
cards move to discard. Future jobs are cancelled and already queued effects
owned by that player are skipped. Existing statuses on surviving pieces retain
their duration and attribution. The last active player wins; if a single resolution
batch eliminates all players, the result is a draw. Elimination and draw state are
serialized and included in undo/replay. These are adopted free-for-all rules;
teams are not supported.

Owner timers advance only on that owner's turns, so three opponent turns do not
shorten a cooldown. Rounds advance when the next active seat wraps past the end
of the original roster, even if seat 0 was eliminated. A player eliminated during
owner-start jobs gets no action; the attempted turn is closed and the next live
seat starts. Turn/round effects run after establishing the incoming seat and before accepting input.
Two-player setup and king-removal victory retain their existing behavior.

## Basilisk, Gargoyle and Mermaid

All three are in `--expansion-demo` and the Portal summon roster. White's Basilisk
starts at c1, Gargoyle at g1 and Mermaid at c2. Each has the Demonic death reward
(one mana to its owner); Mermaid is also Biological and Transportable 3.

- **Basilisk:** unlimited diagonal Move/Take. `gaze` applies Deactivate 3 to the
  first eligible enemy on each orthogonal ray, at distances 3–10 inclusive. Any
  occupied square blocks the ray, including pieces inside the minimum range,
  allies and protected enemies. It cannot see through blockers to a later enemy.
  A cast with no eligible targets is rejected without cost. Provisional cost is
  one action, zero mana, with a three-owner-turn cooldown.
- **Gargoyle:** orthogonal ray Move/Take up to five squares, with normal blocking.
  It has no Attack or active ability in the design.
- **Mermaid:** Move/Take combine all adjacent squares with knight leaps. `hasten`
  selects a pending summon from an adjacent friendly Portal and reduces its delay
  by two ticks of that job's clock. Provisional cost is one action, zero mana, with
  a two-owner-turn cooldown. The Portal must still exist, be friendly, adjacent
  and able to receive the ability; its selected job must be a direct SpawnTarget
  summon. Unrelated jobs, hostile jobs and jobs from dead/distant portals are invalid.

Use `scheduled` to inspect job IDs and `abilities c2` for eligible choices. Example
from a fresh expansion demo:

```text
ability d2 portals
pass
ability c3 summon_witch c4
pass
preview c2 hasten event:1
ability c2 hasten event:1
```

A delay reaching the current tick resolves immediately through the shared queue.
If the destination is occupied, spawning is skipped and the Portal unlocks. Failed
resolution rolls back costs, cooldowns, rescheduling and spawning. The selection is
serialized as `ActionIntent::ScheduledAbility`, and previews use the same path.
The new `ScheduledSummon` selection is deliberately restricted to a single direct
summon job; general scheduled-effect choices remain future work.

## Golem and Ogre

Both are available in `--expansion-demo`: White's Ogre starts at e2 and Golem at
f2. Try `preview e2 shove f2`, `ability e2 shove f2`, then `undo`.

Golem has Tough 3, Demonic and Heroic. It moves and captures within Taxicab range
two (including diagonal one-step squares), without ray blocking. Heroic rejects
Attack actions; ordinary captures, statuses and incoming abilities still apply.
It has no active ability in the design.

Ogre has Tough 2, Demonic, and the same movement/capture pattern. Its `shove`
selects one adjacent eligible piece, friendly or hostile, and pushes it three
squares directly away from Ogre in any of eight directions. No Transportable or
Strength check applies. The shared collision resolver stops before the first
occupant, deals one ability damage to each collision participant, and preserves
protection, death rewards, status attribution, promotion and four-player elimination.
The Ogre stays in place. Destroying a blocker does not extend the push.

At the board edge, a shove ends at the last in-bounds square without extra damage.
If the target is already at the outward edge and cannot advance, validation rejects
the action without cost. Forced movement preserves owner, identity, statuses and
acted state. Provisional balance values are one action, zero mana, a two-owner-turn
cooldown and one collision damage. Both pieces grant their owner one mana on removal.
Portal offers `summon_ogre` after three owner turn starts and `summon_golem` after
four; these delays are provisional. Summons start acted and Mermaid can hasten them.

## Oni and Necromancer

Both are in `--expansion-demo` and the Portal summon roster (provisional four-owner-turn
summon delays). White's Oni starts at h1; Necromancer starts at d1. Both grant their
owner one mana on removal through the normal Demonic death hook.

Oni is Demonic and Biological. Move/Take combine orthogonal rays up to five squares
with knight leaps. `seek` chooses any adjacent square as a direction, then follows
that ray to the edge or first occupant. If that occupant has effective Heroic and
the square immediately beyond is empty and on-board, Oni lands there. Otherwise
it lands at the last empty square before the blocker. No damage is dealt, and the
Heroic piece may belong to any player or be Immune. An action with no relocation
is rejected without cost. Provisional cost: one action, zero mana, two-owner-turn
cooldown. First-occupant blocking and far-side landing are adopted interpretations
of the design's unspecified traversal details.

Necromancer is Demonic, moves by orthogonal rays up to two squares, and has no
Take/Attack. `revive` selects an empty square within Taxicab range three and raises
the latest unclaimed friendly death as Undead. Provisional cost: one action, three
mana, three-owner-turn cooldown. No friendly corpse means no legal revival.

Revival creates a **new entity ID**, restores the archetype's base stats/full health,
clears statuses, cooldowns and once-per-life ability flags, and starts acted.
Spawn lifecycle hooks run again, so revived eggs receive a fresh hatch schedule.
The old death record remains in history with `revived_as`; the new piece stores a
`RevivalLink` to the corpse, caster and action. Each death record may be used once;
a subsequent death of the revived piece creates a new eligible record. Old entity-bound
jobs never attach to the new identity. Revival never undoes elimination or game over.

Undead adds its own tag and explicitly suppresses Biological, Heroic and Immune.
Demonic, Structure and Royal remain effective. Suppression and the revival link
survive promotion/transformation. `Game::effective_tags` is shared by targeting,
protection, Oni's search, modifiers and death-card matchers, including queries on
dead records. `abilities <square>` shows the effective tags.

Try this from a fresh expansion demo:

```text
preview h1 seek h2
ability b3 sacrifice
pass
preview d1 revive c3
ability d1 revive c3
abilities c3
```

Revival choices, claims, links and tag changes are serialized, previewable and
restored by undo. The reusable `ReviveLatest` effect also supports non-Undead
revival used by Paladin.

## Dragon — catalogue complete

All 36 catalogue archetypes are now registered; some older pieces still have
unfinished abilities, listed in the gap report.

Dragon has Tough 3, Immune and Heroic, with Move/Take combining knight leaps and
all adjacent squares. It has no Attack or active ability. Heroic blocks incoming
Attack; Immune blocks incoming abilities/cards, including beneficial effects.
Ordinary Take damage still works and must accumulate three damage before removal.
The design explicitly omits Demonic, so Dragon grants no Demonic death mana and
is not in the Portal's Demonic summon roster. Necromancer can revive it as Undead,
suppressing Heroic/Immune and restoring its full Tough 3.

`--expansion-demo` places White's Dragon at f1. Try `abilities f1`, `move f1 h2`,
and `undo`. The new `summon_dragon` card costs one action and a provisional six
mana, using the existing empty-square adjacency rules. The 21-card starter deck
now includes one copy, replacing its second Summon Knight. Summoned Dragons start
acted, with full health and their normal tags. Use `card summon_dragon` to inspect
it and `play summon_dragon <square>` when it is in hand.


### Rook throw and completed Paladin choices

Rook `throw` selects **all rooks of either owner in the initial 5×5 square
centered on the caster (halfside 2), including the caster**. It does not select
other pieces or recursively recruit rooks outside that square. Choose an adjacent
orthogonal square for direction. Resolve front to back, breaking ties by entity
ID. Each rook tries to capture the first enemy on its ray with ordinary Take
protection and one damage. A removed target permits landing on its square;
otherwise the rook moves to the last empty square before the blocker, or the
board edge when unobstructed. Friendly pieces and Invulnerable enemies block;
no empty space means no movement. A surviving damaged enemy also stops the rook.
Friend/enemy is relative to each moving rook. Selected rooks are forcibly moved
even if previously acted or disabled and are marked acted; only the caster pays
one action, zero mana. The caster must satisfy normal ability eligibility.
Selection, ordering, damage, movement and serialization are deterministic.

Paladin now offers `smite`, `ward`, and `revive`, each costing one action and one
mana and consuming its matching on-board card. Smite deals one damage to an
enemy Demonic piece anywhere, respecting ability protection. Ward retains its
friendly Invulnerable status for five affected-owner turns. Revival places the
latest unclaimed friendly death on any empty square as a fresh, full-health,
acted instance with normal archetype tags (not Undead), and records its revival
link. Failed validation consumes neither cards nor resources. Attack Demonic
and Revive are OnBoard cards costing three and four mana respectively; both are
in the starter deck, card demo, and preloaded ability demo.
