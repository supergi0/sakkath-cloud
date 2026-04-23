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

`itr` changes the seeded random winners used during swiss and playoff play while remaining reproducible for the same value.

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

## Scenario Coverage

### `match-flow`

- Verifies match visibility for admin, super-admin, and both POCs
- Starts a match, records events, ends it, confirms score from both teams, and submits spirit rows
- Confirms the API rejects new events after the score is finalized
- Re-checks public match, team, standings, and player-stat surfaces

### `full-tournament`

- Loads all swiss rounds across both divisions
- Uses seeded randomness to decide winners and occasional swiss draws
- Validates standings and rematch-aware swiss pairings after each round
- Runs playoff rounds `1001` and `1002`
- Verifies final public stats, team match histories, grid seed labels, and standings state

## Design Rules

- Tests hit `/v1` endpoints only
- No direct table mutation is used to advance match state
- The tracker mirrors expected public state and is compared back against API responses
- Every run starts from a copied SQLite snapshot so the repo DB remains untouched