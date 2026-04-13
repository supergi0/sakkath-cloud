use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const CSV_DATA_ROWS_TO_SKIP: usize = 2;
const EXPECTED_OPEN_TEAMS: usize = 22;
const EXPECTED_WOMEN_TEAMS: usize = 10;
const STANDARD_MEMBER_START: usize = 19;
const STANDARD_MEMBER_FIELDS: usize = 5;
const STANDARD_MEMBER_SLOTS: usize = 19;
const EXTRA_MEMBER_START: usize = STANDARD_MEMBER_START + (STANDARD_MEMBER_FIELDS * STANDARD_MEMBER_SLOTS);
const EXTRA_MEMBER_FIELDS: usize = 6;
const EXTRA_MEMBER_SLOTS: usize = 3;
const DEFAULT_STAFF_PASSWORD: &str = "helloworld";

struct ImportedTeamRow {
    division: i64,
    team_name: String,
    location: Option<String>,
    admin_name: String,
    admin_email: String,
    admin_phone: Option<String>,
    members: Vec<ImportedMember>,
}

struct ImportedMember {
    name: String,
    dob: Option<String>,
}

pub fn resolve_teams_csv_path(requested_path: Option<&str>) -> Result<PathBuf, sqlx::Error> {
    let requested = requested_path.unwrap_or("teams.csv");
    let requested_path = PathBuf::from(requested);

    if requested_path.is_absolute() && requested_path.exists() {
        return Ok(requested_path);
    }

    let current_dir = std::env::current_dir().map_err(|err| {
        sqlx::Error::Configuration(format!("Failed to resolve current directory: {err}").into())
    })?;

    for candidate in [
        current_dir.join(&requested_path),
        current_dir.join("..").join(&requested_path),
    ] {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(sqlx::Error::Configuration(
        format!("Could not find teams CSV at {}", requested_path.display()).into(),
    ))
}

pub async fn populate_from_teams_csv(pool: &SqlitePool, requested_path: Option<&str>) -> Result<(), sqlx::Error> {
    let csv_path = resolve_teams_csv_path(requested_path)?;
    let rows = load_team_rows(&csv_path)?;

    let mut tx = pool.begin().await?;
    seed_default_staff_and_fields(&mut tx).await?;

    let mut open_rank = 1i64;
    let mut women_rank = 1i64;
    let mut seen_admin_emails = HashSet::new();

    for row in rows {
        if !seen_admin_emails.insert(row.admin_email.clone()) {
            return Err(sqlx::Error::Configuration(
                format!("Duplicate admin email in teams.csv: {}", row.admin_email).into(),
            ));
        }

        let rank = if row.division == 0 {
            let current = open_rank;
            open_rank += 1;
            current
        } else {
            let current = women_rank;
            women_rank += 1;
            current
        };

        let team_result = sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, ?, ?, ?)"
        )
        .bind(&row.team_name)
        .bind(row.division)
        .bind(&row.location)
        .bind(rank)
        .execute(&mut *tx)
        .await?;

        let team_id = team_result.last_insert_rowid();
        let mut member_ids_by_name = HashMap::new();

        for member in row.members {
            let member_key = normalize_key(&member.name);
            if member_key.is_empty() || member_ids_by_name.contains_key(&member_key) {
                continue;
            }

            let member_result = sqlx::query(
                "INSERT INTO users (name, dob, team_id, role, is_captain, is_spirit_captain) VALUES (?, ?, ?, 2, 0, 0)"
            )
            .bind(&member.name)
            .bind(&member.dob)
            .bind(team_id)
            .execute(&mut *tx)
            .await?;

            member_ids_by_name.insert(member_key, member_result.last_insert_rowid());
        }

        let admin_key = normalize_key(&row.admin_name);
        let admin_password_hash = format!("{:x}", md5::compute(&row.admin_email));

        let admin_id = if let Some(existing_member_id) = member_ids_by_name.get(&admin_key) {
            sqlx::query(
                "UPDATE users SET email = ?, phone = ?, role = 3, password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&row.admin_email)
            .bind(&row.admin_phone)
            .bind(&admin_password_hash)
            .bind(existing_member_id)
            .execute(&mut *tx)
            .await?;

            *existing_member_id
        } else {
            let admin_result = sqlx::query(
                "INSERT INTO users (name, email, phone, team_id, role, password_hash) VALUES (?, ?, ?, ?, 3, ?)"
            )
            .bind(&row.admin_name)
            .bind(&row.admin_email)
            .bind(&row.admin_phone)
            .bind(team_id)
            .bind(&admin_password_hash)
            .execute(&mut *tx)
            .await?;

            admin_result.last_insert_rowid()
        };

        sqlx::query("UPDATE teams SET admin_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(admin_id)
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn load_team_rows(csv_path: &Path) -> Result<Vec<ImportedTeamRow>, sqlx::Error> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(csv_path)
        .map_err(|err| sqlx::Error::Configuration(format!("Failed to open teams CSV: {err}").into()))?;

    let mut records = Vec::new();
    for record in reader.records() {
        let record = record
            .map_err(|err| sqlx::Error::Configuration(format!("Failed to parse teams CSV: {err}").into()))?;
        if is_team_record(&record) {
            records.push(record);
        }
    }

    let rows = records
        .into_iter()
        .skip(CSV_DATA_ROWS_TO_SKIP)
        .map(|record| build_team_row(&record))
        .collect::<Result<Vec<_>, _>>()?;

    let open_count = rows.iter().filter(|row| row.division == 0).count();
    let women_count = rows.iter().filter(|row| row.division == 1).count();

    if open_count != EXPECTED_OPEN_TEAMS || women_count != EXPECTED_WOMEN_TEAMS {
        return Err(sqlx::Error::Configuration(
            format!(
                "teams.csv import expected 23 Open and 10 Women teams after skipping the first 2 actual team submissions, got {open_count} Open and {women_count} Women"
            )
            .into(),
        ));
    }

    Ok(rows)
}

fn build_team_row(record: &StringRecord) -> Result<ImportedTeamRow, sqlx::Error> {
    let division = match normalize_text(record.get(2)).as_deref() {
        Some("open") => 0,
        Some("women") => 1,
        Some(other) => {
            return Err(sqlx::Error::Configuration(
                format!("Unsupported division in teams.csv: {other}").into(),
            ))
        }
        None => return Err(sqlx::Error::Configuration("Missing division in teams.csv".into())),
    };

    let team_name = normalize_display_text(record.get(3))
        .ok_or_else(|| sqlx::Error::Configuration("Missing team name in teams.csv".into()))?;
    let admin_email = normalize_email(record.get(1))
        .ok_or_else(|| sqlx::Error::Configuration(format!("Missing form email for team {team_name}").into()))?;
    let members = extract_members(record);
    let admin_name = normalize_person_name(record.get(15))
        .filter(|value| !looks_like_phone(value) && !looks_like_email(value))
        .or_else(|| members.first().map(|member| member.name.clone()))
        .unwrap_or_else(|| fallback_name_from_email(&admin_email));

    Ok(ImportedTeamRow {
        division,
        team_name,
        location: combine_location(record.get(4), record.get(5)),
        admin_name,
        admin_email,
        admin_phone: normalize_phone(record.get(17)).or_else(|| normalize_phone(record.get(18))),
        members,
    })
}

fn extract_members(record: &StringRecord) -> Vec<ImportedMember> {
    let mut members = Vec::new();
    let mut seen_names = HashSet::new();

    for slot in 0..STANDARD_MEMBER_SLOTS {
        let start = STANDARD_MEMBER_START + (slot * STANDARD_MEMBER_FIELDS);
        push_member(
            &mut members,
            &mut seen_names,
            record.get(start),
            record.get(start + 1),
            record.get(start + 2),
        );
    }

    for slot in 0..EXTRA_MEMBER_SLOTS {
        let start = EXTRA_MEMBER_START + (slot * EXTRA_MEMBER_FIELDS);
        push_member(
            &mut members,
            &mut seen_names,
            record.get(start + 1),
            record.get(start + 2),
            record.get(start + 3),
        );
    }

    members
}

fn push_member(
    members: &mut Vec<ImportedMember>,
    seen_names: &mut HashSet<String>,
    full_name: Option<&str>,
    common_name: Option<&str>,
    dob: Option<&str>,
) {
    let display_name = normalize_person_name(common_name)
        .filter(|value| value != "-")
        .or_else(|| normalize_person_name(full_name));

    let Some(name) = display_name else {
        return;
    };

    let key = normalize_key(&name);
    if key.is_empty() || seen_names.contains(&key) {
        return;
    }

    seen_names.insert(key);
    members.push(ImportedMember {
        name,
        dob: normalize_date(dob),
    });
}

async fn seed_default_staff_and_fields(tx: &mut Transaction<'_, Sqlite>) -> Result<(), sqlx::Error> {
    let password_hash = format!("{:x}", md5::compute(DEFAULT_STAFF_PASSWORD));

    sqlx::query(
        r#"INSERT INTO users (name, email, phone, dob, team_id, role, password_hash) VALUES
        ('Super Admin', 'super@sakkath.com', '+919000000000', '1985-01-01', NULL, 0, ?),
        ('Admin One', 'admin1@sakkath.com', '+919000000001', '1988-05-15', NULL, 1, ?),
        ('Admin Two', 'admin2@sakkath.com', '+919000000002', '1990-10-20', NULL, 1, ?)"#,
    )
    .bind(&password_hash)
    .bind(&password_hash)
    .bind(&password_hash)
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Field Alpha', 'Main Astroturf Ground, Sports Complex, North Block', 'https://maps.google.com/?q=field+alpha+sports+complex'),
        ('Field Bravo', 'Secondary Astroturf, Sports Complex, East Block', 'https://maps.google.com/?q=field+bravo+sports+complex'),
        ('Field Charlie', 'Natural Grass Field, Sports Complex, South Block', 'https://maps.google.com/?q=field+charlie+sports+complex'),
        ('Field Delta', 'Open Grass Field, Sports Complex, West Block', 'https://maps.google.com/?q=field+delta+sports+complex')"#,
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn is_team_record(record: &StringRecord) -> bool {
    record
        .get(0)
        .and_then(|value| value.get(..10))
        .and_then(|value| NaiveDate::parse_from_str(value, "%d/%m/%Y").ok())
        .is_some()
}

fn combine_location(city: Option<&str>, state: Option<&str>) -> Option<String> {
    match (normalize_display_text(city), normalize_display_text(state)) {
        (Some(city), Some(state)) if city.eq_ignore_ascii_case(&state) => Some(city),
        (Some(city), Some(state)) => Some(format!("{city}, {state}")),
        (Some(city), None) => Some(city),
        (None, Some(state)) => Some(state),
        (None, None) => None,
    }
}

fn normalize_date(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    let date = NaiveDate::parse_from_str(value, "%d/%m/%Y").ok()?;
    let year = date.format("%Y").to_string().parse::<i32>().ok()?;
    if !(1900..=2100).contains(&year) {
        return None;
    }

    Some(date.format("%Y-%m-%d").to_string())
}

fn normalize_person_name(value: Option<&str>) -> Option<String> {
    normalize_display_text(value)
}

fn normalize_display_text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    normalize_display_text(value).map(|value| value.to_ascii_lowercase())
}

fn normalize_email(value: Option<&str>) -> Option<String> {
    let value = normalize_display_text(value)?;
    if looks_like_email(&value) {
        Some(value.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_phone(value: Option<&str>) -> Option<String> {
    let value = normalize_display_text(value)?;
    if looks_like_phone(&value) {
        Some(value)
    } else {
        None
    }
}

fn looks_like_email(value: &str) -> bool {
    value.contains('@')
}

fn looks_like_phone(value: &str) -> bool {
    value.chars().filter(|ch| ch.is_ascii_digit()).count() >= 7
}

fn fallback_name_from_email(email: &str) -> String {
    email
        .split('@')
        .next()
        .unwrap_or("team-admin")
        .replace('.', " ")
        .replace('_', " ")
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}