# Security Policy

## Supported Versions

BashMenu-RS is actively developed in rolling release fashion on the `main` branch. Security fixes and patches are prioritized and applied directly to `main`.

| Version / Branch | Supported          |
| ---------------- | ------------------ |
| `main`           | :white_check_mark: |
| Older releases   | :x:                |

## Threat Model & Considerations

BashMenu-RS is a terminal menu launcher and companion tool suite designed to execute local shell scripts, supervise child processes, and manage local configuration and swap files. Please consider the following boundaries when assessing security reports:

1. **Local Configuration Files:** Configurations (`bashmenu.yml`, `bashmenu.mnu`, `shortcuts.yml`) are executed with the permissions of the invoking user. Untrusted configuration files should never be loaded without inspection.
2. **Child Process & Shell Execution:** Spawning shell commands, child TUIs, or scripts is intentional core functionality. Input validation vulnerabilities in macro interpolation or argument handling that lead to unintended command injection on trusted inputs are valid security concerns.
3. **Swap Files & Data Privacy:** `bashedit` and notebook engines create colocated swap files (`.<filename>.swp`) to ensure crash recovery. Reports involving race conditions, insecure permissions, or data leakage in temporary or swap files are treated seriously.
4. **Console & Hardware Boundaries:** Framebuffer features (`/dev/fb0`) require appropriate group permissions (e.g. `video`). The application must fail cleanly when access is denied, without memory corruption or privileged escalation.

## Reporting a Vulnerability

If you discover a security vulnerability in BashMenu-RS, please report it privately rather than creating a public GitHub issue.

- **Email:** Send details to **happyamos-rs@anglernook.com** with the subject line `[SECURITY] BashMenu-RS Vulnerability Report`.
- **Details to Include:**
  - Description of the issue and potential impact.
  - Minimal reproducible example, test case, or step-by-step reproduction instructions.
  - Affected binary or crate (`bashmenu`, `bashedit`, `bmpager`, `bashmenu-core`, etc.).
  - Operating system, terminal environment, and Rust version.

## Response Process

- We will acknowledge receipt of your report within 48 hours.
- We will coordinate with you to validate the issue, prepare a fix, and confirm resolution prior to public disclosure.
- Security updates will be credited in release notes unless anonymity is requested.
