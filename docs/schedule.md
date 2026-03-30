# Schedule

## Tournament model in the current code

The scheduler is split between two files:

- `api/src/helpers/sorting.rs` ranks teams
- `api/src/helpers/rounds.rs` creates swiss pairings

The controller in `api/src/controllers/scheduling.rs` decides when to generate the next round, playoffs, and finals.

## Standings rules

Official standings use completed swiss matches only. Live standings can temporarily include in-progress scores for display.

Teams are sorted by these criteria, in this exact order:

- `C1`: total wins
- `C2`: head to head result
- `C3`: Buchholz, which is the total wins of all opponents faced
- `C4`: point difference
- `C5`: total points scored
- `C6`: momentum score, which rewards earlier wins more than later wins
- `C7`: random coin flip

`C7` is a real random tiebreak in the current code, so two fully tied teams can swap order.

## How swiss rounds are generated

Round 1 is generated from seeding if a division has no matches yet.

The current code uses `OPEN_ROUNDS = 5` and `WOMEN_ROUNDS = 5`.

For later rounds:

1. Teams are sorted using `C1` to `C7`
2. Teams are grouped by equal win count
3. If a group has an odd size, the last team is pushed into the next group
4. Inside each group, the top half is paired against the bottom half
5. If a rematch appears, backtracking tries alternate pairings
6. If a whole group fails, the scheduler swaps teams with an adjacent group and retries
7. If everything fails, it force-pairs the group as a last fallback

The scheduler will not create the next swiss round until the previous one is fully completed.

## How playoffs and finals are handled

After all 5 swiss rounds are completed, the controller creates post-swiss matches for the whole standings table.

The standings are split into brackets of 4 teams in seed order:

- `1 to 4`
- `5 to 8`
- `9 to 12`
- and so on

If the last bracket has only 2 teams, that bracket gets only one match.

First post-swiss round:

- match type is `1001`
- each 4-team bracket plays `1 vs 4` and `2 vs 3`
- each 2-team bracket plays one direct match

Second post-swiss round:

- match type is `1002`
- each 4-team bracket then plays `1 vs 2` and `3 vs 4`
- 2-team brackets do not get a second match

This second round is seed-based. It does not depend on who won the first post-swiss round.

## Match state fields that drive scheduling

- `matches.type` decides whether a match is swiss, playoff, or final
- `matches.time` is the scheduled kickoff time
- `matches.field_id` chooses the field
- `matches.possession` decides status

Status meaning:

- `NULL`: not started
- `1` or `2`: live
- `>= 3`: completed

The scheduler uses completion state, not just score, to decide whether the next stage can be generated.


