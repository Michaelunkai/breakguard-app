#![allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    OpenFolder(String),
    CopyText(String),
    None,
}

pub trait Launcher {
    fn open(&mut self, p: &str);
    fn copy(&mut self, t: &str);
}

pub fn open_folder_action(cwd: &str) -> Action {
    let t = cwd.trim();
    if t.is_empty() {
        return Action::None;
    }
    let s = t.replace('/', "\\");
    let stripped = s.trim_end_matches('\\');
    if stripped.is_empty() {
        return Action::OpenFolder("\\".to_string());
    }
    let b = stripped.as_bytes();
    if b.len() == 2 && b[1] == b':' && b[0].is_ascii_alphabetic() && s.len() > 2 {
        return Action::OpenFolder(format!("{}\\", stripped));
    }
    Action::OpenFolder(stripped.to_string())
}

pub fn copy_id_action(id: &str) -> Action {
    if id.is_empty() {
        Action::None
    } else {
        Action::CopyText(id.to_string())
    }
}

pub fn run(a: &Action, l: &mut dyn Launcher) {
    match a {
        Action::OpenFolder(p) => l.open(p),
        Action::CopyText(t) => l.copy(t),
        Action::None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        calls: Vec<String>,
    }
    impl Launcher for Rec {
        fn open(&mut self, p: &str) {
            self.calls.push(format!("open:{}", p));
        }
        fn copy(&mut self, t: &str) {
            self.calls.push(format!("copy:{}", t));
        }
    }

    fn o(s: &str) -> Action {
        Action::OpenFolder(s.to_string())
    }

    #[test]
    fn blank_is_none() {
        assert_eq!(open_folder_action(""), Action::None);
        assert_eq!(open_folder_action("  \t\n "), Action::None);
    }
    #[test]
    fn trims_and_converts() {
        assert_eq!(open_folder_action("  C:/a/b  "), o("C:\\a\\b"));
        assert_eq!(open_folder_action("C:\\a/b\\c"), o("C:\\a\\b\\c"));
    }
    #[test]
    fn strips_trailing() {
        assert_eq!(open_folder_action("C:\\a\\"), o("C:\\a"));
        assert_eq!(open_folder_action("C:/a///"), o("C:\\a"));
        assert_eq!(open_folder_action("C:\\a\\\\ "), o("C:\\a"));
    }
    #[test]
    fn drive_root_kept() {
        assert_eq!(open_folder_action("C:\\"), o("C:\\"));
        assert_eq!(open_folder_action("C:/"), o("C:\\"));
        assert_eq!(open_folder_action("d:///"), o("d:\\"));
    }
    #[test]
    fn bare_drive_unchanged() {
        assert_eq!(open_folder_action("C:"), o("C:"));
    }
    #[test]
    fn only_separators() {
        assert_eq!(open_folder_action("///"), o("\\"));
        assert_eq!(open_folder_action("\\"), o("\\"));
    }
    #[test]
    fn unc_and_spaces() {
        assert_eq!(open_folder_action("//srv/share/"), o("\\\\srv\\share"));
        assert_eq!(open_folder_action("C:/My Docs/"), o("C:\\My Docs"));
    }
    #[test]
    fn idempotent() {
        for s in ["C:/a/b/", "C:\\", "///", "//s/x/", " x "] {
            let first = open_folder_action(s);
            if let Action::OpenFolder(p) = &first {
                assert_eq!(open_folder_action(p), first);
            }
        }
    }
    #[test]
    fn copy_id() {
        assert_eq!(copy_id_action(""), Action::None);
        assert_eq!(copy_id_action("abc"), Action::CopyText("abc".into()));
        assert_eq!(copy_id_action(" "), Action::CopyText(" ".into()));
    }
    #[test]
    fn run_dispatch() {
        let mut r = Rec::default();
        run(&o("C:\\a"), &mut r);
        run(&Action::CopyText("id".into()), &mut r);
        run(&Action::None, &mut r);
        assert_eq!(r.calls, vec!["open:C:\\a", "copy:id"]);
    }
    #[test]
    fn run_from_helpers_noop_on_blank() {
        let mut r = Rec::default();
        run(&open_folder_action(" "), &mut r);
        run(&copy_id_action(""), &mut r);
        assert!(r.calls.is_empty());
    }
    #[test]
    fn adversarial_unicode_long() {
        let long = "a/".repeat(10000);
        match open_folder_action(&long) {
            Action::OpenFolder(p) => assert!(!p.ends_with('\\') && !p.contains('/')),
            _ => panic!(),
        }
        assert_eq!(open_folder_action("C:/日本/"), o("C:\\日本"));
        assert_eq!(open_folder_action("é"), o("é"));
    }
}
