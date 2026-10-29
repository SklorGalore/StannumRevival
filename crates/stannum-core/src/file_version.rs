//! Version lines for Stannum's text files (run files and bus coordinates).
//!
//! A text file says which version of its format it follows with a comment
//! line before anything else in it:
//!
//! ```text
//! # stannum-run-file version 1
//! ```
//!
//! The line is a comment, so a Stannum that predates it reads the file as
//! before. A file without one is version 1. A file of a newer version than
//! this Stannum reads is refused, with a message saying so, rather than read
//! wrongly.

/// The version a file declares in a `# <tag> version N` comment among its
/// first comment lines, with the line it is on, or None when it declares
/// none. Only the comments and blank lines before the first other line are
/// looked at. The tag is matched without regard to case.
pub fn declared(text: &str, tag: &str) -> Result<Option<(usize, u32)>, String> {
    let tag = tag.to_ascii_lowercase();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        let Some(comment) = line.strip_prefix('#') else {
            break;
        };
        let mut words = comment.split_whitespace();
        if words.next().map(str::to_ascii_lowercase).as_deref() != Some(tag.as_str()) {
            continue;
        }
        if words.next().map(str::to_ascii_lowercase).as_deref() != Some("version") {
            continue;
        }
        let number = words.next().unwrap_or("");
        return match number.parse::<u32>() {
            Ok(n) if n >= 1 && words.next().is_none() => Ok(Some((i + 1, n))),
            _ => Err(format!(
                "Line {}: the {tag} version should be a whole number such as 1, not \"{}\".",
                i + 1,
                comment.trim()
            )),
        };
    }
    Ok(None)
}

/// Checks a file's declared version against the newest this Stannum reads.
/// `what` names the format in the message, such as "run file".
pub fn check(text: &str, tag: &str, newest: u32, what: &str) -> Result<u32, String> {
    match declared(text, tag)? {
        None => Ok(1),
        Some((_, n)) if n <= newest => Ok(n),
        Some((line, n)) => Err(format!(
            "Line {line}: this {what} is version {n}, from a newer Stannum. \
             This Stannum reads up to version {newest}; update Stannum to use it."
        )),
    }
}

/// The comment line that declares a version, without a line break.
pub fn line(tag: &str, version: u32) -> String {
    format!("# {tag} version {version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAG: &str = "stannum-run-file";

    #[test]
    fn reads_the_version_comment() {
        assert_eq!(declared("open_case path=a.m\n", TAG), Ok(None));
        assert_eq!(
            declared("# stannum-run-file version 1\nx\n", TAG),
            Ok(Some((1, 1)))
        );
        assert_eq!(
            declared(
                "\u{feff}# a heading\n\n#Stannum-Run-File  VERSION 3\nx\n",
                TAG
            ),
            Ok(Some((3, 3)))
        );
        // After the first other line it is just a comment.
        assert_eq!(declared("x\n# stannum-run-file version 9\n", TAG), Ok(None));
        // Another format's tag is just a comment.
        assert_eq!(declared("# stannum-coordinates version 9\n", TAG), Ok(None));
        assert!(declared("# stannum-run-file version one\n", TAG).is_err());
        assert!(declared("# stannum-run-file version 0\n", TAG).is_err());
    }

    #[test]
    fn newer_versions_are_refused() {
        assert_eq!(check("x\n", TAG, 1, "run file"), Ok(1));
        assert_eq!(check(&line(TAG, 1), TAG, 1, "run file"), Ok(1));
        let e = check("# stannum-run-file version 2\n", TAG, 1, "run file").unwrap_err();
        assert_eq!(
            e,
            "Line 1: this run file is version 2, from a newer Stannum. \
             This Stannum reads up to version 1; update Stannum to use it."
        );
    }
}
