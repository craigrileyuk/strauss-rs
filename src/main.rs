mod analyser;
mod cli;
mod config;
mod packages;
mod php_pattern;

fn main() -> std::process::ExitCode {
    cli::run()
}
