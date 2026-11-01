//! Cross-platform clipboard integration with OSC 52, desktop CLI fallbacks, and shared cutbuffer.

use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static IN_MEMORY_CLIPBOARD: Mutex<Option<String>> = Mutex::new(None);
static TEST_MODE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static THREAD_CLIPBOARD: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Enables or disables isolated test mode clipboard
pub fn set_test_mode(enabled: bool) {
    TEST_MODE.store(enabled, Ordering::SeqCst);
}

/// Checks if clipboard is in test mode (thread-isolated)
pub fn is_test_mode() -> bool {
    TEST_MODE.load(Ordering::SeqCst)
        || std::env::var_os("BASHMENU_TEST_MODE").is_some()
        || cfg!(test)
}

/// Returns the shared cutbuffer cache file path
pub fn get_shared_cutbuffer_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache/bashmenu/cutbuffer.txt")
    } else {
        std::env::temp_dir().join("bashmenu_cutbuffer.txt")
    }
}

/// Minimal standard Base64 encoder for OSC 52 clipboard sequences
pub fn base64_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Writes text to system clipboard using OSC 52, desktop clipboard helpers, file cutbuffer, and in-memory cache.
pub fn set_clipboard_text(text: &str) {
    THREAD_CLIPBOARD.with(|cb| {
        *cb.borrow_mut() = Some(text.to_string());
    });

    if is_test_mode() {
        return;
    }

    // 1. Update in-memory static cache
    if let Ok(mut lock) = IN_MEMORY_CLIPBOARD.lock() {
        *lock = Some(text.to_string());
    }

    // 2. Persist to shared cutbuffer file
    let path = get_shared_cutbuffer_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, text);

    // 3. Emit OSC 52 escape sequence to stdout (works locally & over SSH/tmux)
    let b64 = base64_encode(text.as_bytes());
    print!("\x1b]52;c;{}\x07", b64);
    let _ = std::io::stdout().flush();

    // 4. Try native desktop clipboard CLI if available
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut child) = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            if let Ok(mut child) = Command::new("wl-copy")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
                return;
            }
        }

        if std::env::var_os("DISPLAY").is_some() {
            // Try xclip first
            if let Ok(mut child) = Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
                return;
            }
            // Fallback to xsel
            if let Ok(mut child) = Command::new("xsel")
                .args(["-b", "-i"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(text.as_bytes());
                }
                let _ = child.wait();
            }
        }
    }
}

/// Reads text from system clipboard using desktop CLI helpers, shared cutbuffer file, or in-memory cache.
pub fn get_clipboard_text() -> Option<String> {
    if is_test_mode() {
        return THREAD_CLIPBOARD.with(|cb| cb.borrow().clone());
    }

    // 1. Try native desktop clipboard CLI if available
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("pbpaste").output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            if let Ok(output) = Command::new("wl-paste").arg("--no-newline").output() {
                if output.status.success() {
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
        }

        if std::env::var_os("DISPLAY").is_some() {
            // Try xclip
            if let Ok(output) = Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()
            {
                if output.status.success() {
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
            // Fallback to xsel
            if let Ok(output) = Command::new("xsel").args(["-b", "-o"]).output() {
                if output.status.success() {
                    if let Ok(s) = String::from_utf8(output.stdout) {
                        if !s.is_empty() {
                            return Some(s);
                        }
                    }
                }
            }
        }
    }

    // 2. Fallback to shared cutbuffer file
    let path = get_shared_cutbuffer_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if !content.is_empty() {
                return Some(content);
            }
        }
    }

    // 3. Fallback to in-memory static cache
    if let Ok(lock) = IN_MEMORY_CLIPBOARD.lock() {
        if let Some(ref s) = *lock {
            if !s.is_empty() {
                return Some(s.clone());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_set_and_get_clipboard_text_roundtrip() {
        let sample = format!("test_clip_{}", std::process::id());
        set_clipboard_text(&sample);
        let retrieved = get_clipboard_text().expect("Clipboard must return content");
        assert_eq!(retrieved, sample);
    }
}
