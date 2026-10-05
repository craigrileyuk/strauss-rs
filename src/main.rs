mod analyser;
mod cli;
mod config;
mod packages;
mod parallel;
mod php_pattern;
mod rename;
mod symbols;

fn main() -> std::process::ExitCode {
    cli::run()
}
