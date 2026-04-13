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
- `api/src/controllers/scheduling.rs` also owns the fixed Friday/Saturday/Sunday slot grid and super-admin schedule edits
- `api/src/migration.rs` resolves the SQLite path, creates tables, verifies startup state, and dispatches the selected seed source
- `api/src/seeder.rs` inserts the current mock tournament data
- `api/src/helpers/csv_seed.rs` imports filtered team and roster data from `teams.csv` into the existing schema
- `teams.abbreviation` stores an optional compact code that public and admin UIs can prefer over generated abbreviations

## Main database tables

- `teams`: tournament teams, division, seed, logos, location
- `users`: admins, POCs, and players
- `fields`: playable fields and map links
- `matches`: pairings, scores, field, time, spirit, match phase
- `match_events`: goals, assists, blocks, turnovers
- `announcements`: public updates

## Database location and lifecycle

The default SQLite file lives one level above the runtime `api/` directory:

- development from `api/`: `<repo>/sakkath.db`
- release from `release_v1/api/`: `release_v1/sakkath.db`

Normal server startup does not create or seed a missing database file.

Available CLI commands:

1. `sakkath-api create-database` creates the SQLite file and runs schema migrations
2. `sakkath-api seed-database` seeds demo data into an empty migrated database
3. `sakkath-api seed-database csv [path]` seeds filtered team and roster data from `teams.csv`

To inspect the SQLite database directly from a shell:

1. `cd /mnt/work/sakkath-cloud`
2. `sqlite3 sakkath.db`
3. run `.tables` to list all tables

You can also run it in one line:

`sqlite3 /mnt/work/sakkath-cloud/sakkath.db '.tables'`

That keeps production safe when you want to load real tournament data without accidental demo inserts.

## Where the current data comes from

The sample tournament data still lives in `api/src/seeder.rs`, but it only runs when `seed-database` is called explicitly.

## How to use real data

For real usage, keep the schema in `api/src/migration.rs` and skip the seed step.

The practical options are:

1. Edit `api/src/seeder.rs` only if you intentionally want a reusable bulk import path
2. Or load real data into `sakkath.db` through SQL or a separate import script

Minimum real data needed before the app is useful:

- teams with correct `division` and `init_rank`
- users linked to the right `team_id`
- fields
- initial matches with `t1_id`, `t2_id`, `field_id`, `time`, and `type`

## Schedule structure

- The tournament now uses 6 swiss rounds for both divisions
- Friday holds rounds 1 to 3, Saturday holds rounds 4 to 6, and Sunday holds playoff rounds 1 and 2
- The public schedule is one combined 4-ground grid for Open and Women together
- Fixed row timing lives in the scheduler and can be overridden by SUPER users through schedule APIs without changing the database schema

## Important operational notes

- `division`: `0 = Open`, `1 = Women`
- `matches.type`: `1..N = swiss rounds`, `1001 = playoffs`, `1002 = finals`
- `possession`: `NULL = not started`, `1 or 2 = live`, `>= 3 = completed`
- completed match scores should not be tied, because the standings logic assumes a winner