# Sakkath Cloud

This is a standalone binary for Sakkath 2026. It serves both APIs and static UI pages via the same server and uses an embedded SQLite database.

## Requirements (Dev)

- **Rust** (latest stable version)
- **Node.js** (v20 or higher)
- **npm**
- **cargo-watch**
- **redis**

## Project Structure

```
sakkath-cloud/
├── api/          # Rust backend API
├── ui/           # Next.js frontend
├── release.sh    # Local release + optional cloud deploy script
├── .env          # Root deploy credentials for release.sh
└── start.sh      # Development start script
```

## Development Mode

### Starting the API (Development)

```bash
bash start.sh api
```

This will:
- Start the Rust API on port **9000**
- Automatically rebuild on file changes
- Make a sqlite database from migrate.rs (if not present)

### Starting the UI (Development)

```bash
bash start.sh ui
```

This will:
- Start the Next.js development server on port **3000**
- Enable hot-reloading for React components

## Release Flow

### Creating or Refreshing a Release

```bash
bash release.sh [--prod true|false] [--database_replace true|false] [--upload true|false]
```

Examples:

```bash
bash release.sh
bash release.sh --prod true
bash release.sh --database_replace true
bash release.sh --upload true
bash release.sh --prod true --upload true
bash release.sh --database_replace true --upload true
```

Defaults:
- `--prod false` which targets staging at `release_staging_v1` on port `9001`
- `--database_replace false`
- `--upload false`

What `release.sh` does every run:
1. Builds the Rust API in release mode.
2. Builds the Next.js UI static export.
3. Replaces the target release's `sakkath-api` binary and UI static bundle.
4. Prepares the target release `api/.env` with `release=true`, the target `PORT`, and a mode-specific `REDIS_NAMESPACE`.
5. If `database_replace` is true copies the whole local db.
6. If `upload` is true, pushes the release to staging or prod on the configured EC2 host.

### Release Directory Structure

After running `release.sh`, the local release stays at:

```
release_v1/ or release_staging_v1/
├── api/
│   ├── sakkath-api      # Compiled Rust binary
│   └── .env             # Prepared by release.sh for the selected target
├── backups/             # Remote DB snapshots
├── ui/                  # Static UI files
└── sakkath.db           # SQLite DB
```

## Running a Release

### Starting the Release

Navigate to the api release directory:

```bash
cd release_staging_v1/api  # or release_v1/api for prod
```

**Start the API:**
```bash
./sakkath-api
```

This will start,
- The ui on `http://localhost:9001` for staging or `http://localhost:9000` for prod
- The api on the same port under `/v1`

If `NEXT_PUBLIC_API_URL` is not set during the UI build, the exported UI now uses same-origin `/v1` calls in release, while local dev on port `3000` still falls back to `http://localhost:9000`.