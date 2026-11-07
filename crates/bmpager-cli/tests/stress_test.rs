//! Stress tests for `bmpager` markdown renderer, hyperlink extractor,
//! and extreme stdin payloads.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_stress_bmpager_markdown_deep_formatting() {
    let mut md = String::new();
    for i in 0..500 {
        md.push_str(&format!("# Header {}\n", i));
        md.push_str("> Deeply nested blockquote with [link](https://example.com/)\n");
        md.push_str("```rust\nfn main() { println!(\"test\"); }\n```\n");
        md.push_str("- Item with `inline code` and **bold** and *italic*\n\n");
    }

    let mut child = Command::new("cargo")
        .args(["run", "-p", "bmpager-cli", "--bin", "bmpager", "--", "-m"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(md.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Header 0"));
}

#[test]
fn test_stress_bmpager_huge_stream_non_tty() {
    let mut child = Command::new("cargo")
        .args(["run", "-p", "bmpager-cli", "--bin", "bmpager"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    // Send 10,000 lines over stdin pipe
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        for i in 0..10_000 {
            let line = format!(
                "Stress line {:05}: The quick brown fox jumps over the lazy dog\n",
                i
            );
            if stdin.write_all(line.as_bytes()).is_err() {
                break;
            }
        }
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    assert!(!output.stdout.is_empty());
}
