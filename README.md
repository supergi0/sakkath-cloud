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
bash release.sh [--database_upload true|false] [--upload true|false]
```

Examples:

```bash
bash release.sh
bash release.sh --database_upload true
bash release.sh --upload true
bash release.sh --database_upload true --upload true
```

Defaults:
- `--database_upload false`
- `--upload false`

What `release.sh` does every run:
1. Builds the Rust API in release mode.
2. Builds the Next.js UI static export.
3. Replaces only `release_v1/api/sakkath-api` locally.
4. Replaces the entire local `release_v1/ui` bundle.
5. Leaves `release_v1/api/.env` untouched.

What changes when `--database_upload true`:
1. Copies the repo root `sakkath.db` into `release_v1/sakkath.db`.
2. Copies `sakkath.db-shm` and `sakkath.db-wal` too when they exist.
3. Removes stale `release_v1/sakkath.db-shm` or `release_v1/sakkath.db-wal` if they do not exist locally.

What changes when `--upload true`:
1. Loads SSH and remote path settings from the root `.env` file.
2. SSHes into the VM.
3. Stops `sakkath.service`.
4. Replaces the remote API binary only, without touching `release_v1/api/.env`.
5. Replaces the remote `release_v1/ui` folder.
6. Replaces remote database files only when `--database_upload true`.
7. Starts `sakkath.service` again and prints its status.

### Release Directory Structure

After running `release.sh`, the local release stays at:

```
release_v1/
├── api/
│   ├── sakkath-api      # Compiled Rust binary
│   └── .env             # Existing release env, preserved by release.sh
├── ui/                  # Static UI files
├── sakkath.db           # Database
├── sakkath.db-shm       # Optional SQLite WAL companion
└── sakkath.db-wal       # Optional SQLite WAL companion
```

### Root `.env` for Cloud Uploads

`release.sh` reads deployment credentials from the root `.env` file in the repo. These values are only required when `--upload true`.

```bash
DEPLOY_SSH_USER="sakkathultimate"
DEPLOY_SSH_HOST="YOUR_VM_IP"
DEPLOY_SSH_PORT="22"
DEPLOY_SSH_KEY_PATH="/absolute/path/to/private/key"
DEPLOY_REMOTE_RELEASE_DIR="/home/sakkathultimate/release_v1"
DEPLOY_REMOTE_SERVICE_NAME="sakkath.service"
```

Keep API runtime secrets in `release_v1/api/.env`. Do not put the API's runtime `.env` into the root `.env`; the root file is for deploy credentials only.

### GCP SSH Key Setup

Generate a key pair locally:

```bash
ssh-keygen -t ed25519 -C "sakkath-release" -f ~/.ssh/sakkath_release
```

That gives you:
- private key: `~/.ssh/sakkath_release`
- public key: `~/.ssh/sakkath_release.pub`

Add the public key to the VM user in GCP:
1. In Google Cloud Console, go to `Compute Engine` -> `VM instances`.
2. Open the VM details for your instance.
3. Click `Edit`.
4. Find `SSH Keys`.
5. Paste the contents of `~/.ssh/sakkath_release.pub`.
6. Save.

Then set the matching private key path in the root `.env`:

```bash
DEPLOY_SSH_KEY_PATH="/home/<your-user>/.ssh/sakkath_release"
```

To find the VM IP for `DEPLOY_SSH_HOST`:
1. In `Compute Engine` -> `VM instances`, copy the external IP from the instance row.
2. Put that IP in the root `.env`.

You can test access before the script uploads anything:

```bash
ssh -i ~/.ssh/sakkath_release sakkathultimate@YOUR_VM_IP
```

If you prefer `gcloud`, you can also add keys with:

```bash
gcloud compute os-login ssh-keys add --key-file=~/.ssh/sakkath_release.pub
```

Use that only if your VM is configured for OS Login. Otherwise, the VM `SSH Keys` section is the simpler path.

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