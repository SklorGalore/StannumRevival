//! A `.stnrun` run file: parsed, then written back out. What Stannum
//! writes must read back as the same steps.
#![no_main]

use libfuzzer_sys::fuzz_target;
use stannum_core::run_file::{self, Step};
use std::path::Path;

/// What a written run file keeps: no line numbers, `case` or empty
/// arguments. An empty `settings` object means the defaults, as no
/// `settings` does, so a writer may leave it out.
fn kept(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.line = 0;
            s.args.retain(|k, v| {
                k != "case"
                    && !v.is_null()
                    && !(k == "settings" && v.as_object().is_some_and(|m| m.is_empty()))
            });
            s
        })
        .collect()
}

fuzz_target!(|bytes: &[u8]| {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let Ok(mut steps) = run_file::parse(text) else {
        return;
    };
    let written = run_file::format(&steps, None, "fuzz").expect("parsed steps can be written");
    let again = run_file::parse(&written).unwrap_or_else(|e| {
        panic!(
            "a written run file can't be read back: {}\n{written}",
            e.message
        )
    });
    assert_eq!(kept(&steps), kept(&again), "\n{written}");
    run_file::resolve_paths(&mut steps, Path::new("/cases"));
    let _ = run_file::format(&steps, Some(Path::new("/cases")), "fuzz");
});
