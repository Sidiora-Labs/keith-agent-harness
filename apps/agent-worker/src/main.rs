fn main() {
    if matches!(std::env::args().nth(1).as_deref(), Some("--version" | "-V")) {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Err(error) = keith_worker_runtime::run_from_environment() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
