//! Local Unix-domain-socket UI-RPC client/server and interactive Markdown
//! script runner for `interactive: true` Markdown documents (`bashmenu-bash`).

use bashmenu_core::{
    parse_interactive_markdown, transpile_bashmenu_bash, MarkdownFrontmatter, MarkdownSegment,
    UiRpcRequest, UiRpcResponse, UI_RPC_SOCKET_ENV,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// A single UI-RPC request awaiting handling by the BashMenu TUI event loop.
pub struct PendingUiRpc {
    /// Deserialized request payload from the running script.
    pub request: UiRpcRequest,
    /// One-shot reply channel to unblock the script once the user responds.
    pub reply_tx: mpsc::Sender<UiRpcResponse>,
}

/// Final result produced once an interactive Markdown session finishes running.
pub struct InteractiveMdResult {
    /// Parsed YAML frontmatter from the Markdown document.
    pub frontmatter: MarkdownFrontmatter,
    /// Fallback title (filename) when `frontmatter.title` is `None`.
    pub fallback_title: String,
    /// Combined Markdown body (static segments + captured script stdout).
    pub markdown_body: String,
}

/// Live background session running an `interactive: true` Markdown document.
pub struct InteractiveMdSession {
    /// Incoming modal/toast/placeholder requests from the running script.
    pub rpc_rx: mpsc::Receiver<PendingUiRpc>,
    /// Final rendered Markdown notification once all segments complete.
    pub done_rx: mpsc::Receiver<InteractiveMdResult>,
}

/// Handles `--ui-rpc-*` client invocations from inside a running
/// `bashmenu-<lang>` script. Returns `Some(exit_code)` if a UI-RPC flag was
/// handled (so `main()` should exit immediately without initializing the TUI).
pub fn maybe_run_ui_rpc_client(args: &[String]) -> Option<i32> {
    let flag = args.get(1)?.as_str();
    let req = match flag {
        "--ui-rpc-param" => UiRpcRequest::Param {
            title: args.get(2).cloned().unwrap_or_default(),
            text: args.get(3).cloned().unwrap_or_default(),
            default_value: args.get(4).cloned().unwrap_or_default(),
            masked: args
                .get(5)
                .map(|s| s == "1" || s == "true")
                .unwrap_or(false),
        },
        "--ui-rpc-message" => UiRpcRequest::Message {
            title: args.get(2).cloned().unwrap_or_default(),
            text: args.get(3).cloned().unwrap_or_default(),
        },
        "--ui-rpc-toast" => UiRpcRequest::Toast {
            duration_secs: args.get(2).and_then(|s| s.parse::<f64>().ok()),
            text: args.get(3).cloned().unwrap_or_default(),
        },
        "--ui-rpc-placeholder" => UiRpcRequest::Placeholder {
            text: args.get(2).cloned().unwrap_or_default(),
        },
        _ => return None,
    };

    Some(send_ui_rpc_request(&req))
}

#[cfg(unix)]
fn send_ui_rpc_request(req: &UiRpcRequest) -> i32 {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let Ok(sock_path) = std::env::var(UI_RPC_SOCKET_ENV) else {
        return 1;
    };
    let Ok(mut stream) = UnixStream::connect(&sock_path) else {
        return 1;
    };
    let Ok(line) = serde_json::to_string(req) else {
        return 1;
    };
    if writeln!(stream, "{line}").is_err() || stream.flush().is_err() {
        return 1;
    }

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    if reader.read_line(&mut resp_line).is_err() {
        return 1;
    }
    let Ok(resp) = serde_json::from_str::<UiRpcResponse>(&resp_line) else {
        return 1;
    };
    if !resp.value.is_empty() {
        print!("{}", resp.value);
        let _ = std::io::stdout().flush();
    }
    if resp.ok && !resp.cancelled {
        0
    } else {
        1
    }
}

#[cfg(not(unix))]
fn send_ui_rpc_request(_req: &UiRpcRequest) -> i32 {
    1
}

/// Generates a 128-bit unpredictable hex token for socket and temp script names.
#[cfg(unix)]
fn random_hex_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut bytes = [0u8; 16];
    let filled = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
        .is_ok();
    if !filled {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut h1 = RandomState::new().build_hasher();
        h1.write_u32(std::process::id());
        h1.write_u128(nanos);
        h1.write_u64(seq);
        let mut h2 = RandomState::new().build_hasher();
        h2.write_u64(h1.finish());
        bytes[..8].copy_from_slice(&h1.finish().to_ne_bytes());
        bytes[8..].copy_from_slice(&h2.finish().to_ne_bytes());
    }
    let mut hex = String::with_capacity(32);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

#[cfg(unix)]
fn runtime_base_dir() -> PathBuf {
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR") {
        let trimmed = rt.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_dir() {
                return p;
            }
        }
    }
    std::env::temp_dir()
}

/// Spawns a background runner for an `interactive: true` Markdown document.
///
/// Executes each `bashmenu-bash` code block in order while relaying live UI-RPC
/// modal requests (`{param}`, `{message}`, `{toast}`) back to the TUI event loop.
#[cfg(unix)]
pub fn spawn_interactive_markdown(
    frontmatter: MarkdownFrontmatter,
    fallback_title: String,
    raw_body: String,
    self_bin: PathBuf,
) -> std::io::Result<InteractiveMdSession> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let base_dir = runtime_base_dir();
    let sock_path = base_dir.join(format!("bashmenu_ui_rpc_{}.sock", random_hex_token()));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;
    let _ = std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600));
    let _ = listener.set_nonblocking(true);

    let (rpc_tx, rpc_rx) = mpsc::channel::<PendingUiRpc>();
    let (done_tx, done_rx) = mpsc::channel::<InteractiveMdResult>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_listener = Arc::clone(&stop_flag);
    let sock_for_listener = sock_path.clone();

    // 1. Socket listener thread
    let listener_handle = std::thread::spawn(move || {
        while !stop_listener.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let mut line = String::new();
                    let read_ok = {
                        let mut reader = BufReader::new(&stream);
                        reader.read_line(&mut line).is_ok()
                    };
                    if !read_ok || line.trim().is_empty() {
                        continue;
                    }
                    let resp = match serde_json::from_str::<UiRpcRequest>(&line) {
                        Ok(request) => {
                            let (reply_tx, reply_rx) = mpsc::channel();
                            if rpc_tx.send(PendingUiRpc { request, reply_tx }).is_ok() {
                                reply_rx.recv().unwrap_or(UiRpcResponse {
                                    ok: false,
                                    value: String::new(),
                                    cancelled: true,
                                })
                            } else {
                                UiRpcResponse {
                                    ok: false,
                                    value: String::new(),
                                    cancelled: true,
                                }
                            }
                        }
                        Err(_) => UiRpcResponse {
                            ok: false,
                            value: String::new(),
                            cancelled: false,
                        },
                    };
                    if let Ok(json) = serde_json::to_string(&resp) {
                        let _ = writeln!(stream, "{json}");
                        let _ = stream.flush();
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = std::fs::remove_file(&sock_for_listener);
    });

    // 2. Script execution thread
    std::thread::spawn(move || {
        let segments = parse_interactive_markdown(&raw_body);
        let mut combined_md = String::new();

        for seg in segments {
            match seg {
                MarkdownSegment::Markdown(md) => {
                    combined_md.push_str(&md);
                }
                MarkdownSegment::Script { language, code } => {
                    if language == "bash" {
                        let transpiled = transpile_bashmenu_bash(&code, &self_bin);
                        let script_path =
                            base_dir.join(format!("bashmenu_md_script_{}.sh", random_hex_token()));
                        let write_res = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o600)
                            .open(&script_path)
                            .and_then(|mut f| {
                                f.write_all(transpiled.as_bytes())?;
                                f.flush()
                            });
                        if write_res.is_ok() {
                            let out = std::process::Command::new("/bin/bash")
                                .arg(&script_path)
                                .env(UI_RPC_SOCKET_ENV, &sock_path)
                                .output();
                            let _ = std::fs::remove_file(&script_path);
                            if let Ok(output) = out {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                if !stdout.trim().is_empty() {
                                    if !combined_md.is_empty() && !combined_md.ends_with('\n') {
                                        combined_md.push('\n');
                                    }
                                    combined_md.push_str(&stdout);
                                    if !combined_md.ends_with('\n') {
                                        combined_md.push('\n');
                                    }
                                }
                            }
                        }
                    } else {
                        combined_md.push_str(&format!(
                            "\n> Unsupported interactive block language: `bashmenu-{language}`\n"
                        ));
                    }
                }
            }
        }

        stop_flag.store(true, Ordering::Relaxed);
        let _ = listener_handle.join();
        let _ = std::fs::remove_file(&sock_path);

        let _ = done_tx.send(InteractiveMdResult {
            frontmatter,
            fallback_title,
            markdown_body: combined_md,
        });
    });

    Ok(InteractiveMdSession { rpc_rx, done_rx })
}

#[cfg(not(unix))]
pub fn spawn_interactive_markdown(
    frontmatter: MarkdownFrontmatter,
    fallback_title: String,
    raw_body: String,
    _self_bin: PathBuf,
) -> std::io::Result<InteractiveMdSession> {
    let (_rpc_tx, rpc_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let _ = done_tx.send(InteractiveMdResult {
        frontmatter,
        fallback_title,
        markdown_body: raw_body,
    });
    Ok(InteractiveMdSession { rpc_rx, done_rx })
}

/// Resolves the path to the currently running `bashmenu` executable for UI-RPC callbacks.
pub fn current_bashmenu_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| Path::new("bashmenu").to_path_buf())
}
