//! End-to-end integration test for `interactive: true` Markdown (`bashmenu-bash`)
//! and live Unix-domain-socket UI-RPC round-trips (`--ui-rpc-param`, `--ui-rpc-message`).

#[cfg(unix)]
#[test]
fn test_interactive_markdown_bash_guessing_game_roundtrip() {
    use bashmenu_core::{
        parse_interactive_markdown, parse_markdown_frontmatter_full, transpile_bashmenu_bash,
        MarkdownSegment, UiRpcRequest, UiRpcResponse, UI_RPC_SOCKET_ENV,
    };
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::process::Command;

    let doc = "\
---   
title: playground.md
width: 80%
height: 70%
interactive: true
---   

````bashmenu-bash
#!/usr/bin/env bash
while true; do
    GUESS=\"{param:'Guess a number (q to quit):'}\"
    case \"$GUESS\" in
        [Qq]*) echo \"Aborted.\"; exit 1;;
        *) {message:\"You entered $GUESS\"} ;;
    esac
done
````

# This is a title
The game has concluded.
";

    let (fm, body) = parse_markdown_frontmatter_full(doc);
    assert_eq!(fm.title.as_deref(), Some("playground.md"));
    assert_eq!(fm.width, Some(80));
    assert_eq!(fm.height, Some(70));
    assert!(fm.interactive);

    let segments = parse_interactive_markdown(body);
    assert_eq!(segments.len(), 2);
    let bash_code = match &segments[0] {
        MarkdownSegment::Script { language, code } => {
            assert_eq!(language, "bash");
            code.clone()
        }
        other => panic!("expected bash script segment, got {other:?}"),
    };

    let bashmenu_bin = PathBuf::from(env!("CARGO_BIN_EXE_bashmenu"));
    let transpiled = transpile_bashmenu_bash(&bash_code, &bashmenu_bin);

    let sock_path = std::env::temp_dir().join(format!(
        "bashmenu_interactive_md_test_{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind test unix socket");

    // Mock TUI server: responds "42" to first param, acknowledges message, then "q" to second param
    let server = std::thread::spawn(move || {
        let mut observed: Vec<UiRpcRequest> = Vec::new();
        let mut param_count = 0usize;
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept client");
            let mut line = String::new();
            {
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut line).expect("read rpc line");
            }
            let req: UiRpcRequest = serde_json::from_str(&line).expect("parse UiRpcRequest");
            let resp = match &req {
                UiRpcRequest::Param { .. } => {
                    param_count += 1;
                    let val = if param_count == 1 { "42" } else { "q" };
                    UiRpcResponse {
                        ok: true,
                        value: val.to_string(),
                        cancelled: false,
                    }
                }
                UiRpcRequest::Message { .. } => UiRpcResponse {
                    ok: true,
                    value: String::new(),
                    cancelled: false,
                },
                _ => UiRpcResponse {
                    ok: true,
                    value: String::new(),
                    cancelled: false,
                },
            };
            observed.push(req);
            writeln!(stream, "{}", serde_json::to_string(&resp).unwrap()).expect("write resp");
            stream.flush().expect("flush resp");
        }
        observed
    });

    let script_path = std::env::temp_dir().join(format!(
        "bashmenu_interactive_md_test_{}.sh",
        std::process::id()
    ));
    std::fs::write(&script_path, &transpiled).expect("write temp script");

    let out = Command::new("/bin/bash")
        .arg(&script_path)
        .env(UI_RPC_SOCKET_ENV, &sock_path)
        .output()
        .expect("run transpiled bash script");

    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&sock_path);

    let observed = server.join().expect("join mock server");
    assert_eq!(observed.len(), 3);
    assert_eq!(
        observed[0],
        UiRpcRequest::Param {
            title: String::new(),
            text: "Guess a number (q to quit):".to_string(),
            default_value: String::new(),
            masked: false,
        }
    );
    assert_eq!(
        observed[1],
        UiRpcRequest::Message {
            title: String::new(),
            text: "You entered 42".to_string(),
        }
    );
    assert_eq!(
        observed[2],
        UiRpcRequest::Param {
            title: String::new(),
            text: "Guess a number (q to quit):".to_string(),
            default_value: String::new(),
            masked: false,
        }
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Aborted.");
    assert_eq!(out.status.code(), Some(1));
}
