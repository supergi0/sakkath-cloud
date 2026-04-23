pub mod tournament;

fn main() {
    if let Err(error) = tournament::cli_main() {
        eprintln!("tournament test run failed: {error}");
        std::process::exit(1);
    }
}