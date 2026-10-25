mod app;
mod login;
fn main() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
    if let Err(error) = app::run() {
        eprintln!("wla_my_wonderland backend stopped: {error}");
        std::process::exit(1)
    }
}
