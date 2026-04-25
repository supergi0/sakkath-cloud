use sqlx::SqlitePool;

fn hash_password(password: &str) -> Result<String, sqlx::Error> {
    crate::helpers::auth::hash_password(password)
        .map_err(|err| sqlx::Error::Configuration(format!("Failed to hash password: {err}").into()))
}

pub async fn populate_mock_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let pw_hash = hash_password("helloworld")?;

    // -----------------------------------------------------------------------
    // TEAMS: 22 Open (IDs 1-22) + 10 Women (IDs 23-32) = 32 total
    // -----------------------------------------------------------------------
    let open_team_names = [
        "Bangalore Bolts",
        "Chennai Challengers",
        "Mumbai Mavericks",
        "Delhi Dragons",
        "Hyderabad Hawks",
        "Kolkata Knights",
        "Pune Panthers",
        "Ahmedabad Aces",
        "Jaipur Jaguars",
        "Lucknow Lions",
        "Kochi Kings",
        "Goa Gladiators",
        "Chandigarh Chargers",
        "Indore Infernos",
        "Nagpur Ninjas",
        "Vizag Vikings",
        "Coimbatore Cosmos",
        "Mysore Mambas",
        "Surat Strikers",
        "Bhopal Blazers",
        "Patna Pioneers",
        "Vadodara Vipers",
    ];
    let open_locations = [
        "Bangalore",
        "Chennai",
        "Mumbai",
        "Delhi",
        "Hyderabad",
        "Kolkata",
        "Pune",
        "Ahmedabad",
        "Jaipur",
        "Lucknow",
        "Kochi",
        "Goa",
        "Chandigarh",
        "Indore",
        "Nagpur",
        "Vizag",
        "Coimbatore",
        "Mysore",
        "Surat",
        "Bhopal",
        "Patna",
        "Vadodara",
    ];
    let women_team_names = [
        "Bangalore Blaze",
        "Chennai Chargers",
        "Mumbai Meteors",
        "Delhi Divas",
        "Hyderabad Hurricanes",
        "Kolkata Queens",
        "Pune Pythons",
        "Ahmedabad Angels",
        "Jaipur Jewels",
        "Lucknow Legends",
    ];
    let women_locations = [
        "Bangalore",
        "Chennai",
        "Mumbai",
        "Delhi",
        "Hyderabad",
        "Kolkata",
        "Pune",
        "Ahmedabad",
        "Jaipur",
        "Lucknow",
    ];

    // Insert Open teams
    for (i, (name, loc)) in open_team_names
        .iter()
        .zip(open_locations.iter())
        .enumerate()
    {
        let rank = i + 1;
        sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, 0, ?, ?)"
        ).bind(name).bind(loc).bind(rank as i64).execute(pool).await?;
    }

    // Insert Women teams
    for (i, (name, loc)) in women_team_names
        .iter()
        .zip(women_locations.iter())
        .enumerate()
    {
        let rank = i + 1;
        sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, 1, ?, ?)"
        ).bind(name).bind(loc).bind(rank as i64).execute(pool).await?;
    }

    // -----------------------------------------------------------------------
    // PLAYERS: 16 per team, looping through name lists
    // Open team IDs: 1-22, Women team IDs: 23-32
    // -----------------------------------------------------------------------
    let open_first = [
        "Raj", "Amit", "Vikram", "Rohan", "Arjun", "Sanjay", "Karthik", "Nikhil", "Aditya",
        "Pranav", "Rahul", "Vivek", "Suresh", "Ganesh", "Mohan", "Ravi",
    ];
    let women_first = [
        "Priya", "Sneha", "Ananya", "Kavya", "Divya", "Meera", "Riya", "Neha", "Pooja", "Shruti",
        "Swati", "Nisha", "Aditi", "Isha", "Tara", "Kiara",
    ];
    let last_names = [
        "Kumar", "Patel", "Sharma", "Singh", "Reddy", "Rao", "Das", "Gupta", "Jain", "Mehta",
        "Bose", "Nair", "Iyer", "Pillai", "Chauhan", "Shah",
    ];

    let mut player_id: usize = 1;
    let mut batch: Vec<String> = Vec::new();

    macro_rules! flush_batch {
        ($b:expr, $p:expr) => {
            if !$b.is_empty() {
                let q = format!(
                    "INSERT INTO users (name, common_name, email, phone, dob, team_id, role, password_hash) VALUES {}",
                    $b.join(",")
                );
                sqlx::query(&q).execute($p).await?;
                $b.clear();
            }
        };
    }

    // Open division players (teams 1-22)
    for team_id in 1usize..=22 {
        for slot in 0usize..16 {
            let first = open_first[slot % 16];
            let last = last_names[(team_id + slot) % 16];
            let name = format!("{} {}", first, last);
            let dob = if slot % 2 == 0 {
                "1995-03-15"
            } else {
                "1997-07-22"
            };
            batch.push(format!(
                "('{}', '{}', 'p{}@example.com', '+919876{:06}', '{}', {}, 2, '{}')",
                name,
                first,
                player_id,
                player_id % 1000000,
                dob,
                team_id,
                pw_hash
            ));
            player_id += 1;
            if batch.len() >= 50 {
                flush_batch!(batch, pool);
            }
        }
    }

    // Women division players (teams 23-32)
    for team_offset in 0usize..10 {
        let team_id = 23 + team_offset;
        for slot in 0usize..16 {
            let first = women_first[slot % 16];
            let last = last_names[(team_id + slot) % 16];
            let name = format!("{} {}", first, last);
            let dob = if slot % 2 == 0 {
                "1996-05-10"
            } else {
                "1998-11-25"
            };
            batch.push(format!(
                "('{}', '{}', 'p{}@example.com', '+919876{:06}', '{}', {}, 2, '{}')",
                name,
                first,
                player_id,
                player_id % 1000000,
                dob,
                team_id,
                pw_hash
            ));
            player_id += 1;
            if batch.len() >= 50 {
                flush_batch!(batch, pool);
            }
        }
    }

    // Flush remainder
    flush_batch!(batch, pool);

    // -----------------------------------------------------------------------
    // ADMIN / POC USERS
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO users (name, common_name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, ?),
        ('Admin One', 'Admin One', 'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, ?),
        ('Admin Two', 'Admin Two', 'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, ?)"#
    ).bind(&pw_hash).bind(&pw_hash).bind(&pw_hash).execute(pool).await?;

    // POC per team (32 teams)
    let mut poc_batch: Vec<String> = Vec::new();
    for team_id in 1usize..=32 {
        poc_batch.push(format!(
            "('POC Team {tid}', 'POC Team {tid}', 'poc{tid}@sakkath.com', '+91900{tid:07}', '1992-03-10', {tid}, 3, '{pw}')",
            tid = team_id, pw = pw_hash
        ));
    }
    let poc_q = format!(
        "INSERT INTO users (name, common_name, email, phone, dob, team_id, role, password_hash) VALUES {}",
        poc_batch.join(",")
    );
    sqlx::query(&poc_q).execute(pool).await?;

    // -----------------------------------------------------------------------
    // FIELDS (4 fields)
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Ground 1', 'Main Astroturf Ground, Sports Complex, North Block',   'https://maps.google.com/?q=ground+1+sports+complex'),
        ('Ground 2', 'Secondary Astroturf, Sports Complex, East Block',      'https://maps.google.com/?q=ground+2+sports+complex'),
        ('Ground 3', 'Natural Grass Field, Sports Complex, South Block',     'https://maps.google.com/?q=ground+3+sports+complex'),
        ('Ground 4', 'Open Grass Field, Sports Complex, West Block',         'https://maps.google.com/?q=ground+4+sports+complex')"#
    ).execute(pool).await?;

    // -----------------------------------------------------------------------
    // ROUND 1 MATCHES  (type = 1)
    //
    // Open (teams 1-22): 11 matches (1v12, 2v13, ... 11v22)
    //   Completed (possession=3): matches 1-8
    //   Live (possession=1): match 9v20
    //   Not started (possession=NULL): 10v21, 11v22
    //
    // Women (teams 23-32): 5 matches (23v28, 24v29, 25v30, 26v31, 27v32)
    //   Completed: 23v28, 24v29, 25v30
    //   Live: 26v31
    //   Not started: 27v32
    // -----------------------------------------------------------------------

    // Open R1 - completed matches (1v12 through 8v19)
    let open_completed: &[(i64, i64, i64, i64, i64)] = &[
        (1, 12, 1, 15, 9),
        (2, 13, 2, 15, 10),
        (3, 14, 3, 14, 11),
        (4, 15, 4, 15, 8),
        (5, 16, 1, 13, 11),
        (6, 17, 2, 14, 10),
        (7, 18, 3, 15, 12),
        (8, 19, 4, 13, 9),
    ];
    for &(t1, t2, field, s1, s2) in open_completed {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (?, ?, ?, '2026-03-15 09:00:00', ?, ?, 3, 1)"
        ).bind(t1).bind(t2).bind(field).bind(s1).bind(s2).execute(pool).await?;
    }

    // Open R1 - live match (9v20, score 7-5, possession = t1)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (9, 20, 1, '2026-03-15 11:00:00', 7, 5, NULL, NULL, 1, 1)"
    ).execute(pool).await?;

    // Open R1 - not started (10v21, 11v22)
    let open_upcoming: &[(i64, i64, i64)] = &[(10, 21, 2), (11, 22, 3)];
    for &(t1, t2, field) in open_upcoming {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (?, ?, ?, '2026-03-15 13:00:00', 0, 0, NULL, 1)"
        ).bind(t1).bind(t2).bind(field).execute(pool).await?;
    }

    // Women R1 - completed matches
    let women_completed: &[(i64, i64, i64, i64, i64)] =
        &[(23, 28, 4, 15, 8), (24, 29, 1, 13, 11), (25, 30, 2, 15, 10)];
    for &(t1, t2, field, s1, s2) in women_completed {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (?, ?, ?, '2026-03-15 09:00:00', ?, ?, 3, 1)"
        ).bind(t1).bind(t2).bind(field).bind(s1).bind(s2).execute(pool).await?;
    }

    // Women R1 - live (26v31, score 6-6)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (26, 31, 3, '2026-03-15 11:00:00', 6, 6, NULL, NULL, 2, 1)"
    ).execute(pool).await?;

    // Women R1 - not started (27v32)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (27, 32, 4, '2026-03-15 13:00:00', 0, 0, NULL, 1)"
    ).execute(pool).await?;

    // -----------------------------------------------------------------------
    // MATCH EVENTS for completed Open matches (goals/assists/blocks for match 1-2)
    // player_id offsets: team 1 = players 1-16, team 12 = players (11*16+1)=177 to 192
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO match_events (match_id, player_id, team_id, event_type, created_at) VALUES
        (1, 1,  1, 0, '2026-03-15 09:05:00'),
        (1, 2,  1, 1, '2026-03-15 09:05:00'),
        (1, 3,  1, 0, '2026-03-15 09:12:00'),
        (1, 1,  1, 2, '2026-03-15 09:15:00'),
        (1, 4,  1, 0, '2026-03-15 09:18:00'),
        (1, 2,  1, 1, '2026-03-15 09:18:00'),
        (2, 17, 2, 0, '2026-03-15 09:06:00'),
        (2, 18, 2, 1, '2026-03-15 09:06:00'),
        (2, 17, 2, 0, '2026-03-15 09:14:00')"#,
    )
    .execute(pool)
    .await?;

    // -----------------------------------------------------------------------
    // ANNOUNCEMENTS
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO announcements (title, message, priority, expires_at) VALUES
        ('Welcome to Sakkath 2026!', 'Tournament runs March 15-17. Check the schedule for your match times.', 0, '2026-03-20 23:59:59'),
        ('Spirit Scoring Reminder', 'Submit spirit scores within 30 minutes after each match ends.', 1, '2026-03-20 23:59:59'),
        ('Round 1 Underway', 'Round 1 is in progress. 11 of 16 matches completed. Live matches on Field 1 and 3!', 1, '2026-03-20 23:59:59')"#
    ).execute(pool).await?;

    tracing::info!(
        "Seeded: 22 open teams, 10 women teams, 512 players, 16 R1 matches (11 done, 2 live, 3 upcoming)"
    );

    Ok(())
}
