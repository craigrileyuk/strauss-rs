use std::env;
use std::fs;

mod analyser;
mod config;
mod php_pattern;

use php_rs_parser::parse;

fn main() {
    let filename = env::args().nth(1).expect("Usage: strauss-rs <php-file>");

    let source = fs::read_to_string(&filename).expect("Failed to read PHP file");

    let result = parse(&source);

    if !result.errors.is_empty() {
        for error in &result.errors {
            eprintln!("Parse error: {error}");
        }
    }

    let analysis = analyser::analyse(&source, &result.program);

    println!("{analysis:#?}");
}
