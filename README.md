# Sakkath Cloud

This is a standalone binary for Sakkath 2026. It serves both APIs and static UI pages via a Rust server and uses an embedded SQLite database.

## Requirements (Dev)

- **Rust** (latest stable version)
- **Node.js** (v20 or higher)
- **npm**
- **cargo-watch**

## Project Structure

```
sakkath-cloud/
├── api/          # Rust backend API
├── ui/           # Next.js frontend
├── build.sh      # Build script for releases
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

## Building for Production

### Creating a Release

```bash
bash build.sh <version>
```

**Example:**
```bash
bash build.sh 1
```

This will:
1. Create a `release_v<version>` directory
2. Build the Rust API in release mode
3. Build the Next.js UI for production
4. Set up the `.env` file with `release=true` and copy db file

### Release Directory Structure

After building, you'll have:

```
release_v1/
├── api/
│   ├── sakkath-api      # Compiled Rust binary
│   └── .env             # Environment configuration
├── ui/                  # Static UI files
└── sakkath.db          # Database
```

## Running a Release

### Starting the Release

Navigate to the api release directory:

```bash
cd release_v<version>/api
```

**Start the API:**
```bash
./sakkath-api
```

This will start,
- The ui on http://localhost:9000
- The api on http://localhost:9000/v1