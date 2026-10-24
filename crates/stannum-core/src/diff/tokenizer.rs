//! RAW field tokenizer: keeps quoted content verbatim and stops at `/` comments.

/// Split one physical RAW line into fields.
///
/// Unquoted fields are trimmed; quoted fields keep their padding. A doubled
/// quote inside a quoted field is an escaped quote. Anything after an
/// unquoted `/` is a comment.
pub fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut buffer = String::new();
    let mut quote: Option<char> = None;
    let mut quoted = false;
    let mut closed = false;
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0;
    let finish = |buffer: &str, quoted: bool| {
        if quoted {
            buffer.to_string()
        } else {
            buffer.trim().to_string()
        }
    };
    while index < chars.len() {
        let ch = chars[index];
        if let Some(q) = quote {
            if ch == q {
                if chars.get(index + 1) == Some(&q) {
                    buffer.push(ch);
                    index += 1;
                } else {
                    quote = None;
                    closed = true;
                }
            } else {
                buffer.push(ch);
            }
        } else if ch == '/' {
            break;
        } else if ch == ',' {
            fields.push(finish(&buffer, quoted));
            buffer.clear();
            quoted = false;
            closed = false;
        } else if ch == '\'' || ch == '"' {
            if closed || !buffer.trim().is_empty() {
                return Err("Quote must start a field".into());
            }
            buffer.clear();
            quoted = true;
            quote = Some(ch);
        } else if closed {
            if !ch.is_whitespace() {
                return Err("Unexpected content after closing quote".into());
            }
        } else {
            buffer.push(ch);
        }
        index += 1;
    }
    if quote.is_some() {
        return Err("Unterminated quoted field".into());
    }
    let last = finish(&buffer, quoted);
    if !fields.is_empty() || quoted || !last.is_empty() {
        fields.push(last);
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::tokenize;

    fn ok(line: &str) -> Vec<String> {
        tokenize(line).unwrap()
    }

    #[test]
    fn quoted_punctuation() {
        assert_eq!(
            ok("1, 'a,b/c', \"d/e,f\" / ignored,9"),
            ["1", "a,b/c", "d/e,f"]
        );
    }

    #[test]
    fn preserves_quoted_padding() {
        assert_eq!(
            ok("  1 , ' ID  ' , \" padded \" "),
            ["1", " ID  ", " padded "]
        );
    }

    #[test]
    fn rejects_bad_quoting() {
        for line in [
            "1,'unfinished",
            "1,\"unfinished",
            "1,ab'cd'",
            "'abc'junk",
            "'abc'\"def\"",
        ] {
            assert!(tokenize(line).is_err(), "{line}");
        }
    }

    #[test]
    fn empty_fields_and_comments() {
        assert_eq!(ok(" 1, ,0, / comment"), ["1", "", "0", ""]);
        assert!(ok("/ only comment").is_empty());
        assert!(ok("  \n").is_empty());
        assert_eq!(ok("''"), [""]);
    }

    #[test]
    fn doubled_quotes() {
        assert_eq!(ok("'O''Brien',\"a\"\"b\""), ["O'Brien", "a\"b"]);
    }
}
