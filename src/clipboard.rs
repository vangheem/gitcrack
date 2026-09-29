use std::io::Write;
use std::process::{Command, Stdio};

pub fn copy(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    system_clipboard(text) || osc52(text)
}

fn system_clipboard(text: &str) -> bool {
    if cfg!(target_os = "macos") {
        return copy_with("pbcopy", &[], text);
    }
    if cfg!(target_os = "windows") {
        return copy_with("clip", &[], text);
    }
    copy_with("wl-copy", &[], text)
        || copy_with("xclip", &["-selection", "clipboard"], text)
        || copy_with("xsel", &["--clipboard", "--input"], text)
}

fn copy_with(program: &str, args: &[&str], text: &str) -> bool {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    if stdin.write_all(text.as_bytes()).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return false;
    }
    drop(stdin);
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        return child.wait().is_ok_and(|status| status.success());
    }
    let _ = std::thread::spawn(move || {
        let _ = child.wait();
    });
    true
}

fn osc52(text: &str) -> bool {
    let encoded = base64(text.as_bytes());
    let osc = format!("\x1b]52;c;{encoded}\x1b\\");
    let seq = if std::env::var_os("TMUX").is_some() {
        format!("\x1bPtmux;\x1b{osc}\x1b\\")
    } else {
        osc
    };
    let mut out = std::io::stdout().lock();
    out.write_all(seq.as_bytes()).is_ok() && out.flush().is_ok()
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= data.len() {
        let n =
            ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8) | data[index + 2] as u32;
        push_base64(&mut out, TABLE, n, 4);
        index += 3;
    }
    match data.len() - index {
        1 => {
            let n = (data[index] as u32) << 16;
            push_base64(&mut out, TABLE, n, 2);
            out.push_str("==");
        }
        2 => {
            let n = ((data[index] as u32) << 16) | ((data[index + 1] as u32) << 8);
            push_base64(&mut out, TABLE, n, 3);
            out.push('=');
        }
        _ => {}
    }
    out
}

fn push_base64(out: &mut String, table: &[u8], n: u32, count: usize) {
    let shifts = [18, 12, 6, 0];
    for shift in shifts.into_iter().take(count) {
        out.push(table[((n >> shift) & 63) as usize] as char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"hello"), "aGVsbG8=");
        assert_eq!(base64(b"hi"), "aGk=");
        assert_eq!(base64(b"h"), "aA==");
    }
}
