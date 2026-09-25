# Current implementation versus the design

**Current rules:** [settled gameplay decisions](SETTLED_RULES.md) supersede earlier provisional timing, immunity, economy and board-shape notes below.

Assessment: 2026-09-22, after the shared-rule decisions and cross-board implementation.
Reference: [`cchess_design_and_implementation.md`](../cchess_design_and_implementation.md).

The runnable replacement is `engine/`. The root `src/` and `assets/pieces/`
remain a separate legacy implementation with known compilation issues. Its 22
piece assets and older handlers are not automatically available in the new
engine. This report does not count an enum, asset or old handler as a complete
mechanic merely because it exists. Legacy compilation repair is outside scope.

## Main differences by design section

| Design section | Current implementation | Remaining difference |
| --- | --- | --- |
| 1–3: geometry/orientation/patterns | Signed positions, rectangular bounds, all four orientation rotations, all 14 deterministic patterns, unions and normalization | Two/four-player setup, inward-facing armies, elimination and live-seat turn order implemented; the 14×14 cross is implemented; arbitrary board masks and teams remain absent (seeded ray selection and deck shuffling also exist) |
| 4: targeting/conditions | Source/fixed/board/friendly/enemy origins; occupancy, ownership, tag, acted and archetype filters; boolean conditions, mana/card checks, direction/transport/scheduled-summon selection and traversal stat comparisons | Magic/buildable terrain filters implemented; no Player origin, neutral ownership, other terrain/status/stat filters, arbitrary target counts or multi-target selection. Cooldown/one-use checks are ability metadata, not general reusable condition nodes |
| 5: actions | Move, Take, Attack, Ability, PlayCard and promotion choice; shared validation, atomic commits, effective costs and structured failures | Tagged Piece/Card/System sources with status/trigger/job attribution; no full generic source/definition modifier framework |
| 6: effects/control flow | Movement, damage/removal, relative spawning, statuses, Sequence/ForEach, Schedule, AddMana, source transformation, ModifyStat, oriented spawning, directional Traverse, forced transport/push/collisions, tag-seeking traversal, revival and explicit tag suppression, drawing/shuffling, action grants/reset and scheduled-event changes | No generic Choice/Conditional, arbitrary traversal callback effects, effect references, arbitrary-target transform, arbitrary tag changes, health/Transportable modification or arbitrary zone transfers |
| 7: archetypes/instances | Immutable piece/card RON registries; unique piece/card instance IDs; owners, zones, positions, mutable state and death records | Revival links and corpse claims are serialized; no status/modifier definition registries, runtime content reload or migration framework |
| 8: tags/stats/statuses | Separate tags, numeric stats, damage/acted/cooldown/use state and four status kinds; tested timing, refresh, cleanse and expiry; Strength/Impenetrable comparisons, Transportable/Strength checks and persistent numeric buffs | Demonic death mana uses lifecycle hooks; Undead explicitly suppresses Biological/Heroic/Immune via effective tags; status definitions are fixed Rust variants |
| 9–10: pieces/abilities | All 36 catalogue archetypes in the new engine; promotion and catalogue ability branches | Rook throw and all three Paladin choices implemented; see catalogue below |
| 11: cards | 20 card definitions; Deck/Hand/Discard/OnBoard/Consumed zones; seeded shuffle/draw/play; summon and utility cards, persistent unlocks, consumption/removal | No arbitrary discard/recycle/exile effects, card augments or full planned card catalogue |
| 12: modifiers/augments | Royal Decree: live friendly Heroic ability mana discount, deterministic collection, zero floor and unchanged base definitions | General selectors/operations, modifier priorities/lifetimes and card augments remain |
| 13: events/triggers/scheduling | Bounded queue; movement/turn/round/owner clocks; scheduled jobs, haste/cancellation; piece lifecycle and card clock/removal reactions; priority/limits and rollback | No arbitrary event matchers or complete parent-event graph; piece/card trigger schemas are not fully unified |
| 14: turn loop/continuations | Two/four-player ownership, actions, exhaustion, income/draw, cooldown/status timing, elimination and draw outcomes; Action/Transfer state and serialized promotion continuations | Compound transport and scheduled-summon inputs are atomic; no general PendingChoice or multi-stage card/ability continuation; temporary modifier cleanup remains absent |
| 15: assets | Versioned piece/card/deck RON, semantic validation, cross-references, normalized patterns, standalone validators | Schema covers only the implemented subset; no content migrations or definitions/validation for the missing primitives and card/modifier systems |
| 16: architecture/determinism | Engine/client separation, ordered collections, shared effects, atomic snapshots, serialized seeded RNG and deterministic replay tests | Legacy UI remains separate. No general inverse-operation model, public save/load importer or snapshot migration/validation |
| 18: verification/reliability | Automated tests: geometry, core rules, assets, abilities/statuses, CLI, replay and snapshot round trips | Random/First AI replay and mixed-seat tests exist; no fuzz runner, broad performance benchmarks, persistent save/load validation or full-catalogue interaction coverage |

## Catalogue coverage

The new engine loads these thirty-six archetypes:

| Piece | Current coverage |
| --- | --- |
| Pawn | Movement/capture and mandatory promotion choice |
| Knight | Movement/capture and relative left/right pawn spawning |
| Bishop | Movement/capture and orthogonal sidestep |
| Rook | Movement/capture and fixed 5×5 capture-or-move throw |
| Queen | Movement/capture and knight-leap ability |
| King | Movement/capture and one-use teleport |
| Super Pawn | Movement/capture and friendly area-radius-2 cleanse |
| Magician | Orthogonal movement, separate card-gated Fire/Ice area casts |
| Paladin | Movement/capture and card-consuming Demonic smite, Invulnerable ward, normal revival |
| Succubus | Movement/capture, Biological-target Deactivate and Demonic death reward |
| Mandragora | Adjacent movement, two delayed mana payouts and Demonic death reward |
| Tesla Tower | Adjacent orthogonal movement/capture and delayed enemy-Structure Deactivate |
| Spider | Knight movement/capture, egg laying and Demonic death reward |
| Spider Egg | Immobile, scheduled same-entity hatch into Spider and Demonic death reward |
| Witch | Composite movement/capture, adjacent friendly/hostile damage and Demonic death reward |
| Archer | Taxicab movement and ranged Attack without movement |
| Shield Bearer | Forward movement/diagonal capture and adjacent ally Impenetrable buff |
| Builder | Diagonal leap/capture and directional three-wall placement |
| Wall | Immobile Structure with Impenetrable 3 |
| Ballista | Orthogonal movement and penetrating shot stopping before sufficient defense |
| Ram | Orthogonal movement, penetrating charge, last-empty relocation and collision damage |
| Crazy Pawn | Seeded two-square ray selection; sacrifice and delayed two-card draw |
| Cannon | Orthogonal Step movement; square-radius-five Attack |
| Ship | Three-square ray movement, adjacent capture and six-side broadside |
| Warlock | Diagonal ray movement; empty/buildable/magic portal spawning; immunity |
| Portal | Choice of eleven implemented Demonic summons, delayed placement and pending-work lock |
| Catapult | Orthogonal Step; adjacent transport with target/destination choice, strength limits and collision damage |
| Imp | Knight movement, immediate card draw, short cooldown and Demonic death reward |
| Basilisk | Infinite diagonal rays; orthogonal gaze at distances 3–10, first blocker only, Deactivate 3 |
| Gargoyle | Orthogonal Move/Take rays up to five squares |
| Mermaid | Knight/adjacent Move/Take; select and shorten an adjacent friendly Portal summon |
| Golem | Taxicab-2 movement/capture, Tough 3, Heroic and Demonic |
| Ogre | Taxicab-2 movement/capture, Tough 2 and three-square outward shove with shared collision handling |
| Oni | Orthogonal rays/knight leaps; tag-seeking ray traversal with Heroic far-side landing |
| Necromancer | Orthogonal movement; latest-friendly revival, new identities, corpse claims and explicit Undead suppression |
| Dragon | Knight/adjacent Move/Take, Tough 3, Immune/Heroic; dedicated summon card |

All catalogue archetypes are present. This does not mark the full design complete:
Broader systems remain; the catalogue ability branches are implemented.

## Concrete semantic differences and adopted choices

These should not be confused with systems that are simply missing:

- **Promotion:** the plan presents promotion as a pawn ability with a choice of
  transformation/spawn model. Current code makes it a mandatory, free continuation
  on arrival at the final rank, with four RON-defined classical choices. It
  preserves identity/statuses/acted state, resets health and old ability state,
  and delays turn transfer. The plan did not fully settle these semantics.
- **Magician:** Fire and Ice are confirmed separate casts with independent cooldowns.
- **Paladin:** all three branches exist as separate named abilities, each consuming
  its matching board card; no pending branch-selection continuation is required.
- **Statuses:** affected-owner turn-start timers; positive durations keep the
  higher value, negative durations the lower. Ice permits Attack only;
  Deactivate permits Move only.
- **Immunity:** cleanses statuses and blocks self/allied/global ability effects.
  This also blocks King's self-teleport. Take/Attack retain their tag rules.
- **Victory:** in two-player mode, last Royal removal wins, including status damage.
  Four-player free-for-all eliminates the owner and cleans up their army/cards/jobs;
  the last survivor wins, or the game draws when no players remain. The king carries
  an added Royal tag. Checkmate and alternative objectives are not implemented;
  the document lists victory as an open decision, not a checkmate requirement.
- **Economy:** normal matches start at 1 mana and gain 1 every two personal
  turns. Draw one per turn; cooldown 1 is ready next personal turn. All demos use the same starting mana. Individual costs remain explicit in the README.
- **Delayed work:** existing piece delays count owner turn starts; jobs run after
  income/reset/draw and before TurnStarted. Additional explicit clocks count
  completed primary actions, global turns and rounds. Selected targets retain identity; nested selectors query
  current state. Player effects survive source death. Egg hatching preserves
  statuses and identity but resets damage/abilities and starts acted. These timing
  and lifecycle choices are now explicit; arbitrary event reactions remain absent.
- **Siege:** Shield Bearer buffs all eligible adjacent allies, excluding self, with
  a permanent +1 Impenetrable. Builder requires all three placements and chooses a
  cardinal facing independent of owner orientation. Ballista/Ram use cardinal
  rays to the edge and can damage allies. Ram stops before a sufficient blocker,
  damages it and itself, and never lands on surviving occupants. Low-defense
  survivors do not stop the ray, so relocation may pass over them. These complete
  the specified core behaviors while making the previously unspecified details
  explicit. Costs and cooldowns are provisional; see the engine README.
- **Cards/decks:** supplied Utility and OnBoard cards cost zero actions; Summons
  cost one. One active copy of each definition per owner; duplicate hand/deck
  copies are allowed. Empty draws log a shortfall without recycling or fatigue.
  Defaults are a seeded 21-card deck, three opening cards and one draw at subsequent
  turn starts. Blood Pact rewards every Demonic removal, including enemies;
  its reward adds to the deceased piece owner's intrinsic reward.
- **Clocks:** movement, turn and round counters tick at their starts. Ordinary
  spell delays use owner turns; delay 1 resolves at that owner's next start.
  Promotion choices never double-tick, and passes are not movements.
- **Schema:** representative RON in the design cannot be loaded verbatim. The
  runtime uses a smaller vocabulary, scoped ability IDs, single-target/area/direction/transport/scheduled-summon
  selection and constrained effect sequences. Its version field does not yet
  provide schema migration.

## Roadmap position

Phases 1–3 have the minimum-playable foundation. Phase 4 is partial: abilities,
statuses and all catalogue branches work; shared immunity now blocks otherwise
available self-effects. Phase 5 now has bounded
queues, four logical clocks, scheduled jobs and constrained card trigger matchers
with priorities/limits. Full trigger unification and causal event graphs remain.
Phase 6 implements real card/deck zones and plays. Phase 7 has Royal Decree's
controlled cost modifier; the general modifier/augment framework is still absent.
Phase 8 has promotion continuations across action/end-turn boundaries, directional
traversal, oriented spawning, Ram relocation, Catapult transport and Ogre shove/collisions.
Oni traversal and Necromancer revival are also implemented. General choice
continuations and collision chains remain absent. Phase 9 has tests, snapshots and UI
undo, but not the full reliability requirements.

Both the minimum playable and minimum card milestones are satisfied. The full
mechanic definition of done is not. Next dependencies include card augments,
general trigger matching, persistence and UI integration.

See [Cards, decks and logical clock events](CARDS_AND_CLOCK.md) for the exact
current rules, supported API and CLI workflows, and deliberate limits.

The new expansion's adopted timing, random-direction selection, terrain defaults,
portal roster/delays and death behavior are recorded in the engine README.
Random selection samples only directions with eligible Move/Take targets;
players may choose the distance along that direction. Portal choices are separate
named abilities, without a generic pending-choice continuation.

Dragon follows its explicit catalogue tags (Immune/Heroic, without Demonic).
Summon Dragon has a provisional cost of one action/six mana and replaces one
Summon Knight in the 21-card starter deck; Portal remains restricted to its
explicit Demonic roster.
