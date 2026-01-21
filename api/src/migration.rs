use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::time::Duration;
use std::path::Path;

/// Initialize the database connection pool
pub async fn init_db_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
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
            std::fs::File::create(path)
                .map_err(|e| sqlx::Error::Configuration(
                    format!("Failed to create database file: {}", e).into()
                ))?;
        }
    }
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;
    
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
            email VARCHAR(255) NOT NULL,
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
    
    // Matches table: possession (NULL=not started, 1=t1, 2=t2, >=3=done)
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
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES matches(id),
            FOREIGN KEY (player_id) REFERENCES users(id),
            FOREIGN KEY (team_id) REFERENCES teams(id)
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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_player_id ON match_events(player_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_match_id ON match_events(match_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_team_id ON match_events(team_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_event_type ON match_events(event_type)").execute(pool).await?;
    
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
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (match_id) REFERENCES matches(id),
                FOREIGN KEY (player_id) REFERENCES users(id),
                FOREIGN KEY (team_id) REFERENCES teams(id)
            )"#
        ).execute(pool).await?;
        
        // Copy old data (derive team_id from player_id)
        sqlx::query(
            r#"INSERT INTO match_events (id, match_id, player_id, team_id, event_type, created_at)
               SELECT me.id, me.match_id, me.player_id, u.team_id, me.event_type, me.created_at
               FROM match_events_old me
               LEFT JOIN users u ON u.id = me.player_id"#
        ).execute(pool).await?;
        
        sqlx::query("DROP TABLE match_events_old").execute(pool).await?;
        
        // Recreate indexes
        sqlx::query("CREATE INDEX idx_match_events_player_id ON match_events(player_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_match_id ON match_events(match_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_team_id ON match_events(team_id)").execute(pool).await?;
        sqlx::query("CREATE INDEX idx_match_events_event_type ON match_events(event_type)").execute(pool).await?;
    }
    
    // Populate mock data
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    
    if user_count.0 == 0 {
        populate_mock_data(pool).await?;
    }
    
    Ok(())
}

async fn populate_mock_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // MD5 hash of "helloworld" = fc5e038d38a57032085441e7fe7010b0
    let pw_hash = "fc5e038d38a57032085441e7fe7010b0";
    
    // Insert Open Division teams (6 teams, IDs 1-6)
    sqlx::query(
        r#"INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Bolts', NULL, 0, 'Bangalore', 3),
        ('Chennai Challengers', NULL, 0, 'Chennai', 5),
        ('Mumbai Mavericks', NULL, 0, 'Mumbai', 2),
        ('Delhi Dragons', NULL, 0, 'Delhi', 4),
        ('Hyderabad Hawks', NULL, 0, 'Hyderabad', 6),
        ('Kolkata Knights', NULL, 0, 'Kolkata', 1)"#
    ).execute(pool).await?;
    
    // Insert Women Division teams (4 teams, IDs 7-10)
    sqlx::query(
        r#"INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Blaze', NULL, 1, 'Bangalore', 2),
        ('Chennai Chargers', NULL, 1, 'Chennai', 3),
        ('Mumbai Meteors', NULL, 1, 'Mumbai', 4),
        ('Delhi Divas', NULL, 1, 'Delhi', 1)"#
    ).execute(pool).await?;
    
    // Insert players (IDs 1-20, 2 per team)
    sqlx::query(
        r#"INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Raj Kumar', 'raj@example.com', '+919876543210', '1995-03-15', 1, 2, ?),
        ('Amit Patel', 'amit@example.com', '+919876543211', '1993-07-22', 1, 2, ?),
        ('Vikram Singh', 'vikram@example.com', '+919876543212', '1990-11-08', 2, 2, ?),
        ('Rohan Desai', 'rohan@example.com', '+919876543213', '1996-05-12', 2, 2, ?),
        ('Arjun Menon', 'arjun@example.com', '+919876543214', '1992-09-30', 3, 2, ?),
        ('Sanjay Gupta', 'sanjay@example.com', '+919876543215', '1994-06-18', 3, 2, ?),
        ('Karthik Rao', 'karthik@example.com', '+919876543216', '1991-12-25', 4, 2, ?),
        ('Nikhil Verma', 'nikhil@example.com', '+919876543217', '1995-08-14', 4, 2, ?),
        ('Aditya Sharma', 'aditya@example.com', '+919876543218', '1993-04-20', 5, 2, ?),
        ('Pranav Nair', 'pranav@example.com', '+919876543219', '1996-10-05', 5, 2, ?),
        ('Rahul Joshi', 'rahul@example.com', '+919876543220', '1994-02-28', 6, 2, ?),
        ('Vivek Kumar', 'vivek@example.com', '+919876543221', '1992-08-15', 6, 2, ?),
        ('Priya Sharma', 'priya@example.com', '+919876543230', '1995-03-15', 7, 2, ?),
        ('Sneha Reddy', 'sneha@example.com', '+919876543231', '1993-07-22', 7, 2, ?),
        ('Ananya Iyer', 'ananya@example.com', '+919876543232', '1990-11-08', 8, 2, ?),
        ('Kavya Nair', 'kavya@example.com', '+919876543233', '1996-05-12', 8, 2, ?),
        ('Divya Krishna', 'divya@example.com', '+919876543234', '1992-09-30', 9, 2, ?),
        ('Meera Patel', 'meera@example.com', '+919876543235', '1994-06-18', 9, 2, ?),
        ('Riya Singh', 'riya@example.com', '+919876543236', '1991-12-25', 10, 2, ?),
        ('Neha Gupta', 'neha@example.com', '+919876543237', '1995-08-14', 10, 2, ?)"#
    )
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .execute(pool).await?;
    
    // Insert admin/POC users (IDs 21-33)
    sqlx::query(
        r#"INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, ?),
        ('Admin One', 'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, ?),
        ('Admin Two', 'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, ?),
        ('POC Bolts', 'poc1@sakkath.com', '+919000000101', '1992-03-10', 1, 3, ?),
        ('POC Challengers', 'poc2@sakkath.com', '+919000000102', '1993-04-11', 2, 3, ?),
        ('POC Mavericks', 'poc3@sakkath.com', '+919000000103', '1991-05-12', 3, 3, ?),
        ('POC Dragons', 'poc4@sakkath.com', '+919000000104', '1990-06-13', 4, 3, ?),
        ('POC Hawks', 'poc5@sakkath.com', '+919000000105', '1994-07-14', 5, 3, ?),
        ('POC Knights', 'poc6@sakkath.com', '+919000000106', '1989-08-15', 6, 3, ?),
        ('POC Blaze', 'poc7@sakkath.com', '+919000000107', '1995-09-16', 7, 3, ?),
        ('POC Chargers', 'poc8@sakkath.com', '+919000000108', '1988-10-17', 8, 3, ?),
        ('POC Meteors', 'poc9@sakkath.com', '+919000000109', '1992-11-18', 9, 3, ?),
        ('POC Divas', 'poc10@sakkath.com', '+919000000110', '1991-12-19', 10, 3, ?)"#
    )
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .bind(pw_hash).bind(pw_hash).bind(pw_hash)
    .execute(pool).await?;
    
    // Insert fields
    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Field 1', 'Main astroturf ground. Cleats preferred.', 'https://maps.google.com/?q=field1'),
        ('Field 2', 'Secondary astroturf. Cleats recommended.', 'https://maps.google.com/?q=field2'),
        ('Field 3', 'Natural grass. Bring water.', 'https://maps.google.com/?q=field3'),
        ('Field 4', 'Open grass field. Windy afternoons.', 'https://maps.google.com/?q=field4')"#
    ).execute(pool).await?;
    
    // Matches: possession NULL=upcoming, 1/2=live (t1/t2 has disc), >=3=ended
    // ENDED matches (with spirit scores): IDs 1-4
    // LIVE matches (no spirit scores, possession 1 or 2): IDs 5-6
    // UPCOMING matches (no spirit scores, possession NULL, scores 0-0): IDs 7-10
    sqlx::query(
        r#"INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, stream_url) VALUES
        (1, 2, 1, '2026-01-18 09:00:00', 15, 12, 14, 13, 3, 'https://youtube.com/watch?v=match1'),
        (3, 4, 2, '2026-01-18 09:00:00', 13, 15, 12, 15, 3, 'https://youtube.com/watch?v=match2'),
        (5, 6, 3, '2026-01-18 11:00:00', 11, 9, 13, 14, 3, NULL),
        (1, 3, 1, '2026-01-18 14:00:00', 14, 11, 13, 14, 3, NULL),
        (1, 4, 1, '2026-01-20 10:00:00', 7, 5, NULL, NULL, 1, 'https://youtube.com/live/live1'),
        (2, 3, 2, '2026-01-20 10:00:00', 4, 6, NULL, NULL, 2, 'https://youtube.com/live/live2'),
        (5, 1, 3, '2026-01-21 09:00:00', 0, 0, NULL, NULL, NULL, NULL),
        (6, 2, 4, '2026-01-21 09:00:00', 0, 0, NULL, NULL, NULL, NULL),
        (4, 5, 1, '2026-01-21 14:00:00', 0, 0, NULL, NULL, NULL, NULL),
        (3, 6, 2, '2026-01-22 09:00:00', 0, 0, NULL, NULL, NULL, NULL)"#
    ).execute(pool).await?;
    
    // Match events for ended matches (1-4) - timeline data for team 1 (Bangalore Bolts)
    // Match 1: Bolts vs Challengers (15-12) - Bolts win
    // Match 4: Bolts vs Mavericks (14-11) - Bolts win
    sqlx::query(
        r#"INSERT INTO match_events (match_id, player_id, event_type, created_at) VALUES
        (1, 1, 0, '2026-01-18 09:05:00'), (1, 2, 1, '2026-01-18 09:05:00'),
        (1, 3, 0, '2026-01-18 09:10:00'), (1, 4, 1, '2026-01-18 09:10:00'),
        (1, 1, 0, '2026-01-18 09:15:00'), (1, 2, 1, '2026-01-18 09:15:00'),
        (1, 1, 2, '2026-01-18 09:18:00'),
        (1, 3, 0, '2026-01-18 09:20:00'),
        (1, 1, 0, '2026-01-18 09:25:00'), (1, 2, 1, '2026-01-18 09:25:00'),
        (1, 1, 0, '2026-01-18 09:30:00'),
        (1, 4, 3, '2026-01-18 09:32:00'),
        (1, 1, 0, '2026-01-18 09:35:00'), (1, 2, 1, '2026-01-18 09:35:00'),
        (2, 5, 0, '2026-01-18 09:05:00'), (2, 6, 1, '2026-01-18 09:05:00'),
        (2, 7, 0, '2026-01-18 09:10:00'), (2, 8, 1, '2026-01-18 09:10:00'),
        (2, 5, 0, '2026-01-18 09:15:00'), (2, 6, 2, '2026-01-18 09:18:00'),
        (2, 7, 0, '2026-01-18 09:20:00'), (2, 7, 0, '2026-01-18 09:25:00'),
        (3, 9, 0, '2026-01-18 11:05:00'), (3, 10, 1, '2026-01-18 11:05:00'),
        (3, 11, 0, '2026-01-18 11:10:00'), (3, 12, 1, '2026-01-18 11:10:00'),
        (3, 9, 0, '2026-01-18 11:15:00'), (3, 9, 2, '2026-01-18 11:18:00'),
        (4, 1, 0, '2026-01-18 14:05:00'), (4, 2, 1, '2026-01-18 14:05:00'),
        (4, 1, 0, '2026-01-18 14:10:00'), (4, 2, 1, '2026-01-18 14:10:00'),
        (4, 5, 0, '2026-01-18 14:15:00'), (4, 6, 1, '2026-01-18 14:15:00'),
        (4, 1, 0, '2026-01-18 14:20:00'), (4, 1, 2, '2026-01-18 14:22:00'),
        (4, 1, 0, '2026-01-18 14:25:00'), (4, 2, 1, '2026-01-18 14:25:00'),
        (4, 5, 0, '2026-01-18 14:30:00'),
        (5, 1, 0, '2026-01-20 10:05:00'), (5, 2, 1, '2026-01-20 10:05:00'),
        (5, 7, 0, '2026-01-20 10:08:00'),
        (5, 1, 0, '2026-01-20 10:12:00'),
        (6, 3, 0, '2026-01-20 10:05:00'), (6, 4, 1, '2026-01-20 10:05:00'),
        (6, 5, 0, '2026-01-20 10:10:00'), (6, 6, 1, '2026-01-20 10:10:00')"#
    ).execute(pool).await?;
    
    // Insert announcements
    sqlx::query(
        r#"INSERT INTO announcements (title, message, priority, expires_at) VALUES
        ('Welcome to Sakkath 2026!', 'Tournament runs Jan 18-22. Check schedule for matches.', 0, '2026-01-25 23:59:59'),
        ('Spirit Scoring Reminder', 'Submit spirit scores within 30 minutes after each match.', 1, '2026-01-25 23:59:59')"#
    ).execute(pool).await?;
    
    Ok(())
}

/// Check if migrations were successful
pub async fn verify_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let tables = ["users", "teams", "fields", "matches", "match_events", "announcements"];
    
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
