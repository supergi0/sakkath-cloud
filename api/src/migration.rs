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
    let pw_hash = "fc5e038d38a57032085441e7fe7010b0"; // MD5 of "helloworld"
    
    // Insert Open Division teams (20 teams, IDs 1-20)
    sqlx::query(
        r#"INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Bolts', NULL, 0, 'Bangalore', 1),
        ('Chennai Challengers', NULL, 0, 'Chennai', 2),
        ('Mumbai Mavericks', NULL, 0, 'Mumbai', 3),
        ('Delhi Dragons', NULL, 0, 'Delhi', 4),
        ('Hyderabad Hawks', NULL, 0, 'Hyderabad', 5),
        ('Kolkata Knights', NULL, 0, 'Kolkata', 6),
        ('Pune Panthers', NULL, 0, 'Pune', 7),
        ('Ahmedabad Aces', NULL, 0, 'Ahmedabad', 8),
        ('Jaipur Jaguars', NULL, 0, 'Jaipur', 9),
        ('Lucknow Lions', NULL, 0, 'Lucknow', 10),
        ('Kochi Kings', NULL, 0, 'Kochi', 11),
        ('Goa Gladiators', NULL, 0, 'Goa', 12),
        ('Chandigarh Chargers', NULL, 0, 'Chandigarh', 13),
        ('Indore Infernos', NULL, 0, 'Indore', 14),
        ('Nagpur Ninjas', NULL, 0, 'Nagpur', 15),
        ('Vizag Vikings', NULL, 0, 'Vizag', 16),
        ('Coimbatore Cosmos', NULL, 0, 'Coimbatore', 17),
        ('Mysore Mambas', NULL, 0, 'Mysore', 18),
        ('Surat Strikers', NULL, 0, 'Surat', 19),
        ('Bhopal Blazers', NULL, 0, 'Bhopal', 20)"#
    ).execute(pool).await?;
    
    // Insert Women Division teams (12 teams, IDs 21-32)
    sqlx::query(
        r#"INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Blaze', NULL, 1, 'Bangalore', 1),
        ('Chennai Chargers', NULL, 1, 'Chennai', 2),
        ('Mumbai Meteors', NULL, 1, 'Mumbai', 3),
        ('Delhi Divas', NULL, 1, 'Delhi', 4),
        ('Hyderabad Hurricanes', NULL, 1, 'Hyderabad', 5),
        ('Kolkata Queens', NULL, 1, 'Kolkata', 6),
        ('Pune Pythons', NULL, 1, 'Pune', 7),
        ('Ahmedabad Angels', NULL, 1, 'Ahmedabad', 8),
        ('Jaipur Jewels', NULL, 1, 'Jaipur', 9),
        ('Lucknow Legends', NULL, 1, 'Lucknow', 10),
        ('Kochi Kites', NULL, 1, 'Kochi', 11),
        ('Goa Gazelles', NULL, 1, 'Goa', 12)"#
    ).execute(pool).await?;
    
    // Insert 2 players per team (IDs 1-64)
    let mut player_sql = String::from("INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES ");
    let mut values = Vec::new();
    let open_names = ["Raj", "Amit", "Vikram", "Rohan", "Arjun", "Sanjay", "Karthik", "Nikhil", "Aditya", "Pranav", "Rahul", "Vivek", "Suresh", "Ganesh", "Mohan", "Ravi", "Ajay", "Vijay", "Krishna", "Venkat"];
    let women_names = ["Priya", "Sneha", "Ananya", "Kavya", "Divya", "Meera", "Riya", "Neha", "Pooja", "Shruti", "Swati", "Nisha"];
    
    let mut player_id = 1;
    for team_id in 1..=20 {
        let name1 = format!("{} Kumar", open_names[(team_id - 1) as usize]);
        let name2 = format!("{} Patel", open_names[(team_id - 1) as usize]);
        values.push(format!("('{}', 'player{}@example.com', '+91987654{:04}', '1995-03-15', {}, 2, '{}')", name1, player_id, player_id, team_id, pw_hash));
        player_id += 1;
        values.push(format!("('{}', 'player{}@example.com', '+91987654{:04}', '1993-07-22', {}, 2, '{}')", name2, player_id, player_id, team_id, pw_hash));
        player_id += 1;
    }
    for team_id in 21..=32 {
        let idx = (team_id - 21) as usize;
        let name1 = format!("{} Sharma", women_names[idx]);
        let name2 = format!("{} Reddy", women_names[idx]);
        values.push(format!("('{}', 'player{}@example.com', '+91987654{:04}', '1995-03-15', {}, 2, '{}')", name1, player_id, player_id, team_id, pw_hash));
        player_id += 1;
        values.push(format!("('{}', 'player{}@example.com', '+91987654{:04}', '1993-07-22', {}, 2, '{}')", name2, player_id, player_id, team_id, pw_hash));
        player_id += 1;
    }
    player_sql.push_str(&values.join(","));
    sqlx::query(&player_sql).execute(pool).await?;
    
    // Insert admin/POC users
    sqlx::query(
        r#"INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, ?),
        ('Admin One', 'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, ?),
        ('Admin Two', 'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, ?)"#
    ).bind(pw_hash).bind(pw_hash).bind(pw_hash).execute(pool).await?;
    
    // Insert POCs for all teams
    let mut poc_sql = String::from("INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES ");
    let mut poc_values = Vec::new();
    for team_id in 1..=32 {
        poc_values.push(format!("('POC Team {}', 'poc{}@sakkath.com', '+91900000{:04}', '1992-03-10', {}, 3, '{}')", team_id, team_id, team_id, team_id, pw_hash));
    }
    poc_sql.push_str(&poc_values.join(","));
    sqlx::query(&poc_sql).execute(pool).await?;
    
    // Insert fields
    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Field 1', 'Main astroturf ground. Cleats preferred.', 'https://maps.google.com/?q=field1'),
        ('Field 2', 'Secondary astroturf. Cleats recommended.', 'https://maps.google.com/?q=field2'),
        ('Field 3', 'Natural grass. Bring water.', 'https://maps.google.com/?q=field3'),
        ('Field 4', 'Open grass field. Windy afternoons.', 'https://maps.google.com/?q=field4'),
        ('Field 5', 'Practice field. Flat surface.', 'https://maps.google.com/?q=field5')"#
    ).execute(pool).await?;
    
    // Open Division: R1 complete, R2 has 9 done + 1 live (for testing updates)
    // Swiss Round 1 (type=1): Initial seeding 1v11, 2v12, etc. (top half vs bottom half)
    sqlx::query(
        r#"INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES
        (1, 11, 1, '2026-01-18 09:00:00', 15, 9, 14, 13, 3, 1),
        (2, 12, 2, '2026-01-18 09:00:00', 15, 10, 13, 14, 3, 1),
        (3, 13, 3, '2026-01-18 09:00:00', 14, 11, 14, 12, 3, 1),
        (4, 14, 4, '2026-01-18 09:00:00', 15, 8, 13, 13, 3, 1),
        (5, 15, 5, '2026-01-18 09:00:00', 13, 11, 15, 14, 3, 1),
        (6, 16, 1, '2026-01-18 11:00:00', 14, 10, 14, 13, 3, 1),
        (7, 17, 2, '2026-01-18 11:00:00', 15, 12, 13, 14, 3, 1),
        (8, 18, 3, '2026-01-18 11:00:00', 13, 9, 14, 15, 3, 1),
        (9, 19, 4, '2026-01-18 11:00:00', 15, 11, 12, 13, 3, 1),
        (10, 20, 5, '2026-01-18 11:00:00', 14, 10, 13, 14, 3, 1)"#
    ).execute(pool).await?;
    
    // Swiss Round 2 (type=2): 9 completed, 1 live (15v20 at field 5)
    sqlx::query(
        r#"INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES
        (1, 6, 1, '2026-01-18 14:00:00', 15, 13, 14, 14, 3, 2),
        (2, 7, 2, '2026-01-18 14:00:00', 12, 15, 13, 13, 3, 2),
        (3, 8, 3, '2026-01-18 14:00:00', 15, 11, 14, 14, 3, 2),
        (4, 9, 4, '2026-01-18 14:00:00', 13, 15, 14, 13, 3, 2),
        (5, 10, 5, '2026-01-18 14:00:00', 15, 14, 15, 14, 3, 2),
        (11, 16, 1, '2026-01-18 16:00:00', 12, 15, 13, 14, 3, 2),
        (12, 17, 2, '2026-01-18 16:00:00', 15, 13, 14, 13, 3, 2),
        (13, 18, 3, '2026-01-18 16:00:00', 14, 15, 13, 14, 3, 2),
        (14, 19, 4, '2026-01-18 16:00:00', 15, 12, 14, 13, 3, 2),
        (15, 20, 5, '2026-01-18 16:00:00', 7, 6, NULL, NULL, 1, 2)"#
    ).execute(pool).await?;
    
    // Women Division: No matches yet - will be auto-generated on startup
    
    // Match events for some Open matches
    sqlx::query(
        r#"INSERT INTO match_events (match_id, player_id, event_type, created_at) VALUES
        (1, 1, 0, '2026-01-18 09:05:00'), (1, 2, 1, '2026-01-18 09:05:00'),
        (1, 1, 0, '2026-01-18 09:15:00'), (1, 2, 1, '2026-01-18 09:15:00'),
        (1, 1, 2, '2026-01-18 09:18:00'),
        (2, 3, 0, '2026-01-18 09:05:00'), (2, 4, 1, '2026-01-18 09:05:00'),
        (2, 3, 0, '2026-01-18 09:10:00'), (2, 4, 1, '2026-01-18 09:10:00'),
        (11, 1, 0, '2026-01-18 14:05:00'), (11, 2, 1, '2026-01-18 14:05:00'),
        (11, 1, 0, '2026-01-18 14:15:00')"#
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
