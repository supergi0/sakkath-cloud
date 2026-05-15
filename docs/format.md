# Tournament Format

## Overview

Both divisions play 6 Swiss rounds.

After Swiss:

- Open seeds 1-8 go into Sunday semifinal and placement rounds, while seeds 9-22 go straight into Sunday direct placement games.
- Women seeds 1-4 go into Sunday semifinal and placement rounds, while seeds 5-10 go straight into Sunday direct placement games.

- All Swiss games run for 65 minutes, with 15 minutes between scheduled starts.
- Sunday playoff and placement games run for 75 minutes.
- Sunday playoff and placement games must finish with a winner.

## Swiss goals

The Swiss generator is trying to do three things at the same time:

- keep teams with same records nearby
- avoid Swiss rematches
- make the late-round cut lines more meaningful

## Round 1

Round 1 is seeded from the starting order.

The field is split into a top half and a bottom half, and the pairings are made across that split.

Example for a 10-team division:

- 1 plays 6
- 2 plays 7
- 3 plays 8
- 4 plays 9
- 5 plays 10

## Rounds 2 to 6

From Round 2 onward, the event uses Swiss standings.

Swiss points are:

- win = 2
- draw = 1
- loss = 0

The standings order is:

1. Swiss points
2. Head-to-head
3. Median Buchholz
4. Point difference
5. Points scored
6. Momentum
7. Random fallback

## Pairing search

The scheduler does not lock teams into rigid score groups first. It starts from the full standings and asks a question:

- what is the smallest points-gap rule that allows a complete round with no Swiss rematches?

It tries the strictest option first:

- same-points games only

If that cannot produce a full legal round, it widens only as much as needed:

- allow a 1-point gap
- then 2 points
- then 3 points
- and so on

Once it has a legal gap, it still prefers the most natural Swiss-looking round. In practice that means it prefers:

1. more same-points pairings
2. smaller points gaps
3. smaller standings-rank gaps
4. more neighbour-style pairings from the standings list

## Rounds 4, 5, and 6

Round 4 adds a mild cut-line bias. If two legal no-rematch rounds are otherwise very close, the system leans toward pairings near the important borders such as:

- 4v5
- 8v9
- 12v13
- 16v17

Rounds 5 and 6 add bounded lookahead.

The current late-round pipeline is:

1. find the smallest legal points-gap rule
2. explore up to 384 full no-rematch candidates
3. keep the top 192 candidates by Swiss shape and boundary usefulness
4. simulate 72 deterministic futures for each kept candidate

Those futures focus on the reported cut-line pairs:

- 4v5
- 3v6
- 8v9
- 7v10

The late-round chooser prefers the candidate that minimizes the worst same-points miss risk at those boundaries, and only after that falls back to the normal Swiss-style preferences.

## Slot assignment inside a round

The published Swiss slot map stays fixed.

Inside that fixed map, the actual generated Swiss pairings are shuffled before insertion. That means:

- the slot codes stay stable for operations and publishing
- the teams are randomized across those available slots within the round

So the scheduler now keeps the published layout while avoiding a predictable seed-to-slot pattern.

## Rematches

The no-rematch rule applies to Swiss only.

Once Sunday placement begins, rematches are allowed if that is what the bracket or direct placement path produces.