# Tournament Test Infrastructure

This folder contains the endpoint-driven tournament e2e suite for the API.

## Entry Point

- The integration target is `api/tests/mod.rs`.
- Cargo runs it as a custom test target with `harness = false`, so the suite can parse its own CLI args.
- The file still exposes `pub mod tournament;` so the actual implementation stays under this folder.

## How To Run

Use the custom `itr` argument to select a deterministic random seed.

```bash
cargo test --test mod -- --itr 1
```

Supported forms:

- `--itr 1`
- `--itr=1`
- `itr=1`

`itr` changes the seeded random winners used during swiss and playoff play while remaining reproducible for the same value. Swiss simulation also injects a deterministic `20%` draw chance, so the same `itr` reproduces the same drawn matches too.

## Logging And Artifacts

Each run writes markdown logs under the repo root:

- `logs/test_tournament_itrN/matches.md`
- `logs/test_tournament_itrN/standings.md`
- `logs/test_tournament_itrN/tournament.md`

The same run also prints round pairings, match outcomes, standings snapshots, and summary notes to stdout.

## Suite Layout

- `config.rs`: parses CLI args, computes the deterministic seed, and resolves the repo-root log directory
- `harness.rs`: copies the SQLite snapshot, boots the in-process Axum app, and exposes typed endpoint helpers
- `model/`: API DTOs, internal tracker state, standings sort logic, swiss pairing logic, and playoff expectations
- `support.rs`: shared login, scoring, confirmation, and spirit submission helpers
- `assertions.rs`: public API consistency checks for matches, standings, player stats, and playoff grids
- `simulation.rs`: seeded outcome generation for swiss and playoff matches
- `reporting.rs`: CLI logging plus markdown report generation
- `scenarios/`: the actual scenario runners

The harness copies `sakkath.db` and its WAL file into a temp directory for each run. It intentionally does not copy SQLite shared-memory state.

## Scenario Coverage

### `match-flow`

- Verifies match visibility for admin, super-admin, and both POCs
- Resets `Round 1` reporting to disabled through `/v1/super/reporting-rounds/1` when the copied DB snapshot already has it enabled, so the scenario stays deterministic
- Starts a match, records events, ends it, confirms score from both teams, and submits spirit rows
- Confirms the API rejects new events after the score is finalized
- Re-checks public match, team, standings, and player-stat surfaces

### `full-tournament`

- Loads all swiss rounds across both divisions
- Uses seeded randomness to decide winners and a deterministic `20%` swiss draw rate
- Validates standings and rematch-aware swiss pairings after each round
- Runs playoff rounds `1001` and `1002`
- Verifies final public stats, team match histories, grid seed labels, and standings state
- Verifies `/v1/standings` reflects playoff seed-holder swaps after round `1001` and final placement swaps after round `1002`
- Verifies `/v1/teams/:id` `current_rank` matches the same API-visible final placement order
- Verifies elimination matches contribute to displayed `wins`, `losses`, `points_for`, `points_against`, `games_played`, and all-match spirit average while placement order still follows seed swapping instead of swiss sorting

## Design Rules

- Tests hit `/v1` endpoints only
- No direct table mutation is used to advance match state
- The tracker mirrors expected public state and is compared back against API responses
- Every run starts from a copied SQLite snapshot so the repo DB remains untouched