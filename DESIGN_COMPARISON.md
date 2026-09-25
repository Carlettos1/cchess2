# Implementation, original design, and gameplay expectations

As of 2026-09-23. The [settled rules](SETTLED_RULES.md) supersede the original disputed choices.

Scope: the runnable replacement in `engine/`, not the legacy root crate.
“Original design” refers to [the implementation plan](../cchess_design_and_implementation.md)
and [game notes](../idea.md), before later explicit rule changes. The gameplay
column contains recommendations for a clear, coherent game—not additional agreed
requirements or claims that balance has been established.

| Area | Current implementation | Original design expectation | Gameplay expectation / decision needed |
| --- | --- | --- | --- |
| Overall coverage | All 36 catalogue pieces and their ability branches; 20 cards; playable two- and four-player CLI. | A reusable, data-driven engine covering pieces, cards, modifiers, triggers and continuations. | Catalogue completeness does not establish balance or a finished player experience. Playtest complete matches. |
| Rook | All rooks of either owner in the initial centered 5×5 square, including caster; front-to-back capture attempt, otherwise movement. No recursive recruitment. | Recursive throwing of adjacent towers. **Superseded by the latest requested 5×5 rule.** | Clearly preview selected rooks and landing squares. Either-owner selection and forcing already-acted/disabled rooks are implementation interpretations that deserve explicit rules visibility. |
| Paladin | Separate Smite, Ward and Revival commands consume matching board cards. Smite targets enemy Demonic pieces; Ward targets allies; Revival restores the latest unclaimed friendly death with normal tags. | Three card-dependent branches: damage any Demonic piece, grant Invulnerable 5, or revive the latest friendly death. | Branches are implemented, but enemy-only Smite narrows “any Demonic.” Show eligible targets, required card and the exact corpse to revive. Separate commands satisfy play without a generic choice system. |
| Promotion and revival | Mandatory free promotion to four classical choices; revival creates a fresh identity with reset health/ability state and a claimed death record. | Promotion by transformation/spawn; friendly revival and Undead suppression, with lifecycle details incomplete. | State identity, status retention and once-use resets explicitly; test revival/reset loops for excessive value. |
| Statuses and protection | Owner-start timers; positive max/negative min stacking; Immune cleanses and blocks self/allied/global effects and card modifiers; Ice permits Attack only, Deactivate Move only. | Four statuses and tag-based protection; several timing and self/global interactions unresolved. | These rules are now settled. Display expiration and blocked effects, including self-teleport on an Immune King. |
| Magician and siege | Separate Fire/Ice casts are confirmed; collision, penetration and last-empty landing rules are explicit. | Magician applies unlocked effects; siege descriptions leave some range/order/landing details open. | Previews should reveal friendly damage, surviving blockers and forced movement. |
| Cards and economy | All presets start with 1 mana; arbitrary-precision balances gain 1 every two personal turns; draw 1 per turn; cooldown 1 is ready next personal turn. Limited modifiers; no empty-deck recycling/fatigue. | Broader card catalogue, augments, reusable modifiers and zone operations. | Costs, cooldowns and card access need match-based balance testing; choose an intentional long-game/empty-deck policy. |
| Victory and four players | Last-Royal removal; four-player elimination removes the defeated army, cards and jobs on a cross-shaped 14×14 board (36 corner cells removed). | Four orientations supported; victory and elimination details left open. | Royal elimination and FFA cross-board play are settled; no checkmate or teams. Evaluate seat advantage and elimination incentives. |
| Engine and delivery | Shared effects, deterministic RNG, snapshots, replay tests, CLI preview/undo; constrained triggers and choices. | General modifiers/augments, conditional effects, pending choices, richer event matching, save/load and validation support. | Prioritize reliable persistence and understandable interaction before expanding abstractions without a gameplay use. Legacy UI integration remains separate. |

The main remaining implementation gap is **general systems and delivery**, not
missing catalogue pieces. The listed status, clock, economy and match rules are now settled. The main
game-design work is **testing balance** and remaining targeting/lifecycle details.
Recommended next work: run two- and four-player playtests, then prioritize persistence,
UI integration and the card/modifier mechanics those sessions actually need.

For the longer technical inventory, see [DESIGN_GAP.md](DESIGN_GAP.md).
