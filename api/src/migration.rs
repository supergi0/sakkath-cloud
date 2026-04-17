use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub enum SeedSource {
    MockData,
    TeamsCsv { path: Option<String> },
}

pub fn default_database_path() -> Result<PathBuf, sqlx::Error> {
    let current_dir = std::env::current_dir().map_err(|err| {
        sqlx::Error::Configuration(format!("Failed to read current directory: {err}").into())
    })?;

    if current_dir.file_name().and_then(|name| name.to_str()) == Some("api") {
        if let Some(parent) = current_dir.parent() {
            return Ok(parent.join("sakkath.db"));
        }
    }

    let current_exe = std::env::current_exe().map_err(|err| {
        sqlx::Error::Configuration(format!("Failed to read current executable: {err}").into())
    })?;

    if let Some(exe_dir) = current_exe.parent() {
        if exe_dir.file_name().and_then(|name| name.to_str()) == Some("api") {
            if let Some(parent) = exe_dir.parent() {
                return Ok(parent.join("sakkath.db"));
            }
        }
    }

    Ok(current_dir.join("sakkath.db"))
}

pub fn resolve_database_url() -> Result<String, sqlx::Error> {
    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        return Ok(database_url);
    }

    let database_path = default_database_path()?;
    Ok(format!("sqlite:{}", database_path.display()))
}

pub fn sqlite_path_from_url(database_url: &str) -> Option<PathBuf> {
    database_url.strip_prefix("sqlite:").map(PathBuf::from)
}

pub fn is_sqlite_file_present(database_url: &str) -> bool {
    sqlite_path_from_url(database_url)
        .map(|path| path.exists())
        .unwrap_or(true)
}

async fn enable_sqlite_wal_mode(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode = WAL;")
        .fetch_one(pool)
        .await?;

    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(sqlx::Error::Configuration(
            format!("Failed to enable SQLite WAL mode; SQLite reported journal_mode={journal_mode}").into(),
        ));
    }

    tracing::info!("SQLite WAL mode enabled");

    Ok(())
}

/// Initialize the database connection pool
pub async fn init_db_pool(database_url: &str, create_if_missing: bool) -> Result<SqlitePool, sqlx::Error> {
    tracing::info!("Connecting to database: {}", database_url);
    
    if let Some(path) = database_url.strip_prefix("sqlite:") {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| sqlx::Error::Configuration(
                        format!("Failed to create database directory: {}", e).into()
                    ))?;
            }
        }
        
        if !Path::new(path).exists() {
            if create_if_missing {
                std::fs::File::create(path)
                    .map_err(|e| sqlx::Error::Configuration(
                        format!("Failed to create database file: {}", e).into()
                    ))?;
            } else {
                return Err(sqlx::Error::Configuration(
                    format!(
                        "Database file not found at {}. Run `sakkath-api create-database` first.",
                        path
                    )
                    .into(),
                ));
            }
        }
    }
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    if database_url.starts_with("sqlite:") {
        enable_sqlite_wal_mode(&pool).await?;
    }
    
    Ok(pool)
}

/// Run migrations - creates tables if they don't exist
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Users table: role (0=superadmin, 1=admin, 2=player, 3=POC), team_id links directly to team
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) NOT NULL,
            email VARCHAR(255),
            phone VARCHAR(20),
            dob DATE,
            team_id INTEGER,
            role INTEGER DEFAULT 2,
            password_hash VARCHAR(255),
            is_captain INTEGER DEFAULT 0,
            is_spirit_captain INTEGER DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            deleted_at TIMESTAMP,
            FOREIGN KEY (team_id) REFERENCES teams(id)
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Teams table: division (0=Open, 1=Women)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS teams (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) NOT NULL,
            abbreviation VARCHAR(5),
            admin_id INTEGER,
            division INTEGER DEFAULT 0,
            location VARCHAR(255),
            full_logo TEXT,
            small_logo TEXT,
            init_rank INTEGER,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            deleted_at TIMESTAMP,
            FOREIGN KEY (admin_id) REFERENCES users(id)
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Fields table: hints for helpful info
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fields (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) NOT NULL,
            hints TEXT,
            map_link VARCHAR(500),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Matches table: possession (NULL=not started, 1=t1, 2=t2, >=3=done), type (1-N=swiss round, 1001=playoffs, 1002=finals)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS matches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            t1_id INTEGER NOT NULL,
            t2_id INTEGER NOT NULL,
            field_id INTEGER,
            time TIMESTAMP,
            t1_score INTEGER DEFAULT 0,
            t2_score INTEGER DEFAULT 0,
            t1_spirit INTEGER,
            t2_spirit INTEGER,
            stream_url VARCHAR(500),
            possession INTEGER,
            volunteer_id INTEGER,
            type INTEGER DEFAULT 1,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            deleted_at TIMESTAMP,
            FOREIGN KEY (t1_id) REFERENCES teams(id),
            FOREIGN KEY (t2_id) REFERENCES teams(id),
            FOREIGN KEY (field_id) REFERENCES fields(id),
            FOREIGN KEY (volunteer_id) REFERENCES users(id)
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Match events: event_type (0=goal, 1=assist, 2=block, 3=turnover)
    // team_id for NULL player_id cases (score without identified player)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS match_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            player_id INTEGER,
            team_id INTEGER,
            event_type INTEGER NOT NULL,
            actor_user_id INTEGER,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES matches(id),
            FOREIGN KEY (player_id) REFERENCES users(id),
            FOREIGN KEY (team_id) REFERENCES teams(id),
            FOREIGN KEY (actor_user_id) REFERENCES users(id)
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Announcements: priority (0=high, 1=normal, 2=low)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS announcements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title VARCHAR(255) NOT NULL,
            message TEXT NOT NULL,
            priority INTEGER DEFAULT 1,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Create indexes for query optimization
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_team_id ON users(team_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users(deleted_at)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_teams_division ON teams(division)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_teams_deleted_at ON teams(deleted_at)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_matches_t1_id ON matches(t1_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_matches_t2_id ON matches(t2_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_matches_deleted_at ON matches(deleted_at)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_matches_type ON matches(type)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_player_id ON match_events(player_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_match_id ON match_events(match_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_team_id ON match_events(team_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_actor_user_id ON match_events(actor_user_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_event_type ON match_events(event_type)").execute(pool).await?;

    let teams_has_abbreviation: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM pragma_table_info('teams') WHERE name='abbreviation'"
    ).fetch_optional(pool).await.unwrap_or(None);

    if teams_has_abbreviation.is_none() {
        sqlx::query("ALTER TABLE teams ADD COLUMN abbreviation VARCHAR(5)").execute(pool).await?;
    }

    let users_email_not_null: Option<(i64,)> = sqlx::query_as(
        "SELECT \"notnull\" FROM pragma_table_info('users') WHERE name='email'"
    ).fetch_optional(pool).await.unwrap_or(None);

    if matches!(users_email_not_null, Some((1,))) {
        let mut conn = pool.acquire().await?;
        sqlx::query("PRAGMA foreign_keys=OFF").execute(&mut *conn).await?;
        sqlx::query(
            r#"
            CREATE TABLE users_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255),
                phone VARCHAR(20),
                dob DATE,
                team_id INTEGER,
                role INTEGER DEFAULT 2,
                password_hash VARCHAR(255),
                is_captain INTEGER DEFAULT 0,
                is_spirit_captain INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                deleted_at TIMESTAMP,
                FOREIGN KEY (team_id) REFERENCES teams(id)
            )
            "#
        ).execute(&mut *conn).await?;

        sqlx::query(
            r#"INSERT INTO users_new (id, name, email, phone, dob, team_id, role, password_hash, is_captain, is_spirit_captain, created_at, updated_at, deleted_at)
                SELECT id, name, email, phone, dob, team_id, role, password_hash, is_captain, is_spirit_captain, created_at, updated_at, deleted_at FROM users"#
        ).execute(&mut *conn).await?;

        sqlx::query("DROP TABLE users").execute(&mut *conn).await?;
        sqlx::query("ALTER TABLE users_new RENAME TO users").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_team_id ON users(team_id)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)").execute(&mut *conn).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users(deleted_at)").execute(&mut *conn).await?;
        sqlx::query("PRAGMA foreign_keys=ON").execute(&mut *conn).await?;
    }
    
    // Migrate existing match_events table if needed (add team_id column)
    let has_team_id: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('match_events') WHERE name='team_id'"
    ).fetch_one(pool).await.unwrap_or((0,));
    
    if has_team_id.0 == 0 {
        // Table needs migration - recreate with new schema
        sqlx::query("ALTER TABLE match_events RENAME TO match_events_old").execute(pool).await?;
        
        sqlx::query(
            r#"CREATE TABLE match_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                match_id INTEGER NOT NULL,
                player_id INTEGER,
                team_id INTEGER,
                event_type INTEGER NOT NULL,
                actor_user_id INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (match_id) REFERENCES matches(id),
                FOREIGN KEY (player_id) REFERENCES users(id),
                FOREIGN KEY (team_id) REFERENCES teams(id),
                FOREIGN KEY (actor_user_id) REFERENCES users(id)
            )"#
        ).execute(pool).await?;
        
        // Copy old data (derive team_id from player_id)
        sqlx::query(
                r#"INSERT INTO match_events (id, match_id, player_id, team_id, event_type, actor_user_id, created_at)
                    SELECT me.id, me.match_id, me.player_id, u.team_id, me.event_type, NULL, me.created_at
               FROM match_events_old me
               LEFT JOIN users u ON u.id = me.player_id"#
        ).execute(pool).await?;
        
        sqlx::query("DROP TABLE match_events_old").execute(pool).await?;
        
        // Recreate indexes
        sqlx::query("CREATE INDEX idx_match_events_player_id ON match_events(player_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_match_id ON match_events(match_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_team_id ON match_events(team_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_actor_user_id ON match_events(actor_user_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_event_type ON match_events(event_type)").execute(pool).await?;
    }

    let has_actor_user_id: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('match_events') WHERE name='actor_user_id'"
    ).fetch_one(pool).await.unwrap_or((0,));

    if has_actor_user_id.0 == 0 {
        sqlx::query("ALTER TABLE match_events ADD COLUMN actor_user_id INTEGER")
            .execute(pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_actor_user_id ON match_events(actor_user_id)")
            .execute(pool)
            .await?;
    }
    
    // Migrate matches table: add type column if not exists
    let has_type: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('matches') WHERE name='type'"
    ).fetch_one(pool).await.unwrap_or((0,));
    
    if has_type.0 == 0 {
        sqlx::query("ALTER TABLE matches ADD COLUMN type INTEGER DEFAULT 1").execute(pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_matches_type ON matches(type)").execute(pool).await?;
    }
    
    // Spirit scores table: WFDF 5-category spirit per team per match
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS spirit_scores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            rules_knowledge INTEGER NOT NULL DEFAULT 2,
            fouls_contact INTEGER NOT NULL DEFAULT 2,
            fair_mindedness INTEGER NOT NULL DEFAULT 2,
            positive_attitude INTEGER NOT NULL DEFAULT 2,
            communication INTEGER NOT NULL DEFAULT 2,
            total INTEGER NOT NULL DEFAULT 10,
            mvp_player_id INTEGER,
            msp_player_id INTEGER,
            submitted_by_team_id INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES matches(id),
            FOREIGN KEY (team_id) REFERENCES teams(id),
            FOREIGN KEY (mvp_player_id) REFERENCES users(id),
            FOREIGN KEY (msp_player_id) REFERENCES users(id),
            FOREIGN KEY (submitted_by_team_id) REFERENCES teams(id),
            UNIQUE(match_id, team_id, submitted_by_team_id)
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spirit_scores_match_id ON spirit_scores(match_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_spirit_scores_team_id ON spirit_scores(team_id)").execute(pool).await?;

    // Score confirmations table: teams confirm the final score
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS score_confirmations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            team_id INTEGER NOT NULL,
            t1_score INTEGER NOT NULL,
            t2_score INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES matches(id),
            FOREIGN KEY (team_id) REFERENCES teams(id),
            UNIQUE(match_id, team_id)
        )
        "#
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_score_confirmations_match_id ON score_confirmations(match_id)").execute(pool).await?;

    Ok(())
}

pub async fn seed_database(pool: &SqlitePool, source: SeedSource) -> Result<(), sqlx::Error> {
    let team_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM teams")
        .fetch_one(pool)
        .await?;
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    let field_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM fields")
        .fetch_one(pool)
        .await?;
    let match_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM matches")
        .fetch_one(pool)
        .await?;
    let announcement_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM announcements")
        .fetch_one(pool)
        .await?;

    if team_count.0 > 0 || user_count.0 > 0 || field_count.0 > 0 || match_count.0 > 0 || announcement_count.0 > 0 {
        return Err(sqlx::Error::Configuration(
            "Database already has data. Refusing to seed a non-empty database.".into(),
        ));
    }

    match source {
        SeedSource::MockData => crate::seeder::populate_mock_data(pool).await,
        SeedSource::TeamsCsv { path } => crate::helpers::csv_seed::populate_from_teams_csv(pool, path.as_deref()).await,
    }
}

/// Check if migrations were successful
pub async fn verify_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let tables = ["users", "teams", "fields", "matches", "match_events", "announcements", "spirit_scores", "score_confirmations"];
    
    for table in tables {
        let count: (i64,) = sqlx::query_as(
            &format!("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'", table)
        )
        .fetch_one(pool)
        .await?;
        
        if count.0 == 0 {
            return Err(sqlx::Error::Configuration(format!("Table {} not found", table).into()));
        }
    }
    
    Ok(())
}
