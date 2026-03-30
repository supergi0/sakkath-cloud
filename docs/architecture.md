# Architecture

## What this app is

Sakkath Cloud has three main parts:

- `ui/` is the Next.js frontend
- `api/` is the Rust backend
- `sakkath.db` is the SQLite database used by the backend

There is also optional Redis caching. If Redis is unavailable, the app still runs.

## How requests flow

1. A user opens the UI in `ui/`
2. The UI calls the Rust API under `/v1`
3. The API reads or writes tournament data in SQLite
4. Some read-heavy responses are cached in Redis

In release mode, the Rust server also serves the built UI. In development, the UI and API run separately.

## Main backend responsibilities

- `api/src/main.rs` starts the server, database, cache, and auto-generation flow
- `api/src/routes.rs` defines the API routes
- `api/src/controllers/` handles auth, teams, matches, scheduling, and announcements
- `api/src/helpers/sorting.rs` computes standings and tiebreakers
- `api/src/helpers/rounds.rs` generates swiss pairings
- `api/src/migration.rs` creates tables and seeds demo data on an empty database
- `api/src/seeder.rs` inserts the current mock tournament data

## Main database tables

- `teams`: tournament teams, division, seed, logos, location
- `users`: admins, POCs, and players
- `fields`: playable fields and map links
- `matches`: pairings, scores, field, time, spirit, match phase
- `match_events`: goals, assists, blocks, turnovers
- `announcements`: public updates

## Where the current data comes from

On first startup, `api/src/migration.rs` checks whether the database is empty. If it is, it calls `api/src/seeder.rs` and inserts demo teams, players, fields, matches, and announcements.

That means the current app is not yet loading real tournament data from an admin import flow. It is loading seeded sample data.

## How to use real data

For real usage, keep the schema in `api/src/migration.rs` but replace the seeded records.

The practical options are:

1. Edit `api/src/seeder.rs` so it inserts your real teams, players, fields, opening matches, and announcements
2. Or stop calling the seed path and load data into `sakkath.db` through SQL or a separate import script

Minimum real data needed before the app is useful:

- teams with correct `division` and `init_rank`
- users linked to the right `team_id`
- fields
- initial matches with `t1_id`, `t2_id`, `field_id`, `time`, and `type`

## Important operational notes

- `division`: `0 = Open`, `1 = Women`
- `matches.type`: `1..N = swiss rounds`, `1001 = playoffs`, `1002 = finals`
- `possession`: `NULL = not started`, `1 or 2 = live`, `>= 3 = completed`
- completed match scores should not be tied, because the standings logic assumes a winner