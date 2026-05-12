use super::config::RunConfig;
use super::harness::TestResult;
use super::reporting::ReportWriter;
use super::scenarios::run_full_tournament_with_summary;
use std::collections::HashMap;
use std::fs;

#[derive(Default)]
struct BoundaryAggregate {
    total_runs: u64,
    same_point_cases: u64,
    same_point_met: u64,
    same_point_missed: u64,
    points_split_runs: u64,
}

#[derive(Default)]
struct LookaheadAggregate {
    runs: u64,
    explored_candidates_total: u64,
    kept_candidates_total: u64,
    total_simulations_run: u64,
    generation_timing: TimingAggregate,
    exploration_timing: TimingAggregate,
    simulation_timing: TimingAggregate,
    chosen_worst_case_probability_total: f64,
    chosen_worst_case_probability_samples: u64,
    best_kept_worst_case_probability_total: f64,
    best_kept_worst_case_probability_samples: u64,
    average_kept_worst_case_probability_total: f64,
    average_kept_worst_case_probability_samples: u64,
}

#[derive(Default)]
struct TimingAggregate {
    runs: u64,
    total_ms: f64,
    best_ms: Option<f64>,
    worst_ms: Option<f64>,
}

impl TimingAggregate {
    fn record(&mut self, value_ms: f64) {
        self.runs += 1;
        self.total_ms += value_ms;
        self.best_ms = Some(match self.best_ms {
            Some(current) => current.min(value_ms),
            None => value_ms,
        });
        self.worst_ms = Some(match self.worst_ms {
            Some(current) => current.max(value_ms),
            None => value_ms,
        });
    }
}

#[derive(Default)]
struct DivisionAggregate {
    swiss_rematches_total: u64,
    tournament_rematches_total: u64,
    boundary_stats: HashMap<&'static str, BoundaryAggregate>,
    lookahead_by_round: HashMap<i64, LookaheadAggregate>,
    game_gap_minutes: Vec<i64>,
    ground_distribution_score_total: f64,
    rank_points: Vec<Vec<i64>>,
    team_rematch_total: u64,
    team_sample_total: u64,
    max_team_rematches: usize,
}

#[derive(Default)]
struct IterationAggregate {
    division_aggregates: HashMap<i64, DivisionAggregate>,
    tournament_ground_distribution_score_total: f64,
}

pub(crate) fn cli_main() -> TestResult {
    let iterations = parse_iterations_args(std::env::args().skip(1))?;
    let summary_dir = RunConfig::repo_root()
        .join("logs")
        .join(format!("test_tournament_iterations_n{iterations}"));
    fs::create_dir_all(&summary_dir)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let mut aggregate = IterationAggregate::default();

        for itr in 1..=iterations {
            let config = RunConfig::from_itr(itr)?;
            let mut reporter = ReportWriter::new(&config)?;
            reporter.run_banner()?;

            match run_full_tournament_with_summary(&config, &mut reporter).await {
                Ok(summary) => {
                    reporter.finish()?;
                    aggregate.record(&summary);
                }
                Err(error) => {
                    let details = error.to_string();
                    let _ = reporter.record_failure(
                        "full-tournament iterations run returned an error",
                        &details,
                    );
                    return Err(error);
                }
            }
        }

        let summary_path = summary_dir.join("summary.md");
        fs::write(&summary_path, aggregate.render_markdown(iterations))?;
        println!(
            "wrote tournament iterations summary to {}",
            summary_path.display()
        );

        Ok(())
    })
}

impl IterationAggregate {
    fn record(&mut self, summary: &super::scenarios::FullTournamentSummary) {
        self.tournament_ground_distribution_score_total += summary.ground_distribution_score;

        for division_summary in &summary.divisions {
            let aggregate = self
                .division_aggregates
                .entry(division_summary.division)
                .or_default();

            aggregate.swiss_rematches_total += division_summary.swiss_rematches as u64;
            aggregate.tournament_rematches_total += division_summary.tournament_rematches as u64;

            for boundary_meeting in &division_summary.boundary_meetings {
                let boundary = aggregate
                    .boundary_stats
                    .entry(boundary_meeting.label)
                    .or_default();
                boundary.total_runs += 1;
                if boundary_meeting.same_points {
                    boundary.same_point_cases += 1;
                    if boundary_meeting.met {
                        boundary.same_point_met += 1;
                    } else {
                        boundary.same_point_missed += 1;
                    }
                } else {
                    boundary.points_split_runs += 1;
                }
            }

            for lookahead_round in &division_summary.lookahead_rounds {
                let lookahead = aggregate
                    .lookahead_by_round
                    .entry(lookahead_round.round)
                    .or_default();
                lookahead.runs += 1;
                lookahead.explored_candidates_total +=
                    lookahead_round.diagnostics.explored_candidates as u64;
                lookahead.kept_candidates_total +=
                    lookahead_round.diagnostics.kept_candidates as u64;
                lookahead.total_simulations_run +=
                    lookahead_round.diagnostics.total_simulations_run as u64;
                lookahead
                    .generation_timing
                    .record(lookahead_round.diagnostics.generation_time_ms);
                lookahead
                    .exploration_timing
                    .record(lookahead_round.diagnostics.exploration_time_ms);
                lookahead
                    .simulation_timing
                    .record(lookahead_round.diagnostics.simulation_time_ms);
                if let Some(probability) = lookahead_round
                    .diagnostics
                    .chosen_worst_case_same_point_miss_probability
                {
                    lookahead.chosen_worst_case_probability_total += probability;
                    lookahead.chosen_worst_case_probability_samples += 1;
                }
                if let Some(probability) = lookahead_round
                    .diagnostics
                    .best_kept_worst_case_same_point_miss_probability
                {
                    lookahead.best_kept_worst_case_probability_total += probability;
                    lookahead.best_kept_worst_case_probability_samples += 1;
                }
                if let Some(probability) = lookahead_round
                    .diagnostics
                    .average_kept_worst_case_same_point_miss_probability
                {
                    lookahead.average_kept_worst_case_probability_total += probability;
                    lookahead.average_kept_worst_case_probability_samples += 1;
                }
            }

            aggregate
                .game_gap_minutes
                .extend(division_summary.game_gap_minutes.iter().copied());
            aggregate.ground_distribution_score_total += division_summary.ground_distribution_score;

            if aggregate.rank_points.len() < division_summary.final_swiss_points_by_rank.len() {
                aggregate
                    .rank_points
                    .resize_with(division_summary.final_swiss_points_by_rank.len(), Vec::new);
            }
            for (index, points) in division_summary
                .final_swiss_points_by_rank
                .iter()
                .enumerate()
            {
                aggregate.rank_points[index].push(*points);
            }

            aggregate.team_rematch_total += division_summary
                .team_tournament_rematches
                .iter()
                .map(|(_, count)| *count as u64)
                .sum::<u64>();
            aggregate.team_sample_total += division_summary.final_swiss_points_by_rank.len() as u64;
            aggregate.max_team_rematches = aggregate.max_team_rematches.max(
                division_summary
                    .team_tournament_rematches
                    .iter()
                    .map(|(_, count)| *count)
                    .max()
                    .unwrap_or(0),
            );
        }
    }

    fn render_markdown(&self, iterations: u64) -> String {
        let mut lines = vec![
            "# Tournament Iterations Summary".to_string(),
            String::new(),
            format!("- Iterations: {iterations}"),
            format!("- Aggregate seeds: itr=1..={iterations}"),
            String::new(),
            "## Rematch Summary".to_string(),
            String::new(),
            "| Scope | Avg Swiss Rematches / Run | Avg Tournament Rematches / Run | Avg Tournament Rematches / Team | Max Single-Team Tournament Rematches |".to_string(),
            "| --- | ---: | ---: | ---: | ---: |".to_string(),
        ];

        let all_swiss: u64 = self
            .division_aggregates
            .values()
            .map(|aggregate| aggregate.swiss_rematches_total)
            .sum();
        let all_tournament: u64 = self
            .division_aggregates
            .values()
            .map(|aggregate| aggregate.tournament_rematches_total)
            .sum();
        let all_team_rematches: u64 = self
            .division_aggregates
            .values()
            .map(|aggregate| aggregate.team_rematch_total)
            .sum();
        let all_team_samples: u64 = self
            .division_aggregates
            .values()
            .map(|aggregate| aggregate.team_sample_total)
            .sum();
        let all_team_max = self
            .division_aggregates
            .values()
            .map(|aggregate| aggregate.max_team_rematches)
            .max()
            .unwrap_or(0);

        lines.push(format!(
            "| All divisions | {:.2} | {:.2} | {:.3} | {} |",
            average(all_swiss, iterations),
            average(all_tournament, iterations),
            average(all_team_rematches, all_team_samples),
            all_team_max,
        ));

        for division in [0, 1] {
            if let Some(aggregate) = self.division_aggregates.get(&division) {
                lines.push(format!(
                    "| {} | {:.2} | {:.2} | {:.3} | {} |",
                    division_label(division),
                    average(aggregate.swiss_rematches_total, iterations),
                    average(aggregate.tournament_rematches_total, iterations),
                    average(aggregate.team_rematch_total, aggregate.team_sample_total),
                    aggregate.max_team_rematches,
                ));
            }
        }

        lines.push(String::new());
        lines.push("## Ground Distribution Randomness".to_string());
        lines.push(String::new());
        lines.push("| Scope | Mean MSD Score |".to_string());
        lines.push("| --- | ---: |".to_string());
        lines.push(format!(
            "| All divisions | {:.4} |",
            self.tournament_ground_distribution_score_total / iterations as f64,
        ));
        for division in [0, 1] {
            let Some(aggregate) = self.division_aggregates.get(&division) else {
                continue;
            };
            lines.push(format!(
                "| {} | {:.4} |",
                division_label(division),
                aggregate.ground_distribution_score_total / iterations as f64,
            ));
        }

        lines.push(String::new());
        lines.push("## Game Gap Summary".to_string());
        lines.push(String::new());
        lines.push("| Scope | Mean Gap | Median Gap | Lowest Gap | Highest Gap |".to_string());
        lines.push("| --- | ---: | ---: | ---: | ---: |".to_string());
        for division in [0, 1] {
            let Some(aggregate) = self.division_aggregates.get(&division) else {
                continue;
            };
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                division_label(division),
                format_gap_value(Some(mean_i64(&aggregate.game_gap_minutes))),
                format_gap_value(Some(median(&aggregate.game_gap_minutes))),
                format_gap_value(lowest_gap(&aggregate.game_gap_minutes)),
                format_gap_value(highest_gap(&aggregate.game_gap_minutes)),
            ));
        }

        for division in [0, 1] {
            let Some(aggregate) = self.division_aggregates.get(&division) else {
                continue;
            };

            lines.push(String::new());
            lines.push(format!("## {} Boundary Outcomes", division_label(division)));
            lines.push(String::new());
            lines.push("| Final Boundary Pair | Same-Point Miss | Same-Point Met | Split-Point | Worst-Case Probability |".to_string());
            lines.push("| --- | ---: | ---: | ---: | ---: |".to_string());
            for label in ["4v5", "3v6", "8v9", "7v10"] {
                if let Some(boundary) = aggregate.boundary_stats.get(label) {
                    lines.push(format!(
                        "| {} | {} | {} | {} | {} |",
                        label,
                        format_count_over_total(boundary.same_point_missed, boundary.total_runs),
                        format_count_over_total(boundary.same_point_met, boundary.total_runs),
                        format_count_over_total(boundary.points_split_runs, boundary.total_runs),
                        format_probability(boundary.same_point_missed, boundary.total_runs),
                    ));
                }
            }

            if !aggregate.lookahead_by_round.is_empty() {
                lines.push(String::new());
                lines.push(format!(
                    "## {} Round 4/5/6 Generation Timing",
                    division_label(division)
                ));
                lines.push(String::new());
                lines.push("| Round | Avg Total | Best Total | Worst Total | Avg Explore | Best Explore | Worst Explore | Avg Sim | Best Sim | Worst Sim |".to_string());
                lines.push(
                    "| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
                        .to_string(),
                );
                for round in [4_i64, 5_i64, 6_i64] {
                    let Some(lookahead) = aggregate.lookahead_by_round.get(&round) else {
                        continue;
                    };
                    lines.push(format!(
                        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                        round,
                        format_mean_duration(&lookahead.generation_timing),
                        format_duration_value(lookahead.generation_timing.best_ms),
                        format_duration_value(lookahead.generation_timing.worst_ms),
                        format_mean_duration(&lookahead.exploration_timing),
                        format_duration_value(lookahead.exploration_timing.best_ms),
                        format_duration_value(lookahead.exploration_timing.worst_ms),
                        format_mean_duration(&lookahead.simulation_timing),
                        format_duration_value(lookahead.simulation_timing.best_ms),
                        format_duration_value(lookahead.simulation_timing.worst_ms),
                    ));
                }
            }

            if !aggregate.lookahead_by_round.is_empty() {
                lines.push(String::new());
                lines.push(format!(
                    "## {} Round 5/6 Lookahead Diagnostics",
                    division_label(division)
                ));
                lines.push(String::new());
                lines.push("| Round | Avg Explored Candidates | Avg Kept Candidates | Avg Simulations Run | Chosen Worst-Case Miss | Best Kept Worst-Case Miss | Mean Kept Worst-Case Miss |".to_string());
                lines.push("| ---: | ---: | ---: | ---: | ---: | ---: | ---: |".to_string());
                for round in [5_i64, 6_i64] {
                    let Some(lookahead) = aggregate.lookahead_by_round.get(&round) else {
                        continue;
                    };
                    lines.push(format!(
                        "| {} | {:.2} | {:.2} | {:.2} | {} | {} | {} |",
                        round,
                        average(lookahead.explored_candidates_total, lookahead.runs),
                        average(lookahead.kept_candidates_total, lookahead.runs),
                        average(lookahead.total_simulations_run, lookahead.runs),
                        format_mean_probability(
                            lookahead.chosen_worst_case_probability_total,
                            lookahead.chosen_worst_case_probability_samples,
                        ),
                        format_mean_probability(
                            lookahead.best_kept_worst_case_probability_total,
                            lookahead.best_kept_worst_case_probability_samples,
                        ),
                        format_mean_probability(
                            lookahead.average_kept_worst_case_probability_total,
                            lookahead.average_kept_worst_case_probability_samples,
                        ),
                    ));
                }
            }

            lines.push(String::new());
            lines.push(format!(
                "## {} Final Swiss Points By Rank",
                division_label(division)
            ));
            lines.push(String::new());
            lines.push("| Rank | Mean Points | Median Points |".to_string());
            lines.push("| ---: | ---: | ---: |".to_string());
            for (index, samples) in aggregate.rank_points.iter().enumerate() {
                lines.push(format!(
                    "| {} | {:.2} | {:.2} |",
                    index + 1,
                    mean(samples),
                    median(samples),
                ));
            }
        }

        lines.push(String::new());
        lines.join("\n")
    }
}

fn parse_iterations_args<I>(args: I) -> TestResult<u64>
where
    I: IntoIterator<Item = String>,
{
    let mut iterations = 100u64;
    let mut args = args.into_iter().peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--itr" | "itr" | "--iterations" | "iterations" | "-n" | "-N" | "--N" | "N" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value after itr".to_string())?;
                iterations = parse_positive_u64(&value, "itr")?;
            }
            "--nocapture" | "--show-output" => {}
            "--color" => {
                let _ = args.next();
            }
            _ if arg.starts_with("--itr=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("--itr="), "itr")?;
            }
            _ if arg.starts_with("itr=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("itr="), "itr")?;
            }
            _ if arg.starts_with("--iterations=") => {
                iterations =
                    parse_positive_u64(arg.trim_start_matches("--iterations="), "iterations")?;
            }
            _ if arg.starts_with("iterations=") => {
                iterations =
                    parse_positive_u64(arg.trim_start_matches("iterations="), "iterations")?;
            }
            _ if arg.starts_with("--N=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("--N="), "iterations")?;
            }
            _ if arg.starts_with("N=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("N="), "iterations")?;
            }
            _ if arg.starts_with("-n=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("-n="), "iterations")?;
            }
            _ if arg.starts_with("-N=") => {
                iterations = parse_positive_u64(arg.trim_start_matches("-N="), "iterations")?;
            }
            _ => {}
        }
    }

    Ok(iterations)
}

fn parse_positive_u64(value: &str, label: &str) -> TestResult<u64> {
    let parsed = value.parse::<u64>()?;
    if parsed == 0 {
        return Err(format!("{label} must be greater than zero").into());
    }
    Ok(parsed)
}

fn average(total: u64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        total as f64 / count as f64
    }
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 * 100.0) / total as f64
    }
}

fn format_count_over_total(part: u64, total: u64) -> String {
    format!("{part}/{total}")
}

fn format_probability(part: u64, total: u64) -> String {
    if total == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1}%", percentage(part, total))
    }
}

fn format_mean_probability(total: f64, samples: u64) -> String {
    if samples == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1}%", total * 100.0 / samples as f64)
    }
}

fn format_mean_duration(timing: &TimingAggregate) -> String {
    if timing.runs == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1}ms", timing.total_ms / timing.runs as f64)
    }
}

fn format_duration_value(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1}ms"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_gap_value(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.1} min"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn lowest_gap(values: &[i64]) -> Option<f64> {
    values.iter().min().map(|value| *value as f64)
}

fn highest_gap(values: &[i64]) -> Option<f64> {
    values.iter().max().map(|value| *value as f64)
}

fn mean_i64(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

fn mean(values: &[i64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<i64>() as f64 / values.len() as f64
    }
}

fn median(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    } else {
        sorted[mid] as f64
    }
}

fn division_label(division: i64) -> &'static str {
    if division == 0 { "Open" } else { "Women" }
}
