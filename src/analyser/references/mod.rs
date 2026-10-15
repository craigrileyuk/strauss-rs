//! Collecting the references a file makes to classes, functions and constants.

mod classes;
mod expressions;
mod names;
mod signatures;
mod statements;

pub(super) use statements::collect_stmt;
