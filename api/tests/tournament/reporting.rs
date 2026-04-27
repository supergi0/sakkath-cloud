use super::config::RunConfig;
use super::harness::TestResult;
use super::model::{ScheduleMatchResponse, TournamentTracker};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub(crate) struct ReportWriter {
    config: RunConfig,
    matches_lines: Vec<String>,
    standings_lines: Vec<String>,
    tournament_lines: Vec<String>,
    errors_lines: Vec<String>,
}

impl ReportWriter {
    pub(crate) fn new(config: &RunConfig) -> TestResult<Self> {
        fs::create_dir_all(&config.log_dir)?;

        let writer = Self {
            config: config.clone(),
            matches_lines: vec![
                "# Tournament Match Log".to_string(),
                String::new(),
                format!("- Iteration: {}", config.itr),
                format!("- Seed: {}", config.seed),
                String::new(),
                "| Scenario | Division | Stage | Match ID | Fixture | Score | Notes |".to_string(),
                "| --- | --- | --- | ---: | --- | --- | --- |".to_string(),
            ],
            standings_lines: vec![
                "# Tournament Standings Snapshots".to_string(),
                String::new(),
                format!("- Iteration: {}", config.itr),
                format!("- Seed: {}", config.seed),
                String::new(),
            ],
            tournament_lines: vec![
                "# Tournament Test Run".to_string(),
                String::new(),
                format!("- Iteration: {}", config.itr),
                format!("- Seed: {}", config.seed),
                format!("- Report directory: {}", config.log_dir.display()),
                String::new(),
            ],
            errors_lines: vec![
                "# Tournament Test Errors".to_string(),
                String::new(),
                format!("- Iteration: {}", config.itr),
                format!("- Seed: {}", config.seed),
                format!("- Report directory: {}", config.log_dir.display()),
                String::new(),
            ],
        };
        writer.sync_files()?;
        Ok(writer)
    }

    pub(crate) fn run_banner(&mut self) -> TestResult {
        println!(
            "running tournament suite with itr={} seed={} logs={}",
            self.config.itr,
            self.config.seed,
            self.config.log_dir.display()
        );
        self.tournament_lines
            .push("## Run Configuration".to_string());
        self.tournament_lines.push(String::new());
        self.tournament_lines.push(format!(
            "This run uses a deterministic RNG seed derived from itr={}, so the same itr reproduces the same tournament outcomes.",
            self.config.itr
        ));
        self.tournament_lines.push(String::new());
        self.sync_files()
    }

    pub(crate) fn start_scenario(&mut self, scenario: &str, description: &str) -> TestResult {
        println!();
        println!("== {scenario} ==");
        self.tournament_lines.push(format!("## {scenario}"));
        self.tournament_lines.push(String::new());
        self.tournament_lines.push(description.to_string());
        self.tournament_lines.push(String::new());
        self.sync_files()
    }

    pub(crate) fn note(&mut self, line: impl Into<String>) -> TestResult {
        let line = line.into();
        println!("{line}");
        self.tournament_lines.push(format!("- {line}"));
        self.sync_files()
    }

    pub(crate) fn record_pairings(
        &mut self,
        scenario: &str,
        division: i64,
        stage: &str,
        matches: &[ScheduleMatchResponse],
        tracker: &TournamentTracker,
    ) -> TestResult {
        let pairings = matches
            .iter()
            .map(|schedule_match| {
                format!(
                    "{} vs {}",
                    tracker.team_name(schedule_match.t1_id),
                    tracker.team_name(schedule_match.t2_id)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.note(format!(
            "[{scenario}] {} {stage} pairings: {pairings}",
            division_label(division)
        ))
    }

    pub(crate) fn record_match_result(
        &mut self,
        scenario: &str,
        division: i64,
        stage: &str,
        match_id: i64,
        tracker: &TournamentTracker,
        notes: &str,
    ) -> TestResult {
        let state = tracker.match_state(match_id);
        let fixture = format!(
            "{} vs {}",
            tracker.team_name(state.t1_id),
            tracker.team_name(state.t2_id)
        );
        let score = format!("{}-{}", state.t1_score, state.t2_score);

        println!(
            "[{scenario}] {} {stage}: #{match_id} {fixture} {score} ({notes})",
            division_label(division)
        );

        self.matches_lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} |",
            escape_markdown(scenario),
            division_label(division),
            escape_markdown(stage),
            match_id,
            escape_markdown(&fixture),
            score,
            escape_markdown(notes),
        ));
        self.sync_files()
    }

    pub(crate) fn record_failure(&mut self, summary: &str, details: &str) -> TestResult {
        eprintln!("[tournament-tests] {summary}");
        eprintln!("{details}");
        eprintln!(
            "[tournament-tests] failure details written to {}",
            self.errors_path().display()
        );

        self.errors_lines.push("## Failure".to_string());
        self.errors_lines.push(String::new());
        self.errors_lines
            .push(format!("- Summary: {}", escape_markdown(summary)));
        self.errors_lines.push(String::new());
        self.errors_lines.push("```text".to_string());
        self.errors_lines
            .extend(details.lines().map(ToOwned::to_owned));
        self.errors_lines.push("```".to_string());
        self.errors_lines.push(String::new());

        self.tournament_lines.push("## Failure".to_string());
        self.tournament_lines.push(String::new());
        self.tournament_lines
            .push(format!("- {}", escape_markdown(summary)));
        self.tournament_lines.push(String::new());
        self.sync_files()
    }

    pub(crate) fn record_standings(
        &mut self,
        scenario: &str,
        division: i64,
        label: &str,
        tracker: &TournamentTracker,
    ) -> TestResult {
        let order: Vec<i64> = tracker
            .swiss_summary(division)
            .iter()
            .map(|metrics| metrics.team_id)
            .collect();
        self.record_ordered_standings(scenario, division, label, tracker, &order)
    }

    pub(crate) fn record_ordered_standings(
        &mut self,
        scenario: &str,
        division: i64,
        label: &str,
        tracker: &TournamentTracker,
        order: &[i64],
    ) -> TestResult {
        let standings_by_team: HashMap<i64, _> = tracker
            .display_summary_for_order(division, order)
            .into_iter()
            .map(|metrics| (metrics.team_id, metrics))
            .collect();
        let headline = order
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, team_id)| format!("{}. {}", index + 1, tracker.team_name(*team_id)))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "[{scenario}] {} {label}: {headline}",
            division_label(division)
        );

        self.standings_lines.push(format!(
            "## {scenario} - {} - {label}",
            division_label(division)
        ));
        self.standings_lines.push(String::new());
        self.standings_lines
            .push("| Rank | Team | W | L | D | PF | PA | Spirit |".to_string());
        self.standings_lines
            .push("| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |".to_string());
        for (index, team_id) in order.iter().enumerate() {
            let metrics = standings_by_team
                .get(team_id)
                .unwrap_or_else(|| panic!("missing standings metrics for team {}", team_id));
            self.standings_lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {:.2} |",
                index + 1,
                escape_markdown(tracker.team_name(*team_id)),
                metrics.wins,
                metrics.losses,
                metrics.draws,
                metrics.points_for,
                metrics.points_against,
                metrics.spirit_avg,
            ));
        }
        self.standings_lines.push(String::new());
        self.sync_files()
    }

    pub(crate) fn finish(mut self) -> TestResult {
        self.note(format!(
            "reports written to {}",
            self.config.log_dir.display()
        ))?;
        self.sync_files()
    }

    fn sync_files(&self) -> TestResult {
        fs::create_dir_all(&self.config.log_dir)?;
        fs::write(self.matches_path(), self.matches_lines.join("\n"))?;
        fs::write(self.standings_path(), self.standings_lines.join("\n"))?;
        fs::write(self.tournament_path(), self.tournament_lines.join("\n"))?;
        fs::write(self.errors_path(), self.errors_lines.join("\n"))?;
        Ok(())
    }

    fn matches_path(&self) -> PathBuf {
        self.config.log_dir.join("matches.md")
    }

    fn standings_path(&self) -> PathBuf {
        self.config.log_dir.join("standings.md")
    }

    fn tournament_path(&self) -> PathBuf {
        self.config.log_dir.join("tournament.md")
    }

    fn errors_path(&self) -> PathBuf {
        self.config.log_dir.join("errors.md")
    }
}

pub(crate) fn division_label(division: i64) -> &'static str {
    match division {
        0 => "Open",
        1 => "Women",
        _ => "Unknown",
    }
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|")
}
