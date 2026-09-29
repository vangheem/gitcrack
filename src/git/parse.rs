use crate::model::{ChangeKind, DiffLine, DiffLineKind, FileDiff, FileStatus};

struct PartialFile {
    path: String,
    old_path: Option<String>,
    lines: Vec<DiffLine>,
    additions: u32,
    deletions: u32,
    binary: bool,
    saw_rename: bool,
    saw_copy: bool,
    saw_new: bool,
    saw_deleted: bool,
    saw_old_devnull: bool,
    saw_new_devnull: bool,
}

impl PartialFile {
    fn from_diff_git(line: &str) -> Self {
        let (old, new) = parse_diff_git_paths(line);
        Self {
            path: new,
            old_path: if old.is_empty() { None } else { Some(old) },
            lines: Vec::new(),
            additions: 0,
            deletions: 0,
            binary: false,
            saw_rename: false,
            saw_copy: false,
            saw_new: false,
            saw_deleted: false,
            saw_old_devnull: false,
            saw_new_devnull: false,
        }
    }

    fn finish(self) -> Option<FileDiff> {
        if self.path.is_empty() {
            return None;
        }
        let status = if self.saw_rename {
            FileStatus::Renamed
        } else if self.saw_copy {
            FileStatus::Copied
        } else if self.saw_new || self.saw_old_devnull {
            FileStatus::Added
        } else if self.saw_deleted || self.saw_new_devnull {
            FileStatus::Deleted
        } else {
            FileStatus::Modified
        };
        let old_path = if self.saw_rename || self.saw_copy {
            self.old_path.filter(|path| !path.is_empty())
        } else {
            None
        };
        Some(FileDiff {
            path: self.path,
            old_path,
            status,
            kind: ChangeKind::Committed,
            additions: self.additions,
            deletions: self.deletions,
            binary: self.binary,
            lines: self.lines,
        })
    }
}

pub fn parse_unified_diff(text: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut current: Option<PartialFile> = None;
    let mut in_hunk = false;
    let mut skip_binary = false;
    let mut old_ln = 0u32;
    let mut new_ln = 0u32;

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            if let Some(file) = current.take().and_then(PartialFile::finish) {
                files.push(file);
            }
            current = Some(PartialFile::from_diff_git(line));
            in_hunk = false;
            skip_binary = false;
            continue;
        }
        let Some(file) = current.as_mut() else {
            continue;
        };
        if skip_binary {
            continue;
        }
        if let Some((old_start, new_start)) = parse_hunk_header(line) {
            old_ln = old_start;
            new_ln = new_start;
            in_hunk = true;
            file.lines.push(meta_line(DiffLineKind::Hunk, line));
            continue;
        }
        if !in_hunk {
            if apply_header(file, line) {
                if line.starts_with("GIT binary patch") {
                    skip_binary = true;
                }
            }
            continue;
        }
        if line.starts_with('\\') {
            file.lines.push(meta_line(DiffLineKind::Meta, line));
            continue;
        }
        if let Some(text) = line.strip_prefix('+') {
            file.lines.push(DiffLine {
                kind: DiffLineKind::Add,
                old_lineno: None,
                new_lineno: Some(new_ln),
                text: text.to_string(),
            });
            file.additions = file.additions.saturating_add(1);
            new_ln = new_ln.saturating_add(1);
            continue;
        }
        if let Some(text) = line.strip_prefix('-') {
            file.lines.push(DiffLine {
                kind: DiffLineKind::Del,
                old_lineno: Some(old_ln),
                new_lineno: None,
                text: text.to_string(),
            });
            file.deletions = file.deletions.saturating_add(1);
            old_ln = old_ln.saturating_add(1);
            continue;
        }
        if let Some(text) = line.strip_prefix(' ') {
            file.lines.push(DiffLine {
                kind: DiffLineKind::Context,
                old_lineno: Some(old_ln),
                new_lineno: Some(new_ln),
                text: text.to_string(),
            });
            old_ln = old_ln.saturating_add(1);
            new_ln = new_ln.saturating_add(1);
        }
    }

    if let Some(file) = current.take().and_then(PartialFile::finish) {
        files.push(file);
    }
    files
}

fn meta_line(kind: DiffLineKind, text: &str) -> DiffLine {
    DiffLine {
        kind,
        old_lineno: None,
        new_lineno: None,
        text: text.to_string(),
    }
}

fn apply_header(file: &mut PartialFile, line: &str) -> bool {
    if line.starts_with("new file mode") {
        file.saw_new = true;
        return false;
    }
    if line.starts_with("deleted file mode") {
        file.saw_deleted = true;
        return false;
    }
    if let Some(rest) = line.strip_prefix("rename from ") {
        file.saw_rename = true;
        file.old_path = Some(unquote_path(rest));
        return false;
    }
    if let Some(rest) = line.strip_prefix("rename to ") {
        file.saw_rename = true;
        file.path = unquote_path(rest);
        return false;
    }
    if let Some(rest) = line.strip_prefix("copy from ") {
        file.saw_copy = true;
        file.old_path = Some(unquote_path(rest));
        return false;
    }
    if let Some(rest) = line.strip_prefix("copy to ") {
        file.saw_copy = true;
        file.path = unquote_path(rest);
        return false;
    }
    if let Some(rest) = line.strip_prefix("--- ") {
        let path = parse_ab_path(rest);
        if path == "/dev/null" {
            file.saw_old_devnull = true;
        } else {
            file.old_path = Some(path);
        }
        return false;
    }
    if let Some(rest) = line.strip_prefix("+++ ") {
        let path = parse_ab_path(rest);
        if path == "/dev/null" {
            file.saw_new_devnull = true;
        } else {
            file.path = path;
        }
        return false;
    }
    if line.starts_with("Binary files") || line.starts_with("GIT binary patch") {
        file.binary = true;
        file.lines.push(meta_line(DiffLineKind::Meta, line));
        return true;
    }
    false
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_start, rest) = parse_hunk_num(rest)?;
    let rest = rest.strip_prefix('+')?;
    let (new_start, rest) = parse_hunk_num(rest)?;
    rest.starts_with("@@").then_some((old_start, new_start))
}

fn parse_hunk_num(input: &str) -> Option<(u32, &str)> {
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    let start = input[..index].parse().ok()?;
    if index < bytes.len() && bytes[index] == b',' {
        index += 1;
        let count_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == count_start {
            return None;
        }
    }
    if index < bytes.len() && bytes[index] == b' ' {
        index += 1;
    }
    Some((start, &input[index..]))
}

fn parse_diff_git_paths(line: &str) -> (String, String) {
    let rest = line.strip_prefix("diff --git ").unwrap_or(line);
    if rest.starts_with('"') {
        if let Some((old, rem)) = unquote_c(rest) {
            let rem = rem.trim_start();
            if let Some((new, _)) = unquote_c(rem) {
                return (strip_ab(&old), strip_ab(&new));
            }
            return (strip_ab(&old), String::new());
        }
    }
    if let Some(split) = find_b_separator(rest) {
        let old = strip_ab(&rest[..split]);
        let new = strip_ab(&rest[split + 1..]);
        return (old, new);
    }
    (String::new(), String::new())
}

fn find_b_separator(rest: &str) -> Option<usize> {
    let mut offset = 0;
    while let Some(rel) = rest[offset..].find(" b/") {
        let abs = offset + rel;
        if rest[..abs].starts_with("a/") {
            return Some(abs);
        }
        offset = abs + 1;
        if offset >= rest.len() {
            break;
        }
    }
    None
}

fn parse_ab_path(rest: &str) -> String {
    if rest.starts_with('"') {
        if let Some((path, _)) = unquote_c(rest) {
            return normalize_diff_path(&path);
        }
    }
    let path = rest.split('\t').next().unwrap_or(rest);
    normalize_diff_path(path)
}

fn normalize_diff_path(path: &str) -> String {
    if path == "/dev/null" {
        path.to_string()
    } else {
        strip_ab(path)
    }
}

fn strip_ab(path: &str) -> String {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
        .to_string()
}

fn unquote_path(rest: &str) -> String {
    let rest = rest.split('\t').next().unwrap_or(rest);
    if rest.starts_with('"') {
        if let Some((path, _)) = unquote_c(rest) {
            return path;
        }
    }
    rest.to_string()
}

fn unquote_c(input: &str) -> Option<(String, &str)> {
    if !input.starts_with('"') {
        return None;
    }
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let text = String::from_utf8_lossy(&out).into_owned();
                return Some((text, &input[index + 1..]));
            }
            b'\\' => {
                index += 1;
                if index >= bytes.len() {
                    break;
                }
                match bytes[index] {
                    b'a' => out.push(0x07),
                    b'b' => out.push(0x08),
                    b't' => out.push(b'\t'),
                    b'n' => out.push(b'\n'),
                    b'v' => out.push(0x0b),
                    b'f' => out.push(0x0c),
                    b'r' => out.push(b'\r'),
                    b'\\' => out.push(b'\\'),
                    b'"' => out.push(b'"'),
                    b'0'..=b'7' => {
                        let mut value = u32::from(bytes[index] - b'0');
                        let mut count = 1;
                        while count < 3
                            && index + 1 < bytes.len()
                            && (b'0'..=b'7').contains(&bytes[index + 1])
                        {
                            index += 1;
                            value = value * 8 + u32::from(bytes[index] - b'0');
                            count += 1;
                        }
                        out.push(value as u8);
                    }
                    other => out.push(other),
                }
            }
            byte => out.push(byte),
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_unified_diff;
    use crate::model::{ChangeKind, DiffLineKind, FileStatus};

    #[test]
    fn parse_modify() {
        let text = "\
commit 1

diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -1,3 +1,4 @@ fn keep
 alpha
-beta
+beta2
+gamma
 omega
\\ No newline at end of file
@@ -10 +11 @@
-old
+new
diff --git 
";
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "file.txt");
        assert_eq!(file.old_path, None);
        assert_eq!(file.status, FileStatus::Modified);
        assert_eq!(file.kind, ChangeKind::Committed);
        assert!(!file.binary);
        assert_eq!(file.additions, 3);
        assert_eq!(file.deletions, 2);
        assert_eq!(file.lines[0].kind, DiffLineKind::Hunk);
        assert_eq!(file.lines[0].text, "@@ -1,3 +1,4 @@ fn keep");
        assert_eq!(file.lines[1].kind, DiffLineKind::Context);
        assert_eq!(file.lines[1].text, "alpha");
        assert_eq!(file.lines[1].old_lineno, Some(1));
        assert_eq!(file.lines[1].new_lineno, Some(1));
        assert_eq!(file.lines[2].kind, DiffLineKind::Del);
        assert_eq!(file.lines[2].text, "beta");
        assert_eq!(file.lines[2].old_lineno, Some(2));
        assert_eq!(file.lines[2].new_lineno, None);
        assert_eq!(file.lines[3].kind, DiffLineKind::Add);
        assert_eq!(file.lines[3].text, "beta2");
        assert_eq!(file.lines[3].new_lineno, Some(2));
        assert_eq!(file.lines[4].text, "gamma");
        assert_eq!(file.lines[4].new_lineno, Some(3));
        assert_eq!(file.lines[5].kind, DiffLineKind::Context);
        assert_eq!(file.lines[5].text, "omega");
        assert_eq!(file.lines[5].old_lineno, Some(3));
        assert_eq!(file.lines[5].new_lineno, Some(4));
        assert_eq!(file.lines[6].kind, DiffLineKind::Meta);
        assert_eq!(file.lines[6].text, "\\ No newline at end of file");
        assert_eq!(file.lines[6].old_lineno, None);
        assert_eq!(file.lines[6].new_lineno, None);
        assert_eq!(file.lines[8].kind, DiffLineKind::Del);
        assert_eq!(file.lines[8].old_lineno, Some(10));
        assert_eq!(file.lines[9].kind, DiffLineKind::Add);
        assert_eq!(file.lines[9].new_lineno, Some(11));
    }

    #[test]
    fn parse_add() {
        let text = "\
diff --git a/new.txt b/new.txt
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,2 @@
+one
+two
";
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "new.txt");
        assert_eq!(file.status, FileStatus::Added);
        assert_eq!(file.old_path, None);
        assert_eq!(file.additions, 2);
        assert_eq!(file.deletions, 0);
        assert_eq!(file.lines[1].new_lineno, Some(1));
        assert_eq!(file.lines[1].text, "one");
        assert_eq!(file.lines[2].new_lineno, Some(2));
        assert_eq!(file.lines[2].old_lineno, None);
    }

    #[test]
    fn parse_delete() {
        let text = "\
diff --git a/old.txt b/old.txt
deleted file mode 100644
index 1234567..0000000
--- a/old.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-one
-two
";
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 1);
        let file = &files[0];
        assert_eq!(file.path, "old.txt");
        assert_eq!(file.status, FileStatus::Deleted);
        assert_eq!(file.additions, 0);
        assert_eq!(file.deletions, 2);
        assert_eq!(file.lines[1].kind, DiffLineKind::Del);
        assert_eq!(file.lines[1].old_lineno, Some(1));
        assert_eq!(file.lines[1].text, "one");
        assert_eq!(file.lines[2].old_lineno, Some(2));
    }

    #[test]
    fn parse_rename() {
        let text = "\
diff --git a/old name.txt b/new name.txt
similarity index 80%
rename from old name.txt
rename to new name.txt
index 1111111..2222222 100644
--- a/old name.txt
+++ b/new name.txt
@@ -1 +1,2 @@
 keep
+extra
diff --git a/src.txt b/copied.txt
similarity index 100%
copy from src.txt
copy to copied.txt
";
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 2);
        let renamed = &files[0];
        assert_eq!(renamed.path, "new name.txt");
        assert_eq!(renamed.old_path.as_deref(), Some("old name.txt"));
        assert_eq!(renamed.status, FileStatus::Renamed);
        assert_eq!(renamed.additions, 1);
        assert_eq!(renamed.deletions, 0);
        assert_eq!(renamed.lines[1].kind, DiffLineKind::Context);
        assert_eq!(renamed.lines[1].text, "keep");
        assert_eq!(renamed.lines[1].old_lineno, Some(1));
        assert_eq!(renamed.lines[1].new_lineno, Some(1));
        let copied = &files[1];
        assert_eq!(copied.path, "copied.txt");
        assert_eq!(copied.old_path.as_deref(), Some("src.txt"));
        assert_eq!(copied.status, FileStatus::Copied);
    }

    #[test]
    fn parse_binary() {
        let text = "\
diff --git a/bin.dat b/bin.dat
new file mode 100644
index 0000000..1111111
Binary files /dev/null and b/bin.dat differ
diff --git a/patch.bin b/patch.bin
index 1111111..2222222 100644
GIT binary patch
literal 4
McmYdfNMc9^

literal 0
HcmV?d00001

diff --git a/after.txt b/after.txt
index 1111111..2222222 100644
--- a/after.txt
+++ b/after.txt
@@ -1 +1 @@
-a
+b
";
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 3);
        assert!(files[0].binary);
        assert_eq!(files[0].status, FileStatus::Added);
        assert_eq!(files[0].lines.len(), 1);
        assert_eq!(files[0].lines[0].kind, DiffLineKind::Meta);
        assert_eq!(
            files[0].lines[0].text,
            "Binary files /dev/null and b/bin.dat differ"
        );
        assert_eq!(files[0].additions, 0);
        assert!(files[1].binary);
        assert_eq!(files[1].lines.len(), 1);
        assert_eq!(files[1].lines[0].text, "GIT binary patch");
        assert_eq!(files[1].status, FileStatus::Modified);
        assert_eq!(files[2].path, "after.txt");
        assert!(!files[2].binary);
        assert_eq!(files[2].additions, 1);
        assert_eq!(files[2].deletions, 1);
    }

    #[test]
    fn parse_quoted_paths() {
        let text = r#"diff --git "a/sp ace/f\"ile.txt" "b/sp ace/f\"ile.txt"
new file mode 100644
index 0000000..1234567
--- /dev/null
+++ "b/sp ace/f\"ile.txt"
@@ -0,0 +1 @@
+x
"#;
        let files = parse_unified_diff(text);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "sp ace/f\"ile.txt");
        assert_eq!(files[0].status, FileStatus::Added);
        assert_eq!(files[0].lines[1].text, "x");
    }
}
