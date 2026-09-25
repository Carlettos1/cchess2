# Settled gameplay rules

Authoritative decisions, 2026-09-23. These supersede conflicting provisional
choices in earlier implementation notes and the original design checklist.

| Topic | Rule |
| --- | --- |
| Magician | Fire and Ice are separate casts. |
| Immune | Cleanses existing statuses and blocks spell/ability/status effects, including self, allied and global effects. Ordinary Take and Attack retain their existing tag rules; Heroic separately blocks Attack. |
| Ice | Allows Attack only. Move, Take and Ability are blocked. |
| Deactivate | Allows Move only. Take, Attack and Ability are blocked. |
| Stacking | Positive statuses retain the higher remaining duration; negative statuses retain the lower. Invulnerable is positive; Fire, Ice and Deactivate are negative. Equal values preserve the existing timer and attribution. |
| Victory | Elimination of the last Royal. Checkmate is not evaluated. Four-player matches are FFA; last surviving player wins. |
| Clocks | Tick at the start of their movement, turn or round. Ordinary spell delays use the caster's owner-turn clock: delay 1 resolves at the start of that owner's next turn. Explicit global-turn and round clocks remain distinct. |
| Cooldowns | Decrease at each owner turn start. Cooldown 1 is ready next owner turn, so it can be cast every personal turn, subject to other costs and eligibility. |
| Mana | Normal matches start at 1. Gain 1 after every two personal turns: starts of personal turns 3, 5, 7, etc. No gameplay cap. |
| Cards | Draw one at each subsequent owner turn start, in addition to draws granted by effects. The existing three-card opening hand remains. |
| Four-player board | Classical: 14×14 cross with 3×3 corners removed (160 cells). Full armies: 24×24 cross with 4×4 corners removed (512 cells). Removed cells cannot be selected, occupied, built on or crossed by rays. Leaps still jump; their landing cell must exist. |

Turn start establishes the incoming player and resets acted state, advances
turn/round counters before lethal effects can skip a tick, ticks status/cooldown
timers and resolves clock effects, draws cards, resolves
due owner work and fires owner-start card triggers before accepting input.
Jobs created during a tick wait for a future tick. A pass advances the turn but
is not a movement; promotion choices do not add movement ticks. Movement-start
work that invalidates an already selected action commits its effects and reports
an interruption without charging the invalidated action.

Durations count future owner starts. For example, an enemy Fire(1) expires at
that enemy's next turn start; a disable with duration 1 also expires there.
A negative reapplication with a shorter duration shortens the existing timer.

Immunity applies to effects received, not to casting at another eligible target.
It blocks Paladin's self-Ward and King's self-teleport. Gaining Immune through
transformation/promotion cleanses pending Fire without triggering its damage.
Player resource effects are not statuses on a piece. Card effects and card-supplied
modifiers cannot affect Immune pieces; Royal Decree therefore cannot discount an
Immune caster. All presets, including demonstration sandboxes, start with 1 mana.
Mana balances use arbitrary-precision unsigned values, with no saturation or
fixed integer maximum. Costs and individual grants remain nonnegative u32 values.

The Rook's fixed 5×5 selection and the completed Paladin branches remain as
previously specified, subject to these shared protection and timing rules.

Mana snapshot serialization now uses canonical little-endian 32-bit limbs.
Snapshots using the former scalar mana field require migration; no legacy
snapshot importer is provided.
