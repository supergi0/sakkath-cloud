use sqlx::SqlitePool;

pub async fn populate_mock_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let pw_hash = "fc5e038d38a57032085441e7fe7010b0"; // MD5 of "helloworld"

    // -----------------------------------------------------------------------
    // TEAMS: 24 Open (IDs 1-24) + 10 Women (IDs 25-34) = 34 total
    // -----------------------------------------------------------------------
    let open_team_names = [
        "Bangalore Bolts", "Chennai Challengers", "Mumbai Mavericks", "Delhi Dragons",
        "Hyderabad Hawks", "Kolkata Knights", "Pune Panthers", "Ahmedabad Aces",
        "Jaipur Jaguars", "Lucknow Lions", "Kochi Kings", "Goa Gladiators",
        "Chandigarh Chargers", "Indore Infernos", "Nagpur Ninjas", "Vizag Vikings",
        "Coimbatore Cosmos", "Mysore Mambas", "Surat Strikers", "Bhopal Blazers",
        "Patna Pioneers", "Vadodara Vipers", "Ludhiana Lynx", "Agra Archers",
    ];
    let open_locations = [
        "Bangalore", "Chennai", "Mumbai", "Delhi", "Hyderabad", "Kolkata",
        "Pune", "Ahmedabad", "Jaipur", "Lucknow", "Kochi", "Goa",
        "Chandigarh", "Indore", "Nagpur", "Vizag", "Coimbatore", "Mysore",
        "Surat", "Bhopal", "Patna", "Vadodara", "Ludhiana", "Agra",
    ];
    let women_team_names = [
        "Bangalore Blaze", "Chennai Chargers", "Mumbai Meteors", "Delhi Divas",
        "Hyderabad Hurricanes", "Kolkata Queens", "Pune Pythons", "Ahmedabad Angels",
        "Jaipur Jewels", "Lucknow Legends",
    ];
    let women_locations = [
        "Bangalore", "Chennai", "Mumbai", "Delhi", "Hyderabad",
        "Kolkata", "Pune", "Ahmedabad", "Jaipur", "Lucknow",
    ];

    // Insert Open teams
    for (i, (name, loc)) in open_team_names.iter().zip(open_locations.iter()).enumerate() {
        let rank = i + 1;
        sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, 0, ?, ?)"
        ).bind(name).bind(loc).bind(rank as i64).execute(pool).await?;
    }

    // Insert Women teams
    for (i, (name, loc)) in women_team_names.iter().zip(women_locations.iter()).enumerate() {
        let rank = i + 1;
        sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, 1, ?, ?)"
        ).bind(name).bind(loc).bind(rank as i64).execute(pool).await?;
    }

    // -----------------------------------------------------------------------
    // PLAYERS: 16 per team, looping through name lists
    // Open team IDs: 1-24, Women team IDs: 25-34
    // -----------------------------------------------------------------------
    let open_first = ["Raj", "Amit", "Vikram", "Rohan", "Arjun", "Sanjay", "Karthik", "Nikhil",
                      "Aditya", "Pranav", "Rahul", "Vivek", "Suresh", "Ganesh", "Mohan", "Ravi"];
    let women_first = ["Priya", "Sneha", "Ananya", "Kavya", "Divya", "Meera", "Riya", "Neha",
                       "Pooja", "Shruti", "Swati", "Nisha", "Aditi", "Isha", "Tara", "Kiara"];
    let last_names = ["Kumar", "Patel", "Sharma", "Singh", "Reddy", "Rao", "Das", "Gupta",
                      "Jain", "Mehta", "Bose", "Nair", "Iyer", "Pillai", "Chauhan", "Shah"];

    let mut player_id: usize = 1;
    let mut batch: Vec<String> = Vec::new();

    macro_rules! flush_batch {
        ($b:expr, $p:expr) => {
            if !$b.is_empty() {
                let q = format!(
                    "INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES {}",
                    $b.join(",")
                );
                sqlx::query(&q).execute($p).await?;
                $b.clear();
            }
        };
    }

    // Open division players (teams 1-24)
    for team_id in 1usize..=24 {
        for slot in 0usize..16 {
            let first = open_first[slot % 16];
            let last = last_names[(team_id + slot) % 16];
            let name = format!("{} {}", first, last);
            let dob = if slot % 2 == 0 { "1995-03-15" } else { "1997-07-22" };
            batch.push(format!(
                "('{}', 'p{}@example.com', '+919876{:06}', '{}', {}, 2, '{}')",
                name, player_id, player_id % 1000000, dob, team_id, pw_hash
            ));
            player_id += 1;
            if batch.len() >= 50 { flush_batch!(batch, pool); }
        }
    }

    // Women division players (teams 25-34)
    for team_offset in 0usize..10 {
        let team_id = 25 + team_offset;
        for slot in 0usize..16 {
            let first = women_first[slot % 16];
            let last = last_names[(team_id + slot) % 16];
            let name = format!("{} {}", first, last);
            let dob = if slot % 2 == 0 { "1996-05-10" } else { "1998-11-25" };
            batch.push(format!(
                "('{}', 'p{}@example.com', '+919876{:06}', '{}', {}, 2, '{}')",
                name, player_id, player_id % 1000000, dob, team_id, pw_hash
            ));
            player_id += 1;
            if batch.len() >= 50 { flush_batch!(batch, pool); }
        }
    }

    // Flush remainder
    flush_batch!(batch, pool);

    // -----------------------------------------------------------------------
    // ADMIN / POC USERS
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, ?),
        ('Admin One',   'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, ?),
        ('Admin Two',   'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, ?)"#
    ).bind(pw_hash).bind(pw_hash).bind(pw_hash).execute(pool).await?;

    // POC per team
    let mut poc_batch: Vec<String> = Vec::new();
    for team_id in 1usize..=34 {
        poc_batch.push(format!(
            "('POC Team {tid}', 'poc{tid}@sakkath.com', '+91900{tid:07}', '1992-03-10', {tid}, 3, '{pw}')",
            tid = team_id, pw = pw_hash
        ));
    }
    let poc_q = format!("INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES {}", poc_batch.join(","));
    sqlx::query(&poc_q).execute(pool).await?;

    // -----------------------------------------------------------------------
    // FIELDS
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Field 1', 'Main astroturf ground. Cleats preferred.',      'https://maps.google.com/?q=field1'),
        ('Field 2', 'Secondary astroturf. Cleats recommended.',      'https://maps.google.com/?q=field2'),
        ('Field 3', 'Natural grass. Bring water.',                   'https://maps.google.com/?q=field3'),
        ('Field 4', 'Open grass field. Windy afternoons.',           'https://maps.google.com/?q=field4'),
        ('Field 5', 'Practice field. Flat surface.',                 'https://maps.google.com/?q=field5'),
        ('Field 6', 'Corner field. Good lighting.',                  'https://maps.google.com/?q=field6'),
        ('Field 7', 'Backup field. Limited seating.',                'https://maps.google.com/?q=field7')"#
    ).execute(pool).await?;

    // -----------------------------------------------------------------------
    // ROUND 1 MATCHES  (type = 1)
    //
    // Open (teams 1-24): 12 matches (1v13, 2v14, ... 12v24)
    //   Completed (possession=3): matches 1-8
    //   Live / in-progress (possession=1 or 2): match 9
    //   Not started (possession=NULL, score=0-0): matches 10-12
    //
    // Women (teams 25-34): 5 matches (25v30, 26v31, 27v32, 28v33, 29v34)
    //   Completed: matches 25v30, 26v31, 27v32
    //   Live: 28v33
    //   Not started: 29v34
    // -----------------------------------------------------------------------

    // Open R1 - completed matches (1-8)
    let open_completed: &[(i64, i64, i64, i64, i64, i64, i64)] = &[
        (1, 13, 1, 15, 9,  14, 13),
        (2, 14, 2, 15, 10, 13, 14),
        (3, 15, 3, 14, 11, 14, 12),
        (4, 16, 4, 15, 8,  13, 13),
        (5, 17, 5, 13, 11, 15, 14),
        (6, 18, 6, 14, 10, 14, 13),
        (7, 19, 7, 15, 12, 13, 14),
        (8, 20, 1, 13, 9,  14, 15),
    ];
    for &(t1, t2, field, s1, s2, sp1, sp2) in open_completed {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (?, ?, ?, '2026-03-15 09:00:00', ?, ?, ?, ?, 3, 1)"
        ).bind(t1).bind(t2).bind(field).bind(s1).bind(s2).bind(sp1).bind(sp2).execute(pool).await?;
    }

    // Open R1 - live match (9v21, score 7-5, possession = t1)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (9, 21, 2, '2026-03-15 11:00:00', 7, 5, NULL, NULL, 1, 1)"
    ).execute(pool).await?;

    // Open R1 - not started (10v22, 11v23, 12v24)
    let open_upcoming: &[(i64, i64, i64)] = &[
        (10, 22, 3),
        (11, 23, 4),
        (12, 24, 5),
    ];
    for &(t1, t2, field) in open_upcoming {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (?, ?, ?, '2026-03-15 13:00:00', 0, 0, NULL, 1)"
        ).bind(t1).bind(t2).bind(field).execute(pool).await?;
    }

    // Women R1 - completed matches
    let women_completed: &[(i64, i64, i64, i64, i64, i64, i64)] = &[
        (25, 30, 6, 15, 8,  14, 13),
        (26, 31, 7, 13, 11, 13, 15),
        (27, 32, 1, 15, 10, 14, 14),
    ];
    for &(t1, t2, field, s1, s2, sp1, sp2) in women_completed {
        sqlx::query(
            "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (?, ?, ?, '2026-03-15 09:00:00', ?, ?, ?, ?, 3, 1)"
        ).bind(t1).bind(t2).bind(field).bind(s1).bind(s2).bind(sp1).bind(sp2).execute(pool).await?;
    }

    // Women R1 - live (28v33, score 6-6)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, t1_spirit, t2_spirit, possession, type) VALUES (28, 33, 2, '2026-03-15 11:00:00', 6, 6, NULL, NULL, 2, 1)"
    ).execute(pool).await?;

    // Women R1 - not started (29v34)
    sqlx::query(
        "INSERT INTO matches (t1_id, t2_id, field_id, time, t1_score, t2_score, possession, type) VALUES (29, 34, 3, '2026-03-15 13:00:00', 0, 0, NULL, 1)"
    ).execute(pool).await?;

    // -----------------------------------------------------------------------
    // MATCH EVENTS for completed Open matches (goals/assists/blocks for match 1)
    // player_id offsets: team 1 = players 1-16, team 13 = players (12*16+1)=193 to 208
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
        (2, 17, 2, 0, '2026-03-15 09:14:00')"#
    ).execute(pool).await?;

    // -----------------------------------------------------------------------
    // ANNOUNCEMENTS
    // -----------------------------------------------------------------------
    sqlx::query(
        r#"INSERT INTO announcements (title, message, priority, expires_at) VALUES
        ('Welcome to Sakkath 2026!', 'Tournament runs March 15-17. Check the schedule for your match times.', 0, '2026-03-20 23:59:59'),
        ('Spirit Scoring Reminder', 'Submit spirit scores within 30 minutes after each match ends.', 1, '2026-03-20 23:59:59'),
        ('Round 1 Underway', 'Round 1 is in progress. 11 of 17 matches completed. Live match on Field 2!', 1, '2026-03-20 23:59:59')"#
    ).execute(pool).await?;

    tracing::info!("Seeded: 24 open teams, 10 women teams, 544 players, 17 R1 matches (11 done, 2 live, 4 upcoming)");

    Ok(())
}
