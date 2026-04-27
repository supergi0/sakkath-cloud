# Tournament Format

This summary explains the tournament format.

## Overview

Each division has:

- 6 Swiss draw rounds
- then placement or playoff games after Swiss

The Swiss system is trying to do three things at once:

- keep teams playing others with a similar record
- avoid Swiss rematches completely
- make the late rounds more useful around the important cut lines

## Round 1

Round 1 is seeded from the starting ranking.

The field is split into a top half and a bottom half, and teams are paired across that split.

For example, in a 10-team division:

- 1 plays 6
- 2 plays 7
- 3 plays 8
- 4 plays 9
- 5 plays 10

## Rounds 2 to 6

From Round 2 onward, the draw uses Swiss standings.

The Swiss points system is:

- win = 2 points
- draw = 1 point
- loss = 0 points

After every completed round, teams are sorted in the standings by:

1. Swiss points
2. Head-to-head, but only if exactly 2 teams are tied on points
3. Median Buchholz
4. Point difference
5. Points scored
6. Momentum
7. Starting seed

## How the pairing search works now

Instead of locking the whole round into rigid score brackets, the system works from the full standings list.

It asks one question first:

- what is the smallest points gap that allows a complete round with no rematches?

It starts with the strictest version:

- only allow teams to play opponents on the same points

If a full rematch-free round is impossible, it widens gently:

- allow a 1-point gap
- then a 2-point gap
- then a 3-point gap
- and so on, only as far as needed

So the draw widens only enough to make a full no-rematch round exist.

## What it prefers inside that search

Once the smallest workable gap is found, the system still tries to stay close to natural Swiss pairings.

Its preference order is roughly:

1. more same-points pairings
2. smaller points gaps
3. smaller standings-rank gaps
4. more natural neighbour-style pairings from the standings list

## Round 4, 5, and 6 behaviour

Round 4 adds a mild cut-line bias.

That means if two valid no-rematch choices are otherwise very similar, the system leans a little toward pairings near the important borders such as:

- 4v5
- 8v9
- 12v13
- 16v17

## Late-round pipeline

In Rounds 5 and 6, the system works in four layers:

1. viable set
2. exploration
3. candidates
4. simulation and probability scoring

### 1. Viable set

First, it finds the smallest points-gap rule that allows a full round with no Swiss rematches.

That creates the viable set of legal pairings for that round.

### 2. Exploration

From that viable set, the system explores up to 384 full no-rematch round candidates.

Those explored rounds are still ordered by Swiss logic first, which means the search still prefers:

- same-points pairings
- smaller points gaps
- smaller rank gaps
- natural neighbour-style pairings

In late rounds, the Swiss score also gives extra weight to pairings that cross the important cut lines.

### 3. Candidates

From the explored rounds, the system keeps the top 192 candidates.

This is the favourite set that survives into lookahead.

The reason for keeping a larger set is simple:

- some rounds look slightly less natural by plain Swiss shape
- but are much better for the important equal-points boundaries later

So the shortlist is deliberately wide enough to keep those options alive.

### 4. Simulation and probability scoring

Each kept candidate is then simulated through the remaining Swiss future.

Both Round 5 and Round 6 now run 36 deterministic future scenarios per kept candidate.

After those futures are played out, the system checks the four reported boundary pairs:

- 4v5
- 3v6
- 8v9
- 7v10

For each pair, it asks:

- did those two teams finish on the same Swiss points?
- if yes, had Swiss already made them play each other?

That produces the probability that a boundary finishes as a `same-points miss` (4 and 5 at same points but didn't play).

The late-round chooser then prefers the candidate with the lowest worst-case `same-points miss` probability, and only after that falls back to the normal Swiss-style tie-breaks.

## What the lookahead is trying to improve

The late-round lookahead is trying to improve the important seed-border games by the end of Swiss.

Examples:

- 4v5 and 3v6
- 8v9 and 7v10

## Example Runthrough

Imagine a 10-team division with starting seeds 1 to 10.

### Round 1

Round 1 is the only round driven directly by the starting seed.

The field is split into a top half and a bottom half:

- 1 plays 6
- 2 plays 7
- 3 plays 8
- 4 plays 9
- 5 plays 10

### Round 2

Now the event switches to Swiss standings.

Teams are paired by Swiss points first.

If a score group has an odd number of teams, one team floats the minimum amount needed to complete the round with no rematch.

So if five teams are sitting on 2 points, the round might look like:

- 1 plays 2
- 3 plays 4
- 5 floats to the nearest legal opponent on the next score line

The key point is that the system widens only as much as needed.

### Round 3

The same Swiss rule continues.

If the most natural same-points pairing would repeat a match that already happened in Swiss, the system does not allow it.

Instead, it looks for the smallest possible widening that still gives a full legal round.

### Round 4

Round 4 is still a normal Swiss round first.

But now there is a mild extra preference for games, such as:

- 4v5
- 8v9

That does not mean Round 4 forces those games.

It means that if two legal no-rematch rounds are otherwise very close, the system leans toward the one that is more useful around those cut lines.

### Round 5

Round 5 starts the late-round lookahead.

The system does this in order:

1. explore up to 384 legal full-round candidates, starting from 0 to n point gap.
2. keep the best 192 Swiss-shaped candidates
3. simulate 36 deterministic futures for each kept candidate

Those simulations ask a practical question:

- if two teams finish on the same Swiss points around an important boundary such as 4v5 or 3v6, did Swiss actually make them play each other?

That is why a Round 5 pairing can beat a slightly more natural-looking alternative if it gives a better late-round boundary outcome.

### Round 6

Round 6 uses the same late-round pipeline again.

Because this is the final Swiss round, the choice is even more direct.

Among the legal no-rematch options, the system prefers the round that best reduces same-points misses at the reported boundaries:

- 4v5
- 3v6
- 8v9
- 7v10

So by Round 6, the draw is still Swiss-like, but it is also deliberately trying to make the important equal-points borders more meaningful.

### What this example is showing

The exact pairings will depend on results, standings, and rematch history.

What stays consistent is the order of decisions:

- Round 1 uses the starting seed
- Rounds 2 and 3 use normal Swiss standings with minimal widening
- Round 4 adds a mild border bias
- Rounds 5 and 6 add bounded lookahead on top of the Swiss search

## After Swiss

After Round 6, the event stops being Swiss.

Teams are then split into seed-order brackets.

In a 10-team division, that means:

- seeds 1 to 4 form the top bracket
- seeds 5 to 8 form the next bracket
- seeds 9 and 10 form a 2-team bracket

For each 4-team bracket, the playoff round is:

- 1 plays 4
- 2 plays 3

For a 2-team bracket, there is one direct playoff game:

- 9 plays 10

After that, the finals round is built from the current seed holders inside each 4-team bracket.

If a lower seed upsets a higher seed, it takes over that seed line inside the bracket.

Example in the top bracket:

- Swiss ends with seeds 1, 2, 3, 4
- the playoff pairings are 1v4 and 2v3
- if seed 4 beats seed 1, team 4 takes over the number-1 seed line
- if seed 2 beats seed 3, team 2 keeps the number-2 seed line
- the finals round for that bracket then becomes 4v2 for the higher placing game and 3v1 for the lower placing game

So rematches are allowed after Swiss.

The no-rematch rule applies to Swiss rounds only, not to the post-Swiss placement structure.