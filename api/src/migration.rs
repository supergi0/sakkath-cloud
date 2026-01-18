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
    
    // Matches table: possession (NULL=not started, 1=t1, 2=t2)
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
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            deleted_at TIMESTAMP,
            FOREIGN KEY (t1_id) REFERENCES teams(id),
            FOREIGN KEY (t2_id) REFERENCES teams(id),
            FOREIGN KEY (field_id) REFERENCES fields(id)
        )
        "#
    )
    .execute(pool)
    .await?;
    
    // Match events: event_type (0=goal, 1=assist, 2=block, 3=turnover)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS match_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            player_id INTEGER NOT NULL,
            event_type INTEGER NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (match_id) REFERENCES matches(id),
            FOREIGN KEY (player_id) REFERENCES users(id)
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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_match_events_event_type ON match_events(event_type)").execute(pool).await?;
    
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
    // Insert Open Division teams first (6 teams)
    sqlx::query(
        r#"
        INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Bolts', NULL, 0, 'Bangalore', 3),
        ('Chennai Challengers', NULL, 0, 'Chennai', 5),
        ('Mumbai Mavericks', NULL, 0, 'Mumbai', 2),
        ('Delhi Dragons', NULL, 0, 'Delhi', 4),
        ('Hyderabad Hawks', NULL, 0, 'Hyderabad', 6),
        ('Kolkata Knights', NULL, 0, 'Kolkata', 1)
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert Women Division teams (4 teams)
    sqlx::query(
        r#"
        INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES
        ('Bangalore Blaze', NULL, 1, 'Bangalore', 2),
        ('Chennai Chargers', NULL, 1, 'Chennai', 3),
        ('Mumbai Meteors', NULL, 1, 'Mumbai', 4),
        ('Delhi Divas', NULL, 1, 'Delhi', 1)
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert mock users with team_id
    sqlx::query(
        r#"
        INSERT INTO users (name, email, phone, dob, team_id, role) VALUES
        ('Raj Kumar', 'raj.kumar@example.com', '+919876543210', '1995-03-15', 1, 2),
        ('Amit Patel', 'amit.patel@example.com', '+919876543211', '1993-07-22', 1, 2),
        ('Vikram Singh', 'vikram.singh@example.com', '+919876543212', '1990-11-08', 2, 2),
        ('Rohan Desai', 'rohan.desai@example.com', '+919876543213', '1996-05-12', 2, 2),
        ('Arjun Menon', 'arjun.menon@example.com', '+919876543214', '1992-09-30', 3, 2),
        ('Sanjay Gupta', 'sanjay.gupta@example.com', '+919876543215', '1994-06-18', 3, 2),
        ('Karthik Rao', 'karthik.rao@example.com', '+919876543216', '1991-12-25', 4, 2),
        ('Nikhil Verma', 'nikhil.verma@example.com', '+919876543217', '1995-08-14', 4, 2),
        ('Aditya Sharma', 'aditya.sharma@example.com', '+919876543218', '1993-04-20', 5, 2),
        ('Pranav Nair', 'pranav.nair@example.com', '+919876543219', '1996-10-05', 5, 2),
        ('Rahul Joshi', 'rahul.joshi@example.com', '+919876543220', '1994-02-28', 6, 2),
        ('Vivek Kumar', 'vivek.kumar@example.com', '+919876543221', '1992-08-15', 6, 2),
        ('Priya Sharma', 'priya.sharma@example.com', '+919876543230', '1995-03-15', 7, 2),
        ('Sneha Reddy', 'sneha.reddy@example.com', '+919876543231', '1993-07-22', 7, 2),
        ('Ananya Iyer', 'ananya.iyer@example.com', '+919876543232', '1990-11-08', 8, 2),
        ('Kavya Nair', 'kavya.nair@example.com', '+919876543233', '1996-05-12', 8, 2),
        ('Divya Krishna', 'divya.krishna@example.com', '+919876543234', '1992-09-30', 9, 2),
        ('Meera Patel', 'meera.patel@example.com', '+919876543235', '1994-06-18', 9, 2),
        ('Riya Singh', 'riya.singh@example.com', '+919876543236', '1991-12-25', 10, 2),
        ('Neha Gupta', 'neha.gupta@example.com', '+919876543237', '1995-08-14', 10, 2),
        ('Admin User', 'admin@sakkath.com', '+919876543200', '1990-01-01', NULL, 0)
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert test admin users with MD5 hashed passwords (password = email)
    // POC users: poc1@sakkath.com to poc6@sakkath.com for Open teams (1-6)
    // POC users: poc7@sakkath.com to poc10@sakkath.com for Women teams (7-10)
    sqlx::query(
        r#"
        INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, '7271937c096389c7cfbf9a886d04e531'),
        ('Admin One', 'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, 'ab0a172a23c3d89cda45cac015abaef3'),
        ('Admin Two', 'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, '6588b0f714c50f337776f94b286807b8'),
        ('POC Bolts', 'poc1@sakkath.com', '+919000000101', '1992-03-10', 1, 3, '7271937c096389c7cfbf9a886d04e531'),
        ('POC Challengers', 'poc2@sakkath.com', '+919000000102', '1993-04-11', 2, 3, 'ad0234829205b9033196ba818f7a872b'),
        ('POC Mavericks', 'poc3@sakkath.com', '+919000000103', '1991-05-12', 3, 3, '1a100d2c0dab19c4430e7d73762b3423'),
        ('POC Dragons', 'poc4@sakkath.com', '+919000000104', '1990-06-13', 4, 3, '3afc79b597f88a72528e864cf81856d2'),
        ('POC Hawks', 'poc5@sakkath.com', '+919000000105', '1994-07-14', 5, 3, 'c5fe25896e49ddfe996db7508cf00534'),
        ('POC Knights', 'poc6@sakkath.com', '+919000000106', '1989-08-15', 6, 3, '9f1d4eeb7d6bcbd13c53ddb0bc0954bf'),
        ('POC Blaze', 'poc7@sakkath.com', '+919000000107', '1995-09-16', 7, 3, 'be66e453a93cbe3b65bfb21598d924e0'),
        ('POC Chargers', 'poc8@sakkath.com', '+919000000108', '1988-10-17', 8, 3, '74e8733d436f5dddd28b6c49cc93ac44'),
        ('POC Meteors', 'poc9@sakkath.com', '+919000000109', '1992-11-18', 9, 3, 'b9a7c4d7bffe3e2ceae60ec8d25eb94a'),
        ('POC Divas', 'poc10@sakkath.com', '+919000000110', '1991-12-19', 10, 3, '3a6710e05dec0bf27f99bf3927d5a07d')
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert fields with hints
    sqlx::query(
        r#"
        INSERT INTO fields (name, hints, map_link) VALUES
        ('Astroturf Ground 1', 'Innermost astroturf ground. Cleats are preferred.', 'https://maps.google.com/?q=field1'),
        ('Astroturf Ground 2', 'Second astroturf field. Cleats recommended.', 'https://maps.google.com/?q=field2'),
        ('Natural Grass Field 1', 'Bring your own water. Shade available on sidelines.', 'https://maps.google.com/?q=field3'),
        ('Natural Grass Field 2', 'Wide open space. Can get windy in afternoons.', 'https://maps.google.com/?q=field4'),
        ('Practice Field', 'Smaller field for warmups. Lights available for evening.', 'https://maps.google.com/?q=field5'),
        ('Championship Field', 'Main tournament field with bleachers. Stream setup available.', 'https://maps.google.com/?q=field6')
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert some matches (Round 1)
    sqlx::query(
        r#"
        INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit) VALUES
        (1, 2, 1, '2026-01-15 09:00:00', 15, 12, 14, 13),
        (3, 4, 2, '2026-01-15 09:00:00', 13, 15, 12, 15),
        (5, 6, 3, '2026-01-15 09:00:00', 11, 9, 13, 14),
        (7, 8, 4, '2026-01-15 09:00:00', 15, 10, 15, 12),
        (9, 10, 5, '2026-01-15 09:00:00', 8, 15, 11, 14),
        (1, 3, 1, '2026-01-15 11:00:00', 14, 11, 13, 14),
        (2, 5, 2, '2026-01-15 11:00:00', 15, 13, 14, 13),
        (4, 6, 3, '2026-01-15 11:00:00', 12, 10, 15, 12)
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert match events (goals, assists, blocks, turnovers)
    sqlx::query(
        r#"
        INSERT INTO match_events (match_id, player_id, event_type) VALUES
        (1, 1, 0), (1, 1, 0), (1, 1, 0), (1, 2, 1), (1, 2, 1), (1, 1, 2),
        (1, 3, 0), (1, 3, 0), (1, 4, 1), (1, 4, 3),
        (2, 5, 0), (2, 5, 0), (2, 6, 1), (2, 6, 2),
        (2, 7, 0), (2, 7, 0), (2, 7, 0), (2, 8, 1), (2, 8, 2),
        (3, 9, 0), (3, 9, 0), (3, 10, 1), (3, 10, 3),
        (3, 11, 0), (3, 12, 1), (3, 12, 2),
        (4, 13, 0), (4, 13, 0), (4, 13, 0), (4, 14, 1), (4, 14, 1), (4, 14, 2),
        (4, 15, 0), (4, 15, 0), (4, 16, 1), (4, 16, 3),
        (5, 17, 0), (5, 18, 1), (5, 17, 3),
        (5, 19, 0), (5, 19, 0), (5, 19, 0), (5, 20, 1), (5, 20, 2)
        "#
    )
    .execute(pool)
    .await?;
    
    // Insert announcements
    sqlx::query(
        r#"
        INSERT INTO announcements (title, message, priority, expires_at) VALUES
        ('Welcome to Sakkath 2026!', 'We are excited to host the tournament. Please check the schedule for your matches.', 0, '2026-01-20 23:59:59'),
        ('Field Change Notice', 'Match between Bangalore Bolts and Chennai Challengers moved to Field 2.', 1, '2026-01-16 12:00:00'),
        ('Spirit Scoring Reminder', 'Please remember to submit spirit scores within 30 minutes after each match.', 2, '2026-01-18 23:59:59')
        "#
    )
    .execute(pool)
    .await?;
    
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
