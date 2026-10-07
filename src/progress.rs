//! Progress shown on stderr while the long steps run.
//!
//! Off unless [`enable`] is called, which the CLI does only when stderr is a
//! terminal and `--quiet` isn't given, so output that is piped or logged stays
//! as plain lines and the library code and its tests never draw anything.

use std::sync::OnceLock;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

static ENABLED: OnceLock<()> = OnceLock::new();

pub fn enable() {
    let _ = ENABLED.set(());
}

fn enabled() -> bool {
    ENABLED.get().is_some()
}

/// One step of the run. Drawn while it exists, and cleared when it is dropped.
pub struct Stage(Option<ProgressBar>);

/// A step that works through `total` items, with a bar. Call [`Stage::tick`]
/// as each is done, from any thread.
pub fn stage(label: &'static str, total: usize) -> Stage {
    if !enabled() {
        return Stage(None);
    }

    let bar = ProgressBar::with_draw_target(Some(total as u64), ProgressDrawTarget::stderr());

    bar.set_style(
        ProgressStyle::with_template(
            "{prefix:>12.bold} [{bar:30.cyan/blue}] {pos}/{len} {elapsed}",
        )
        .expect("the template is valid")
        .progress_chars("=> "),
    );
    bar.set_prefix(label);

    Stage(Some(bar))
}

/// A step of unknown length, shown as a spinner.
pub fn spinner(label: &'static str) -> Stage {
    if !enabled() {
        return Stage(None);
    }

    let bar = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr());

    bar.set_style(
        ProgressStyle::with_template("{prefix:>12.bold} {spinner} {elapsed}")
            .expect("the template is valid"),
    );
    bar.set_prefix(label);
    bar.enable_steady_tick(Duration::from_millis(100));

    Stage(Some(bar))
}

impl Stage {
    pub fn tick(&self) {
        if let Some(bar) = &self.0 {
            bar.inc(1);
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(bar) = &self.0 {
            bar.finish_and_clear();
        }
    }
}
