# Tournament Format

This summary explains how the tournament format works.

## Overview

Each division has:

- 6 Swiss draw rounds
- then placement/playoff games after Swiss

The system is built to do three things at the same time:

- keep teams playing others with a similar record
- avoid repeat matchups as much as possible
- still produce a full round with no team left out

## Round 1

Round 1 is seeded from the starting ranking.

All teams are split into a top half and a bottom half, then paired across that split.

For example, in a 10-team division:

- 1 plays 6
- 2 plays 7
- 3 plays 8
- 4 plays 9
- 5 plays 10

So Round 1 is not based on Swiss points yet. It is based on the starting seed.

## Rounds 2 to 6

From Round 2 onward, the pairings are based on Swiss points.

The points system is:

- win = 2 points
- draw = 1 point
- loss = 0 points

## How the Swiss grouping works

After each completed round, teams are sorted into the current standings.

The standings use these ideas in order:

1. Swiss points
2. Head-to-head if only 2 teams are tied on points
3. Median Buchholz
4. Point difference
5. Points scored
6. Momentum score (WWWLLL > LLLWWW)
7. Starting seed

Then the Swiss draw groups teams by equal points.

Example:

- Group A: teams on 8 points
- Group B: teams on 6 points
- Group C: teams on 4 points

Inside each group, teams stay in standings order.

## What happens if a group has an odd number of teams

Swiss rounds need even groups so everyone can be paired.

If a points group has an odd number of teams, the last team in that group is moved down into the next group.

This is a simple balancing step.

Example:

- 8-point group: seeds 1, 2, 3
- 6-point group: seeds 4, 5, 6, 7

The 8-point group has 3 teams, so it cannot be paired cleanly.

The system moves the last team from that group down:

- 8-point group becomes: 1, 2
- 6-point group becomes: 3, 4, 5, 6, 7

If the next group is then odd as well, the same balancing idea continues until the groups become pairable.

So the draw mostly stays within the same score band, but it can float one team down when needed.

## How pairing inside a group works

Once a group has an even number of teams, it is split into:

- a top half
- a bottom half

The natural Swiss pairing is then:

- first team in the top half vs first team in the bottom half
- second team in the top half vs second team in the bottom half
- and so on

For a 6-team group:

- group order: 3, 4, 5, 6, 7, 8
- top half: 3, 4, 5
- bottom half: 6, 7, 8
- natural pairs: 3v6, 4v7, 5v8

That is the first thing the system tries.

## How the backtracking works

Backtracking is the part that tries to avoid rematches.

It works in a this order:

1. take the first team from the top half
2. try its natural opponent first
3. check if they have already played
4. if they have not played, temporarily accept that pair
5. move to the next team and do the same
6. if later the system gets stuck, it goes back, removes the last temporary choice, and tries the next available opponent

So it keeps a small memory of:

- which teams have already been paired in this round
- who each team has already played in earlier rounds
- which trial pairings are currently being tested

In simple terms, it says:

- try the clean Swiss pair first
- if that causes a repeat game or blocks the rest of the group, step back and try the next option

## What does it check first, second, and so on

For the normal Swiss pairing search, the order is:

1. keep teams inside the same points group if possible
2. try the natural top-half vs bottom-half pairing first
3. reject any pairing that is a repeat of an earlier Swiss game
4. if that choice makes the rest of the group impossible, go back and try the next opponent

For rounds 5 and 6, there is one extra preference.

The system still avoids rematches first, but among all valid no-rematch options it now prefers cut-line games such as:

- 4v5 and 3v6
- 8v9 and 7v10
- 12v13 and 11v14
- 16v17 and 15v18
- 20v21 and 19v22

So late in Swiss, the draw tries harder to create meaningful placement games near the important seed lines.

## Simple example

Here is a small example with 10 teams after some rounds.

Current standings by Swiss points:

- 1: 8 points
- 2: 8 points
- 3: 6 points
- 4: 6 points
- 5: 6 points
- 6: 6 points
- 7: 4 points
- 8: 4 points
- 9: 2 points
- 10: 2 points

### Step 1: Build score groups

- 8-point group: 1, 2
- 6-point group: 3, 4, 5, 6
- 4-point group: 7, 8
- 2-point group: 9, 10

All groups are even, so no balancing is needed.

### Step 2: Split each group into halves

In the 6-point group:

- top half: 3, 4
- bottom half: 5, 6

Natural Swiss pairings would be:

- 3v5
- 4v6

### Step 3: Check for repeats

Suppose 3 has already played 5 in an earlier round.

Then the system tries:

- 3v5 -> reject, repeat game
- 3v6 -> accept for now

Now only 4v5 is left, so if that is also new, the group becomes:

- 3v6
- 4v5

That is the backtracking idea in action.

### Step 4: Late-round crossover preference

If this is Round 5 or Round 6, the system looks at all valid no-repeat options and prefers the one that better tests the cut line.

So in this same group it would prefer:

- 4v5
- 3v6

over a less useful placement combination, as long as no rematch is created.

### Step 5: If a group is odd

Now imagine the standings instead were:

- 8-point group: 1, 2, 3
- 6-point group: 4, 5, 6, 7
- 4-point group: 8, 9
- 2-point group: 10

The 8-point group is odd, so team 3 is floated down.

That becomes:

- 8-point group: 1, 2
- 6-point group: 3, 4, 5, 6, 7

Which balances to:

- 8-point group: 1, 2
- 6-point group: 3, 4, 5, 6
- 4-point group: 7, 8, 9

Which balances to:

- 8-point group: 1, 2
- 6-point group: 3, 4, 5, 6
- 4-point group: 7, 8
- 2-point group: 9, 10