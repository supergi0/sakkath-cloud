use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const CSV_DATA_ROWS_TO_SKIP: usize = 2;
const EXPECTED_OPEN_TEAMS: usize = 22;
const EXPECTED_WOMEN_TEAMS: usize = 10;
const RANKING_COLUMN: usize = 4;
const CITY_COLUMN: usize = 5;
const STATE_COLUMN: usize = 6;
const ADMIN_NAME_COLUMN: usize = 16;
const CONTACT_EMAIL_COLUMN: usize = 17;
const ADMIN_PHONE_PRIMARY_COLUMN: usize = 18;
const ADMIN_PHONE_FALLBACK_COLUMN: usize = 19;
const FORM_EMAIL_COLUMN: usize = 1;
const STANDARD_MEMBER_START: usize = 20;
const STANDARD_MEMBER_FIELDS: usize = 5;
const STANDARD_MEMBER_SLOTS: usize = 19;
const EXTRA_MEMBER_START: usize =
    STANDARD_MEMBER_START + (STANDARD_MEMBER_FIELDS * STANDARD_MEMBER_SLOTS);
const EXTRA_MEMBER_FIELDS: usize = 6;
const EXTRA_MEMBER_SLOTS: usize = 3;
const PASSWORD_LENGTH: usize = 12;
const JWT_SECRET_PASSWORD_CHARS: usize = 4;
const RANDOM_PASSWORD_CHARS: usize = 8;
const PASSWORD_OUTPUT_FILE: &str = "passwords.json";
const PASSWORD_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const UPPERCASE_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGIT_ALPHABET: &[u8] = b"0123456789";

fn hash_password(password: &str) -> Result<String, sqlx::Error> {
    crate::helpers::auth::hash_password(password)
        .map_err(|err| sqlx::Error::Configuration(format!("Failed to hash password: {err}").into()))
}

#[derive(Default, Serialize, Deserialize)]
struct PasswordManifest {
    #[serde(rename = "super")]
    super_admins: Vec<PasswordManifestEntry>,
    opens: Vec<PasswordManifestEntry>,
    womens: Vec<PasswordManifestEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PasswordManifestEntry {
    email: String,
    password: String,
    team: Option<String>,
}

impl PasswordManifest {
    fn push_super(&mut self, email: String, password: String) {
        self.super_admins.push(PasswordManifestEntry {
            email,
            password,
            team: None,
        });
    }

    fn push_division(&mut self, division: i64, email: String, password: String, team: String) {
        let entry = PasswordManifestEntry {
            email,
            password,
            team: Some(team),
        };
        match division {
            0 => self.opens.push(entry),
            1 => self.womens.push(entry),
            _ => {}
        }
    }

    fn find_super_password(&self, email: &str) -> Option<String> {
        let email_key = normalize_key(email);
        self.super_admins
            .iter()
            .find(|entry| normalize_key(&entry.email) == email_key)
            .map(|entry| entry.password.clone())
    }

    fn find_division_password(&self, division: i64, email: &str, team: &str) -> Option<String> {
        let entries = match division {
            0 => &self.opens,
            1 => &self.womens,
            _ => return None,
        };
        let team_key = normalize_key(team);
        let email_key = normalize_key(email);

        entries
            .iter()
            .find(|entry| {
                entry
                    .team
                    .as_deref()
                    .map(normalize_key)
                    .as_deref()
                    == Some(team_key.as_str())
            })
            .or_else(|| entries.iter().find(|entry| normalize_key(&entry.email) == email_key))
            .map(|entry| entry.password.clone())
    }
}

struct ImportedTeamRow {
    division: i64,
    team_name: String,
    init_rank: i64,
    location: Option<String>,
    admin_name: String,
    admin_email: String,
    admin_phone: Option<String>,
    members: Vec<ImportedMember>,
}

struct ImportedMember {
    full_name: String,
    common_name: Option<String>,
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

pub async fn populate_from_teams_csv(
    pool: &SqlitePool,
    requested_path: Option<&str>,
    replace_password: bool,
) -> Result<(), sqlx::Error> {
    let csv_path = resolve_teams_csv_path(requested_path)?;
    let rows = load_team_rows(&csv_path)?;
    let jwt_secret = load_jwt_secret_for_passwords()?;
    let super_admin_emails = load_super_admin_emails()?;
    let manifest_path = resolve_password_manifest_path(&csv_path);
    let existing_manifest = if replace_password {
        PasswordManifest::default()
    } else {
        load_existing_password_manifest(&manifest_path)?
    };
    let mut password_manifest = PasswordManifest::default();

    let mut tx = pool.begin().await?;
    seed_default_staff_and_fields(
        &mut tx,
        &super_admin_emails,
        &jwt_secret,
        &existing_manifest,
        &mut password_manifest,
    )
    .await?;

    let mut seen_admin_emails = super_admin_emails.into_iter().collect::<HashSet<_>>();

    for row in rows {
        if !seen_admin_emails.insert(row.admin_email.clone()) {
            return Err(sqlx::Error::Configuration(
                format!(
                    "Duplicate seeded admin email or SUPER_ADMINS overlap: {}",
                    row.admin_email
                )
                .into(),
            ));
        }

        let team_result = sqlx::query(
            "INSERT INTO teams (name, admin_id, division, location, init_rank) VALUES (?, NULL, ?, ?, ?)"
        )
        .bind(&row.team_name)
        .bind(row.division)
        .bind(&row.location)
        .bind(row.init_rank)
        .execute(&mut *tx)
        .await?;

        let team_id = team_result.last_insert_rowid();

        for member in row.members {
            sqlx::query(
                "INSERT INTO users (name, common_name, dob, team_id, role, is_captain, is_spirit_captain) VALUES (?, ?, ?, ?, 2, 0, 0)"
            )
            .bind(&member.full_name)
            .bind(&member.common_name)
            .bind(&member.dob)
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
        }

        let admin_password = resolve_seed_password(
            existing_manifest.find_division_password(row.division, &row.admin_email, &row.team_name),
            &jwt_secret,
            &row.admin_email,
        )?;
        let admin_password_hash = hash_password(&admin_password)?;

        let admin_result = sqlx::query(
            "INSERT INTO users (name, common_name, email, phone, team_id, role, password_hash) VALUES (?, ?, ?, ?, ?, 3, ?)"
        )
        .bind(&row.admin_name)
        .bind(&row.admin_name)
        .bind(&row.admin_email)
        .bind(&row.admin_phone)
        .bind(team_id)
        .bind(&admin_password_hash)
        .execute(&mut *tx)
        .await?;

        let admin_id = admin_result.last_insert_rowid();

        password_manifest.push_division(
            row.division,
            row.admin_email.clone(),
            admin_password,
            row.team_name.clone(),
        );

        sqlx::query("UPDATE teams SET admin_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(admin_id)
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    write_password_manifest(&manifest_path, &password_manifest)?;
    Ok(())
}

fn load_existing_password_manifest(manifest_path: &Path) -> Result<PasswordManifest, sqlx::Error> {
    if !manifest_path.exists() {
        return Ok(PasswordManifest::default());
    }

    let raw = fs::read_to_string(manifest_path).map_err(|err| {
        sqlx::Error::Configuration(
            format!(
                "Failed to read password manifest at {}: {err}",
                manifest_path.display()
            )
            .into(),
        )
    })?;

    serde_json::from_str(&raw).map_err(|err| {
        sqlx::Error::Configuration(
            format!(
                "Failed to parse password manifest at {}: {err}",
                manifest_path.display()
            )
            .into(),
        )
    })
}

fn load_jwt_secret_for_passwords() -> Result<String, sqlx::Error> {
    match std::env::var("JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        _ => Err(sqlx::Error::Configuration(
            "JWT_SECRET is required to generate CSV seed passwords".into(),
        )),
    }
}

fn load_super_admin_emails() -> Result<Vec<String>, sqlx::Error> {
    let raw = std::env::var("SUPER_ADMINS").map_err(|_| {
        sqlx::Error::Configuration(
            "SUPER_ADMINS is required for CSV seeding and must be a JSON array of emails".into(),
        )
    })?;

    let parsed = parse_super_admin_list(&raw)?;

    let mut emails = Vec::new();
    let mut seen = HashSet::new();
    for value in parsed {
        let email = normalize_email(Some(&value)).ok_or_else(|| {
            sqlx::Error::Configuration(
                format!("Invalid email in SUPER_ADMINS: {value}").into(),
            )
        })?;

        if !seen.insert(email.clone()) {
            return Err(sqlx::Error::Configuration(
                format!("Duplicate email in SUPER_ADMINS: {email}").into(),
            ));
        }

        emails.push(email);
    }

    if emails.is_empty() {
        return Err(sqlx::Error::Configuration(
            "SUPER_ADMINS must contain at least one email".into(),
        ));
    }

    Ok(emails)
}

fn parse_super_admin_list(raw: &str) -> Result<Vec<String>, sqlx::Error> {
    let trimmed = raw.trim();

    for candidate in [
        trimmed,
        trim_matching_quotes(trimmed),
        trim_escaped_wrapping_quotes(trimmed),
    ] {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(candidate) {
            return Ok(parsed);
        }
    }

    let csv_like = trimmed
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .trim_matches('"')
                .trim_matches('\'')
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if !csv_like.is_empty() {
        return Ok(csv_like);
    }

    Err(sqlx::Error::Configuration(
        format!(
            "Failed to parse SUPER_ADMINS. Supported formats: JSON array or comma-separated emails. Received: {trimmed}"
        )
        .into(),
    ))
}

fn trim_matching_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return &value[1..value.len() - 1];
        }
    }

    value
}

fn trim_escaped_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 4 && value.starts_with("\\\"") && value.ends_with("\\\"") {
        return &value[2..value.len() - 2];
    }

    value
}

fn resolve_password_manifest_path(csv_path: &Path) -> PathBuf {
    csv_path
        .parent()
        .map(|path| path.join(PASSWORD_OUTPUT_FILE))
        .unwrap_or_else(|| PathBuf::from(PASSWORD_OUTPUT_FILE))
}

fn write_password_manifest(
    manifest_path: &Path,
    manifest: &PasswordManifest,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string_pretty(manifest).map_err(|err| {
        sqlx::Error::Configuration(
            format!("Failed to serialize password manifest: {err}").into(),
        )
    })?;

    fs::write(manifest_path, format!("{json}\n")).map_err(|err| {
        sqlx::Error::Configuration(
            format!(
                "Seeded database, but failed to write password manifest at {}: {err}",
                manifest_path.display()
            )
            .into(),
        )
    })
}

fn resolve_seed_password(
    existing_password: Option<String>,
    jwt_secret: &str,
    email: &str,
) -> Result<String, sqlx::Error> {
    match existing_password {
        Some(password) if !password.trim().is_empty() => Ok(password),
        _ => generate_seed_password(jwt_secret, email),
    }
}

fn generate_seed_password(jwt_secret: &str, email: &str) -> Result<String, sqlx::Error> {
    let jwt_chars = jwt_secret
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<Vec<_>>();

    if jwt_chars.is_empty() {
        return Err(sqlx::Error::Configuration(
            "JWT_SECRET must contain at least one ASCII letter or digit to generate passwords"
                .into(),
        ));
    }

    let mut rng = rand::rng();
    let mut base = String::with_capacity(PASSWORD_LENGTH);

    for _ in 0..JWT_SECRET_PASSWORD_CHARS {
        let index = rng.random_range(0..jwt_chars.len());
        base.push(jwt_chars[index]);
    }

    for _ in 0..RANDOM_PASSWORD_CHARS {
        let index = rng.random_range(0..PASSWORD_ALPHABET.len());
        base.push(PASSWORD_ALPHABET[index] as char);
    }

    let digest = Sha256::digest(format!("{email}:{base}:{jwt_secret}").as_bytes());
    let mut final_chars = (0..PASSWORD_LENGTH)
        .map(|idx| {
            let mixed = digest[idx] as usize
                ^ digest[(idx + PASSWORD_LENGTH) % digest.len()] as usize
                ^ base.as_bytes()[idx % base.len()] as usize;
            PASSWORD_ALPHABET[mixed % PASSWORD_ALPHABET.len()] as char
        })
        .collect::<Vec<_>>();

    enforce_password_class(
        &mut final_chars,
        |ch| ch.is_ascii_uppercase(),
        UPPERCASE_ALPHABET,
        digest[24] as usize,
        digest[25] as usize,
    );
    enforce_password_class(
        &mut final_chars,
        |ch| ch.is_ascii_lowercase(),
        LOWERCASE_ALPHABET,
        digest[26] as usize,
        digest[27] as usize,
    );
    enforce_password_class(
        &mut final_chars,
        |ch| ch.is_ascii_digit(),
        DIGIT_ALPHABET,
        digest[28] as usize,
        digest[29] as usize,
    );

    Ok(final_chars.into_iter().collect())
}

fn enforce_password_class<F>(
    chars: &mut [char],
    predicate: F,
    alphabet: &[u8],
    position_seed: usize,
    value_seed: usize,
) where
    F: Fn(char) -> bool,
{
    if chars.iter().copied().any(predicate) {
        return;
    }

    let position = position_seed % chars.len();
    chars[position] = alphabet[value_seed % alphabet.len()] as char;
}

fn load_team_rows(csv_path: &Path) -> Result<Vec<ImportedTeamRow>, sqlx::Error> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(csv_path)
        .map_err(|err| {
            sqlx::Error::Configuration(format!("Failed to open teams CSV: {err}").into())
        })?;

    let mut records = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|err| {
            sqlx::Error::Configuration(format!("Failed to parse teams CSV: {err}").into())
        })?;
        if is_team_record(&record) {
            records.push(record);
        }
    }

    let rows = records
        .into_iter()
        .map(|record| build_team_row(&record))
        .collect::<Result<Vec<_>, _>>()?;

    if matches_expected_team_counts(&rows) {
        return Ok(rows);
    }

    let skipped_rows = rows
        .into_iter()
        .skip(CSV_DATA_ROWS_TO_SKIP)
        .collect::<Vec<_>>();
    if matches_expected_team_counts(&skipped_rows) {
        return Ok(skipped_rows);
    }

    let (open_count, women_count) = count_divisions(&skipped_rows);
    Err(sqlx::Error::Configuration(
        format!(
            "teams.csv import expected {EXPECTED_OPEN_TEAMS} Open and {EXPECTED_WOMEN_TEAMS} Women teams, got {open_count} Open and {women_count} Women after checking current and legacy skip-first-{CSV_DATA_ROWS_TO_SKIP} layouts"
        )
        .into(),
    ))
}

fn matches_expected_team_counts(rows: &[ImportedTeamRow]) -> bool {
    let (open_count, women_count) = count_divisions(rows);
    open_count == EXPECTED_OPEN_TEAMS && women_count == EXPECTED_WOMEN_TEAMS
}

fn count_divisions(rows: &[ImportedTeamRow]) -> (usize, usize) {
    let open_count = rows.iter().filter(|row| row.division == 0).count();
    let women_count = rows.iter().filter(|row| row.division == 1).count();
    (open_count, women_count)
}

fn build_team_row(record: &StringRecord) -> Result<ImportedTeamRow, sqlx::Error> {
    let division = match normalize_text(record.get(2)).as_deref() {
        Some("open") => 0,
        Some("women") => 1,
        Some(other) => {
            return Err(sqlx::Error::Configuration(
                format!("Unsupported division in teams.csv: {other}").into(),
            ));
        }
        None => {
            return Err(sqlx::Error::Configuration(
                "Missing division in teams.csv".into(),
            ));
        }
    };

    let team_name = normalize_display_text(record.get(3))
        .ok_or_else(|| sqlx::Error::Configuration("Missing team name in teams.csv".into()))?;
    let init_rank = parse_init_rank(record.get(RANKING_COLUMN), &team_name)?;
    let admin_email = normalize_email(record.get(FORM_EMAIL_COLUMN))
        .or_else(|| normalize_email(record.get(CONTACT_EMAIL_COLUMN)))
        .ok_or_else(|| {
            sqlx::Error::Configuration(format!("Missing form email for team {team_name}").into())
        })?;
    let members = extract_members(record);
    let admin_name = normalize_person_name(record.get(ADMIN_NAME_COLUMN))
        .filter(|value| !looks_like_phone(value) && !looks_like_email(value))
        .or_else(|| members.first().map(|member| member.full_name.clone()))
        .unwrap_or_else(|| fallback_name_from_email(&admin_email));

    Ok(ImportedTeamRow {
        division,
        team_name,
        init_rank,
        location: combine_location(record.get(CITY_COLUMN), record.get(STATE_COLUMN)),
        admin_name,
        admin_email,
        admin_phone: normalize_phone(record.get(ADMIN_PHONE_PRIMARY_COLUMN))
            .or_else(|| normalize_phone(record.get(ADMIN_PHONE_FALLBACK_COLUMN))),
        members,
    })
}

fn parse_init_rank(value: Option<&str>, team_name: &str) -> Result<i64, sqlx::Error> {
    let value = normalize_display_text(value).ok_or_else(|| {
        sqlx::Error::Configuration(
            format!("Missing ranking for team {team_name} in teams.csv").into(),
        )
    })?;

    value.parse::<i64>().map_err(|err| {
        sqlx::Error::Configuration(
            format!("Invalid ranking '{value}' for team {team_name} in teams.csv: {err}").into(),
        )
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
    let normalized_common_name = normalize_person_name(common_name).filter(|value| value != "-");
    let full_name = normalize_person_name(full_name).or_else(|| normalized_common_name.clone());

    let Some(full_name) = full_name else {
        return;
    };

    let common_name =
        normalized_common_name.filter(|value| normalize_key(value) != normalize_key(&full_name));

    let key = normalize_key(&full_name);
    if key.is_empty() || seen_names.contains(&key) {
        return;
    }

    seen_names.insert(key);
    members.push(ImportedMember {
        full_name,
        common_name,
        dob: normalize_date(dob),
    });
}

async fn seed_default_staff_and_fields(
    tx: &mut Transaction<'_, Sqlite>,
    super_admin_emails: &[String],
    jwt_secret: &str,
    existing_manifest: &PasswordManifest,
    manifest: &mut PasswordManifest,
) -> Result<(), sqlx::Error> {
    for email in super_admin_emails {
        let password = resolve_seed_password(
            existing_manifest.find_super_password(email),
            jwt_secret,
            email,
        )?;
        let password_hash = hash_password(&password)?;
        let display_name = display_name_from_email(email);

        sqlx::query(
            "INSERT INTO users (name, common_name, email, role, password_hash) VALUES (?, ?, ?, 0, ?)",
        )
        .bind(&display_name)
        .bind(&display_name)
        .bind(email)
        .bind(&password_hash)
        .execute(&mut **tx)
        .await?;

        manifest.push_super(email.clone(), password);
    }

    sqlx::query(
        r#"INSERT INTO fields (name, hints, map_link) VALUES
        ('Ground 1', 'Main Astroturf Ground, Sports Complex, North Block', 'https://maps.google.com/?q=ground+1+sports+complex'),
        ('Ground 2', 'Secondary Astroturf, Sports Complex, East Block', 'https://maps.google.com/?q=ground+2+sports+complex'),
        ('Ground 3', 'Natural Grass Field, Sports Complex, South Block', 'https://maps.google.com/?q=ground+3+sports+complex'),
        ('Ground 4', 'Open Grass Field, Sports Complex, West Block', 'https://maps.google.com/?q=ground+4+sports+complex')"#,
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
        .replace(['.', '_'], " ")
}

fn display_name_from_email(email: &str) -> String {
    fallback_name_from_email(email)
        .split_whitespace()
        .map(capitalize_ascii_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_ascii_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}
