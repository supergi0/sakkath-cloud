#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"
LOCAL_DB_PATH="$REPO_ROOT/sakkath.db"
ROOT_ENV_FILE="$REPO_ROOT/.env"
SOURCE_API_ENV_FILE="$REPO_ROOT/api/.env"

DATABASE_REPLACE="false"
UPLOAD="false"
PROD="false"

RELEASE_LABEL=""
RELEASE_PORT=""
RELEASE_REDIS_NAMESPACE=""
LOCAL_RELEASE_DIR=""
LOCAL_API_DIR=""
LOCAL_UI_DIR=""

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
Usage: ./release.sh [--prod true|false] [--database_replace true|false] [--upload true|false]

Flags:
  --prod true|false               Target prod when true, otherwise staging. Default: false
  --database_replace true|false   Replace the target release database files from the local repo root database. Default: false
  --upload true|false             Upload the built release to the remote server using root .env credentials. Default: false
  --help                          Show this help message.
EOF
}

configure_release_mode() {
    if [[ "$PROD" == "true" ]]; then
        RELEASE_LABEL="prod"
        RELEASE_PORT="9000"
        RELEASE_REDIS_NAMESPACE="sakkath:prod"
        LOCAL_RELEASE_DIR="$REPO_ROOT/release_v1"
    else
        RELEASE_LABEL="staging"
        RELEASE_PORT="9001"
        RELEASE_REDIS_NAMESPACE="sakkath:staging"
        LOCAL_RELEASE_DIR="$REPO_ROOT/release_staging_v1"
    fi

    LOCAL_API_DIR="$LOCAL_RELEASE_DIR/api"
    LOCAL_UI_DIR="$LOCAL_RELEASE_DIR/ui"
}

derive_staging_release_dir() {
    local prod_release_dir="$1"

    if [[ -z "$prod_release_dir" ]]; then
        return
    fi

    printf '%s/release_staging_v1\n' "$(dirname "$prod_release_dir")"
}

derive_staging_service_name() {
    local prod_service_name="$1"

    if [[ -z "$prod_service_name" ]]; then
        printf 'sakkath-staging.service\n'
        return
    fi

    if [[ "$prod_service_name" == *.service ]]; then
        printf '%s-staging.service\n' "${prod_service_name%.service}"
    else
        printf '%s-staging\n' "$prod_service_name"
    fi
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
            --prod)
                [[ $# -ge 2 ]] || { echo "Missing value for --prod" >&2; usage; exit 1; }
                PROD="$(normalize_bool "$2")"
                shift 2
                ;;
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

    local prod_remote_release_dir="${DEPLOY_REMOTE_RELEASE_DIR_PROD:-${DEPLOY_REMOTE_RELEASE_DIR:-}}"
    local prod_remote_service_name="${DEPLOY_REMOTE_SERVICE_NAME_PROD:-${DEPLOY_REMOTE_SERVICE_NAME:-sakkath.service}}"

    if [[ "$PROD" == "true" ]]; then
        REMOTE_RELEASE_DIR="$prod_remote_release_dir"
        REMOTE_SERVICE_NAME="$prod_remote_service_name"
    else
        REMOTE_RELEASE_DIR="${DEPLOY_REMOTE_RELEASE_DIR_STAGING:-$(derive_staging_release_dir "$prod_remote_release_dir")}"
        REMOTE_SERVICE_NAME="${DEPLOY_REMOTE_SERVICE_NAME_STAGING:-$(derive_staging_service_name "$prod_remote_service_name")}"
    fi

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

upsert_env_assignment() {
    local file_path="$1"
    local key="$2"
    local assignment="$3"

    if grep -Eq "^${key}=" "$file_path"; then
        sed -i "s|^${key}=.*$|$assignment|" "$file_path"
    else
        printf '%s\n' "$assignment" >> "$file_path"
    fi
}

sync_release_env() {
    local release_env_path="$LOCAL_API_DIR/.env"

    if [[ ! -f "$release_env_path" ]]; then
        require_file "$SOURCE_API_ENV_FILE" "API env template"
        cp "$SOURCE_API_ENV_FILE" "$release_env_path"
    fi

    upsert_env_assignment "$release_env_path" "release" "release=true"
    upsert_env_assignment "$release_env_path" "PORT" "PORT=$RELEASE_PORT"
    upsert_env_assignment "$release_env_path" "REDIS_NAMESPACE" "REDIS_NAMESPACE=\"$RELEASE_REDIS_NAMESPACE\""
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

    echo "Updating local API binary in $LOCAL_API_DIR..."
    cp "$api_binary" "$LOCAL_API_DIR/sakkath-api"
    chmod +x "$LOCAL_API_DIR/sakkath-api"

    sync_release_env

    echo "Replacing local UI files in $LOCAL_UI_DIR..."
    cp -a "$ui_build_dir"/. "$LOCAL_UI_DIR"/

    if [[ "$DATABASE_REPLACE" == "true" ]]; then
        require_file "$LOCAL_DB_PATH" "local database"

        echo "Replacing local database files in $LOCAL_RELEASE_DIR/..."
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
        echo "Leaving existing local $LOCAL_RELEASE_DIR database files unchanged."
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

    echo "Ensuring remote release directories exist at $REMOTE_RELEASE_DIR..."
    ssh_cmd "mkdir -p '$REMOTE_RELEASE_DIR/api'"

    echo "Stopping remote service $REMOTE_SERVICE_NAME..."
    ssh_cmd "sudo systemctl stop '$REMOTE_SERVICE_NAME'"
    REMOTE_SERVICE_STOPPED="true"

    echo "Uploading API binary..."
    scp_file "$LOCAL_API_DIR/sakkath-api" "$REMOTE_RELEASE_DIR/api/sakkath-api"
    ssh_cmd "chmod +x '$REMOTE_RELEASE_DIR/api/sakkath-api'"

    echo "Uploading API env..."
    scp_file "$LOCAL_API_DIR/.env" "$REMOTE_RELEASE_DIR/api/.env"

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
    configure_release_mode

    echo "prod=$PROD"
    echo "target=$RELEASE_LABEL"
    echo "database_replace=$DATABASE_REPLACE"
    echo "upload=$UPLOAD"
    echo "local_release_dir=$LOCAL_RELEASE_DIR"
    echo "port=$RELEASE_PORT"
    echo "redis_namespace=$RELEASE_REDIS_NAMESPACE"

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