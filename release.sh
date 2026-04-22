#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"
LOCAL_RELEASE_DIR="$REPO_ROOT/release_v1"
LOCAL_API_DIR="$LOCAL_RELEASE_DIR/api"
LOCAL_UI_DIR="$LOCAL_RELEASE_DIR/ui"
LOCAL_DB_PATH="$REPO_ROOT/sakkath.db"
ROOT_ENV_FILE="$REPO_ROOT/.env"

DATABASE_REPLACE="false"
UPLOAD="false"

REMOTE_SERVICE_STOPPED="false"
SSH_USER=""
SSH_HOST=""
SSH_PORT="22"
SSH_KEY_PATH=""
REMOTE_RELEASE_DIR=""
REMOTE_SERVICE_NAME=""

backup_remote_database() {
    local remote_backup_root="$REMOTE_RELEASE_DIR/backups"

    echo "Creating remote database backup snapshot..."
    ssh_cmd "set -e; backup_root='$remote_backup_root'; release_dir='$REMOTE_RELEASE_DIR'; timestamp=\$(date '+%Y%m%d-%H%M%S'); backup_dir=\"\$backup_root/\$timestamp\"; mkdir -p \"\$backup_dir\"; for db_file in sakkath.db sakkath.db-shm sakkath.db-wal; do if [ -f \"\$release_dir/\$db_file\" ]; then cp -p \"\$release_dir/\$db_file\" \"\$backup_dir/\$db_file\"; fi; done"
}

usage() {
    cat <<'EOF'
Usage: ./release.sh [--database_replace true|false] [--upload true|false]

Flags:
  --database_replace true|false   Replace release_v1 database files from the local repo root database. Default: false
  --upload true|false            Upload the built release to the remote server using root .env credentials. Default: false
  --help                         Show this help message.
EOF
}

normalize_bool() {
    local value="${1,,}"
    case "$value" in
        true|false)
            printf '%s\n' "$value"
            ;;
        *)
            echo "Invalid boolean value: $1. Use true or false." >&2
            exit 1
            ;;
    esac
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --database_replace)
                [[ $# -ge 2 ]] || { echo "Missing value for --database_replace" >&2; usage; exit 1; }
                DATABASE_REPLACE="$(normalize_bool "$2")"
                shift 2
                ;;
            --upload)
                [[ $# -ge 2 ]] || { echo "Missing value for --upload" >&2; usage; exit 1; }
                UPLOAD="$(normalize_bool "$2")"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                echo "Unknown argument: $1" >&2
                usage
                exit 1
                ;;
        esac
    done
}

require_file() {
    local file_path="$1"
    local description="$2"

    if [[ ! -f "$file_path" ]]; then
        echo "Missing ${description}: $file_path" >&2
        exit 1
    fi
}

load_root_env() {
    if [[ ! -f "$ROOT_ENV_FILE" ]]; then
        echo "Missing root .env file at $ROOT_ENV_FILE" >&2
        exit 1
    fi

    set -a
    # shellcheck disable=SC1090
    source "$ROOT_ENV_FILE"
    set +a

    SSH_USER="${DEPLOY_SSH_USER:-}"
    SSH_HOST="${DEPLOY_SSH_HOST:-}"
    SSH_PORT="${DEPLOY_SSH_PORT:-22}"
    SSH_KEY_PATH="${DEPLOY_SSH_KEY_PATH:-}"
    REMOTE_RELEASE_DIR="${DEPLOY_REMOTE_RELEASE_DIR:-}"
    REMOTE_SERVICE_NAME="${DEPLOY_REMOTE_SERVICE_NAME:-sakkath.service}"

    local missing=()
    [[ -n "$SSH_USER" ]] || missing+=("DEPLOY_SSH_USER")
    [[ -n "$SSH_HOST" ]] || missing+=("DEPLOY_SSH_HOST")
    [[ -n "$SSH_KEY_PATH" ]] || missing+=("DEPLOY_SSH_KEY_PATH")
    [[ -n "$REMOTE_RELEASE_DIR" ]] || missing+=("DEPLOY_REMOTE_RELEASE_DIR")

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Missing required deployment values in $ROOT_ENV_FILE: ${missing[*]}" >&2
        exit 1
    fi

    if [[ ! -f "$SSH_KEY_PATH" ]]; then
        echo "SSH key file not found: $SSH_KEY_PATH" >&2
        exit 1
    fi
}

build_release_artifacts() {
    echo "Building Rust API..."
    (
        cd "$REPO_ROOT/api"
        cargo build --release
    )

    echo "Building Next.js UI..."
    (
        cd "$REPO_ROOT/ui"
        npm run build
    )
}

sync_local_release() {
    local api_binary="$REPO_ROOT/api/target/release/sakkath-api"
    local ui_build_dir="$REPO_ROOT/ui/out"

    require_file "$api_binary" "API binary"

    if [[ ! -d "$ui_build_dir" ]]; then
        echo "Missing UI export directory: $ui_build_dir" >&2
        exit 1
    fi

    mkdir -p "$LOCAL_API_DIR"
    rm -rf "$LOCAL_UI_DIR"
    mkdir -p "$LOCAL_UI_DIR"

    echo "Updating local API binary in release_v1/api..."
    cp "$api_binary" "$LOCAL_API_DIR/sakkath-api"
    chmod +x "$LOCAL_API_DIR/sakkath-api"

    if [[ ! -f "$LOCAL_API_DIR/.env" ]]; then
        echo "Warning: $LOCAL_API_DIR/.env does not exist. The release API env was left untouched." >&2
    fi

    echo "Replacing local UI files in release_v1/ui..."
    cp -a "$ui_build_dir"/. "$LOCAL_UI_DIR"/

    if [[ "$DATABASE_REPLACE" == "true" ]]; then
        require_file "$LOCAL_DB_PATH" "local database"

        echo "Replacing local database files in release_v1/..."
        cp "$LOCAL_DB_PATH" "$LOCAL_RELEASE_DIR/sakkath.db"

        local optional_db
        for optional_db in sakkath.db-shm sakkath.db-wal; do
            if [[ -f "$REPO_ROOT/$optional_db" ]]; then
                cp "$REPO_ROOT/$optional_db" "$LOCAL_RELEASE_DIR/$optional_db"
            else
                rm -f "$LOCAL_RELEASE_DIR/$optional_db"
            fi
        done
    else
        echo "Leaving existing local release_v1 database files unchanged."
    fi
}

ssh_cmd() {
    ssh -i "$SSH_KEY_PATH" -p "$SSH_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$SSH_USER@$SSH_HOST" "$@"
}

scp_file() {
    local source_path="$1"
    local target_path="$2"
    scp -i "$SSH_KEY_PATH" -P "$SSH_PORT" -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$source_path" "$SSH_USER@$SSH_HOST:$target_path"
}

restart_remote_service_on_exit() {
    if [[ "$REMOTE_SERVICE_STOPPED" == "true" ]]; then
        ssh_cmd "sudo systemctl start '$REMOTE_SERVICE_NAME' >/dev/null 2>&1 || true"
    fi
}

upload_release() {
    load_root_env
    trap restart_remote_service_on_exit EXIT

    local remote_tmp_ui_archive="/tmp/sakkath-ui-release.tar.gz"
    local local_ui_archive
    local_ui_archive="$(mktemp)"
    rm -f "$local_ui_archive"
    local_ui_archive="${local_ui_archive}.tar.gz"

    echo "Stopping remote service $REMOTE_SERVICE_NAME..."
    ssh_cmd "sudo systemctl stop '$REMOTE_SERVICE_NAME'"
    REMOTE_SERVICE_STOPPED="true"

    echo "Uploading API binary..."
    scp_file "$LOCAL_API_DIR/sakkath-api" "$REMOTE_RELEASE_DIR/api/sakkath-api"
    ssh_cmd "chmod +x '$REMOTE_RELEASE_DIR/api/sakkath-api'"

    echo "Uploading UI bundle..."
    tar -C "$LOCAL_RELEASE_DIR" -czf "$local_ui_archive" ui
    scp_file "$local_ui_archive" "$remote_tmp_ui_archive"
    ssh_cmd "mkdir -p '$REMOTE_RELEASE_DIR' && rm -rf '$REMOTE_RELEASE_DIR/ui' && tar -xzf '$remote_tmp_ui_archive' -C '$REMOTE_RELEASE_DIR' && rm -f '$remote_tmp_ui_archive'"
    rm -f "$local_ui_archive"

    if [[ "$DATABASE_REPLACE" == "true" ]]; then
        echo "Uploading database files..."
        backup_remote_database
        scp_file "$LOCAL_RELEASE_DIR/sakkath.db" "$REMOTE_RELEASE_DIR/sakkath.db"
        ssh_cmd "rm -f '$REMOTE_RELEASE_DIR/sakkath.db-shm' '$REMOTE_RELEASE_DIR/sakkath.db-wal'"

        local optional_db
        for optional_db in sakkath.db-shm sakkath.db-wal; do
            if [[ -f "$LOCAL_RELEASE_DIR/$optional_db" ]]; then
                scp_file "$LOCAL_RELEASE_DIR/$optional_db" "$REMOTE_RELEASE_DIR/$optional_db"
            fi
        done
    else
        echo "Leaving remote database files unchanged."
    fi

    echo "Starting remote service $REMOTE_SERVICE_NAME..."
    ssh_cmd "sudo systemctl start '$REMOTE_SERVICE_NAME' && sudo systemctl status --no-pager '$REMOTE_SERVICE_NAME'"
    REMOTE_SERVICE_STOPPED="false"
    trap - EXIT
}

main() {
    parse_args "$@"

    echo "database_replace=$DATABASE_REPLACE"
    echo "upload=$UPLOAD"

    build_release_artifacts
    sync_local_release

    if [[ "$UPLOAD" == "true" ]]; then
        upload_release
    else
        echo "Skipping remote upload. Local release updated at $LOCAL_RELEASE_DIR"
    fi

    echo "Release flow completed successfully."
}

main "$@"