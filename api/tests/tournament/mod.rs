mod assertions;
mod config;
mod harness;
mod iterations_runner;
mod model;
mod reporting;
mod scenarios;
mod simulation;
mod support;

use self::config::RunConfig;
use self::harness::TestResult;
use self::reporting::ReportWriter;
use std::backtrace::Backtrace;
use std::fs;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};

pub fn cli_main() -> TestResult {
    let config = RunConfig::from_process_args(std::env::args().skip(1))?;
    install_panic_hook(&config);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let config_clone = config.clone();
    match panic::catch_unwind(AssertUnwindSafe(move || {
        let config = config_clone;
        runtime.block_on(async move {
            let mut reporter = ReportWriter::new(&config)?;
            reporter.run_banner()?;

            match scenarios::run_all(&config, &mut reporter).await {
                Ok(()) => reporter.finish(),
                Err(error) => {
                    let details = error.to_string();
                    let _ = reporter.record_failure("scenario returned an error", &details);
                    Err(error)
                }
            }
        })
    })) {
        Ok(result) => result,
        Err(_) => Err(format!(
            "tournament test run panicked; see {}",
            config.log_dir.join("errors.md").display()
        )
        .into()),
    }
}

pub fn iterations_main() -> TestResult {
    iterations_runner::cli_main()
}

fn install_panic_hook(config: &RunConfig) {
    let log_dir = config.log_dir.clone();
    let itr = config.itr;
    let seed = config.seed;
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        let message = panic_payload(info);
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        let artifact = format!(
            "# Tournament Test Errors\n\n- Iteration: {itr}\n- Seed: {seed}\n- Report directory: {}\n\n## Panic\n\n- Location: {}\n- Message: {}\n\n```text\n{:?}\n```\n",
            log_dir.display(),
            location,
            message,
            Backtrace::force_capture()
        );

        let path = log_dir.join("errors.md");
        let _ = fs::create_dir_all(&log_dir);
        let _ = fs::write(&path, artifact);
        eprintln!(
            "[tournament-tests] panic captured, wrote {}",
            path.display()
        );

        default_hook(info);
    }));
}

fn panic_payload(info: &PanicHookInfo<'_>) -> String {
    if let Some(message) = info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
