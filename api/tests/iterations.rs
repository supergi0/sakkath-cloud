pub mod tournament;

fn main() {
    if let Err(error) = tournament::iterations_main() {
        eprintln!("tournament iterations run failed: {error}");
        std::process::exit(1);
    }
}