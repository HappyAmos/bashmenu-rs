use std::io::Write;
use std::process::{Command, Stdio};

fn bmpager_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_bmpager"))
}

#[test]
fn test_bmpager_help_and_version() {
    let output = bmpager_cmd()
        .arg("--help")
        .output()
        .expect("failed to execute bmpager --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: command | bmpager [options]"));
    assert!(stdout.contains("-m, --markdown"));
    assert!(stdout.contains("-w, --wrap"));
    assert!(stdout.contains("+N"));
    assert!(stdout.contains("+/PATTERN"));

    let ver_output = bmpager_cmd()
        .arg("--version")
        .output()
        .expect("failed to execute bmpager --version");
    assert!(ver_output.status.success());
    let ver_stdout = String::from_utf8_lossy(&ver_output.stdout);
    assert!(ver_stdout.contains("bmpager"));
}

#[test]
fn test_bmpager_passthrough_non_tty() {
    let mut child = bmpager_cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let test_input = "Line 1: Hello World\nLine 2: Second Line\nLine 3: Final Line\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(test_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("Line 1: Hello World"));
    assert!(out_str.contains("Line 2: Second Line"));
    assert!(out_str.contains("Line 3: Final Line"));
}

#[test]
fn test_bmpager_markdown_flag_passthrough() {
    let mut child = bmpager_cmd()
        .arg("--markdown")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let md_input =
        "# Document Title\n\nWelcome to **Markdown** in `bmpager`.\n\n* Item 1\n* Item 2\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(md_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    // H1 heavy box panel borders
    assert!(out_str.contains('┏') && out_str.contains('┗'));
    assert!(out_str.contains("Document Title"));
    // Unordered bullet
    assert!(out_str.contains('•'));
    assert!(out_str.contains("Item 1"));
}

#[test]
fn test_bmpager_table_rendering_and_wrapping() {
    let mut child = bmpager_cmd()
        .arg("--markdown")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let table_input = "| Script | Description |\n| :--- | :--- |\n| `scripts/ascii.sh` | Standard and Extended ASCII viewing utility with character maps. |\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(table_input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains('┌') && out_str.contains('┐'));
    assert!(out_str.contains('├') && out_str.contains('┤'));
    assert!(out_str.contains('└') && out_str.contains('┘'));
    assert!(out_str.contains("scripts/ascii.sh"));
    assert!(out_str.contains("Standard and Extended"));
}

#[test]
fn test_bmpager_file_argument() {
    let output = bmpager_cmd()
        .arg("Cargo.toml")
        .stdout(Stdio::piped())
        .output()
        .expect("failed to run bmpager with Cargo.toml");

    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("[package]") || out_str.contains("[workspace]"));
}

#[test]
fn test_bmpager_link_extraction_and_anchor_resolution() {
    let mut child = bmpager_cmd()
        .arg("--markdown")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let doc = "\
- [3.1 Main Menu Engine (`bashmenu.py`)](#31-main-menu-engine-bashmenupy)

### 3.1 Main Menu Engine (`bashmenu.py`)

Here is section 3.1 content.
";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(doc.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    assert!(out_str.contains("3.1 Main Menu Engine"));
}

#[test]
fn test_bmpager_code_block_frame_exact_border_alignment() {
    use unicode_width::UnicodeWidthStr;

    let mut child = bmpager_cmd()
        .args(["--markdown", "--no-color"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let md = "\
```yaml
settings:
  inactivity_timeout:
    milliseconds: 300000     # Duration in milliseconds (e.g. 5 minutes)
    # command: 'cmatrix'     # Optional: command to execute (unconfigured by default)
```
";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(md.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = out_str.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(lines.len() >= 6);

    let top_w = UnicodeWidthStr::width(lines[0]);
    for line in &lines {
        assert_eq!(
            UnicodeWidthStr::width(*line),
            top_w,
            "Every code block frame row must have identical visual width ({}): {:?}",
            top_w,
            line
        );
    }
}

#[test]
fn test_bmpager_wrap_flag_plain_text() {
    let mut child = bmpager_cmd()
        .arg("--wrap")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bmpager");

    let long_text = "This is a single line with many words that would normally be quite long and overflow across small screens when soft word-wrapping is enabled in bmpager.\n";
    {
        let stdin = child.stdin.as_mut().expect("failed to open stdin");
        stdin.write_all(long_text.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read stdout");
    assert!(output.status.success());
    let out_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = out_str.lines().collect();
    // Default fallback terminal columns in headless non-tty is 80 cols
    assert!(!lines.is_empty());
    assert!(out_str.contains("soft word-wrapping"));
}

#[test]
fn test_bmpager_markdown_image_halfblock_and_fallback() {
    let dir = std::env::temp_dir().join(format!("bmpager_img_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let img_path = dir.join("pixel.png");
    let md_path = dir.join("doc.md");

    // Create a tiny 4x4 PNG (top 2 rows red, bottom 2 rows blue)
    let mut img = image::RgbaImage::new(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            let px = if y < 2 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            };
            img.put_pixel(x, y, px);
        }
    }
    img.save(&img_path).expect("save temp png");

    let md = "# Image Test\n\n![Red Blue Swatch](pixel.png)\n\n![Missing](nope.png)\n";
    std::fs::write(&md_path, md).expect("write temp md");

    // 1. Color mode renders TrueColor half-block cells + caption + missing fallback
    let out = bmpager_cmd()
        .args(["--markdown", md_path.to_str().unwrap()])
        .output()
        .expect("run bmpager --markdown");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("▀") && stdout.contains("38;2;255;0;0"),
        "expected TrueColor half-block rendering, got:\n{stdout}"
    );
    assert!(stdout.contains("Red Blue Swatch"));
    assert!(stdout.contains("[Image: Missing (nope.png)]"));

    // 2. --no-color falls back cleanly without half-block pixels
    let out_nc = bmpager_cmd()
        .args(["--markdown", "--no-color", md_path.to_str().unwrap()])
        .output()
        .expect("run bmpager --markdown --no-color");
    assert!(out_nc.status.success());
    let stdout_nc = String::from_utf8_lossy(&out_nc.stdout);
    assert!(stdout_nc.contains("[Image: Red Blue Swatch (pixel.png)]"));
    assert!(!stdout_nc.contains('▀'));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_bmpager_remote_images_opt_in_and_cache() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let dir = std::env::temp_dir().join(format!("bmpager_remote_img_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Encode a tiny 4x4 green PNG in memory
    let mut img = image::RgbaImage::new(4, 4);
    for px in img.pixels_mut() {
        *px = image::Rgba([0, 200, 50, 255]);
    }
    let mut png_bytes = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    )
    .expect("encode png");

    // Serve the PNG once over local HTTP
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().unwrap();
    let payload = png_bytes.clone();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&payload);
            let _ = stream.flush();
        }
    });

    let url = format!("http://{addr}/swatch.png");
    let md_path = dir.join("remote.md");
    std::fs::write(&md_path, format!("# Remote\n\n![Web Swatch]({url})\n")).unwrap();

    // 1. Default (without --remote-images): does NOT fetch, shows fallback link
    let out_default = bmpager_cmd()
        .env("XDG_CONFIG_HOME", &dir)
        .args(["--markdown", md_path.to_str().unwrap()])
        .output()
        .expect("run bmpager default");
    assert!(out_default.status.success());
    let stdout_default = String::from_utf8_lossy(&out_default.stdout);
    assert!(stdout_default.contains(&format!("[Image: Web Swatch ({url})]")));
    assert!(!stdout_default.contains('▀'));

    // 2. With --remote-images: downloads natively, caches in cache/images/, renders half-blocks
    let out_remote = bmpager_cmd()
        .env("XDG_CONFIG_HOME", &dir)
        .args(["--markdown", "--remote-images", md_path.to_str().unwrap()])
        .output()
        .expect("run bmpager --remote-images");
    assert!(out_remote.status.success());
    let stdout_remote = String::from_utf8_lossy(&out_remote.stdout);
    assert!(
        stdout_remote.contains('▀') && stdout_remote.contains("38;2;0;200;50"),
        "expected TrueColor remote image rendering, got:\n{stdout_remote}"
    );
    assert!(stdout_remote.contains("Web Swatch"));

    let _ = server.join();
    let cache_images = dir.join("bashmenu").join("cache").join("images");
    assert!(
        cache_images.is_dir() && std::fs::read_dir(&cache_images).unwrap().next().is_some(),
        "expected cached image file in {}",
        cache_images.display()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
