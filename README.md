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

### Creating or Refreshing `release_v1`

```bash
bash release.sh [--database_replace true|false] [--upload true|false]
```

Examples:

```bash
bash release.sh
bash release.sh --database_replace true
bash release.sh --upload true
bash release.sh --database_replace true --upload true
```

Defaults:
- `--database_replace false`
- `--upload false`

What `release.sh` does every run:
1. Builds the Rust API in release mode.
2. Builds the Next.js UI static export.
3. Replaces sakkath-api and UI static bundle.
4. If `database_replace` is true copies the whole local db.
5. If `upload` is true, pushes release to prod.

### Release Directory Structure

After running `release.sh`, the local release stays at:

```
release_v1/
├── api/
│   ├── sakkath-api      # Compiled Rust binary
│   └── .env             # Existing release env, preserved by release.sh
├── backups/             # Remote DB snapshots
├── ui/                  # Static UI files
└── sakkath.db           # SQLite DB
```

## Running a Release

### Starting the Release

Navigate to the api release directory:

```bash
cd release_v1/api
```

**Start the API:**
```bash
./sakkath-api
```

This will start,
- The ui on http://localhost:9000
- The api on http://localhost:9000/v1

If `NEXT_PUBLIC_API_URL` is not set during the UI build, the exported UI now uses same-origin `/v1` calls in release, while local dev on port `3000` still falls back to `http://localhost:9000`.