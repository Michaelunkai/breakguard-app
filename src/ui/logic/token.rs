#![allow(dead_code)]
//! Line tokenizer for log lines. std-only. Concatenated span texts == input.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tk {
    Plain,
    Path,
    Number,
    ErrorWord,
    Code,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Span {
    pub text: String,
    pub kind: Tk,
}

const ERROR_WORDS: [&str; 4] = ["error", "failed", "panic", "fatal"];

fn push(out: &mut Vec<Span>, text: &str, kind: Tk) {
    if text.is_empty() {
        return;
    }
    if kind == Tk::Plain {
        if let Some(last) = out.last_mut() {
            if last.kind == Tk::Plain {
                last.text.push_str(text);
                return;
            }
        }
    }
    out.push(Span { text: text.to_string(), kind });
}

fn is_path(tok: &str) -> bool {
    tok.chars().any(|c| c == '/' || c == '\\') && tok.chars().any(|c| c.is_alphabetic())
}

fn is_number(tok: &str) -> bool {
    tok.chars().any(|c| c.is_ascii_digit())
        && tok.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | ':'))
}

/// Split a non-whitespace, backtick-free token and push spans.
fn classify(tok: &str, out: &mut Vec<Span>) {
    if is_path(tok) {
        push(out, tok, Tk::Path);
    } else if is_number(tok) {
        push(out, tok, Tk::Number);
    } else {
        let start = tok.find(|c: char| c.is_alphanumeric()).unwrap_or(tok.len());
        let end = tok
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(start);
        let core = &tok[start..end];
        if !core.is_empty() && ERROR_WORDS.contains(&core.to_lowercase().as_str()) {
            push(out, &tok[..start], Tk::Plain);
            push(out, core, Tk::ErrorWord);
            push(out, &tok[end..], Tk::Plain);
        } else {
            push(out, tok, Tk::Plain);
        }
    }
}

pub fn tokenize(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let c = rest.chars().next().unwrap();
        if c == '`' {
            if let Some(rel) = rest[1..].find('`') {
                let end = 1 + rel + 1;
                push(&mut out, &rest[..end], Tk::Code);
                i += end;
            } else {
                push(&mut out, "`", Tk::Plain); // unmatched backtick
                i += 1;
            }
        } else if c.is_whitespace() {
            let n = rest.find(|ch: char| !ch.is_whitespace()).unwrap_or(rest.len());
            push(&mut out, &rest[..n], Tk::Plain);
            i += n;
        } else {
            let n = rest.find(|ch: char| ch.is_whitespace() || ch == '`').unwrap_or(rest.len());
            classify(&rest[..n], &mut out);
            i += n;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(v: &[Span]) -> String {
        v.iter().map(|s| s.text.as_str()).collect()
    }
    fn kinds(line: &str) -> Vec<(String, Tk)> {
        tokenize(line).into_iter().map(|s| (s.text, s.kind)).collect()
    }

    #[test]
    fn roundtrip_ten() {
        let inputs = [
            "",
            "   ",
            "plain text here",
            "error: F:/x/y.rs:12:3 failed",
            "  leading and trailing  ",
            "`code` and `unclosed",
            "שלום עולם error שגיאה 12:30",
            "C:\\Users\\a\\b.txt 1,234.56",
            "tab\tsep\r\nnl",
            "emoji 😀 `😀` /a/😀 PANIC!",
            "```",
            "``",
        ];
        for s in inputs {
            assert_eq!(join(&tokenize(s)), s, "input {:?}", s);
        }
    }

    #[test]
    fn path_kinds() {
        assert_eq!(kinds("F:/x/y.rs"), vec![("F:/x/y.rs".into(), Tk::Path)]);
        assert_eq!(kinds("C:\\a\\b")[0].1, Tk::Path);
        assert_eq!(kinds("a/b")[0].1, Tk::Path);
        assert_eq!(kinds("/")[0].1, Tk::Plain);
        assert_eq!(kinds("1/2")[0].1, Tk::Plain);
    }

    #[test]
    fn number_kinds() {
        for n in ["42", "3.14", "1,000", "12:30:05", "1,234.5"] {
            assert_eq!(kinds(n), vec![(n.to_string(), Tk::Number)]);
        }
        assert_eq!(kinds("...")[0].1, Tk::Plain);
        assert_eq!(kinds("12ab")[0].1, Tk::Plain);
    }

    #[test]
    fn error_words() {
        assert_eq!(kinds("ERROR")[0].1, Tk::ErrorWord);
        assert_eq!(kinds("Failed")[0].1, Tk::ErrorWord);
        assert_eq!(kinds("panic")[0].1, Tk::ErrorWord);
        assert_eq!(kinds("fatal")[0].1, Tk::ErrorWord);
        assert_eq!(
            kinds("error:"),
            vec![("error".into(), Tk::ErrorWord), (":".into(), Tk::Plain)]
        );
        assert_eq!(kinds("errors")[0].1, Tk::Plain);
        assert_eq!(kinds("terror")[0].1, Tk::Plain);
        assert_eq!(kinds("error-code")[0].1, Tk::Plain);
    }

    #[test]
    fn code_spans() {
        assert_eq!(
            kinds("run `a b error` now"),
            vec![
                ("run ".into(), Tk::Plain),
                ("`a b error`".into(), Tk::Code),
                (" now".into(), Tk::Plain)
            ]
        );
        assert_eq!(kinds("``"), vec![("``".into(), Tk::Code)]);
        assert_eq!(kinds("`open")[0].1, Tk::Plain);
        assert_eq!(kinds("x`y`z").len(), 3);
    }

    #[test]
    fn unicode_no_panic() {
        let s = "שגיאה: /tmp/קובץ.rs failed ‏١٢٣ `עברית` İstanbul ǅ\u{200f}";
        assert_eq!(join(&tokenize(s)), s);
        assert!(tokenize(s).iter().any(|x| x.kind == Tk::Path));
        assert!(tokenize(s).iter().any(|x| x.kind == Tk::ErrorWord));
        assert!(tokenize(s).iter().any(|x| x.kind == Tk::Code));
    }

    #[test]
    fn adversarial() {
        for s in ["`", "``` ``` `", "!!!error!!!", "\u{a0}\u{3000}x", "\\", "/\\/", "é`é", "a\0b"] {
            assert_eq!(join(&tokenize(s)), s);
        }
        assert_eq!(kinds("!!!error!!!")[1], ("error".into(), Tk::ErrorWord));
        assert!(tokenize("").is_empty());
        let long = "error ".repeat(10000);
        assert_eq!(join(&tokenize(&long)), long);
    }
}
