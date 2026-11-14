# Cheat CLI

A lightweight CLI tool to search, list, and render Markdown cheatsheets.

## Features

- **Markdown Rendering**: Uses `rich.sh` (Python Rich) for terminal rendering, falling back to `bmpager --markdown` and then raw standard output.
- **Tag Support**: Parses YAML frontmatter tags in cheatsheets.
- **Priority Search**: Matches filenames/paths first, then tags, then content.
- **Flexible Listing**: List all cheatsheets with tags or filter matches.

## Requirements & Installation

`cheat` is built directly as part of the BashMenu-RS workspace:

```bash
cargo build --release --bin cheat
# Or install all binaries into ~/.local/bin:
./install.sh
```

## Configuration

The tool reads `cheat.yml` located in `~/.config/bashmenu/cheat.yml`.
It automatically creates a default config if one does not exist.

```yaml
dir: ~/cheat
```

`dir`: Path to the directory containing your Markdown cheatsheet files.

## Usage

### View a Cheatsheet

Find and display a cheatsheet by path, stem, tag, or text content:

```bash
cheat python/dicts
cheat docker
```

### List and Search Cheatsheets

List all available cheatsheets along with their frontmatter tags:

```bash
cheat -l
```

Filter listed files by path, tag, or text match:

```bash
cheat -l git
cheat -s git
```

### Print Configured Directory

Print the configured cheatsheets directory path:

```bash
cheat -d
# or
cheat --directory
```

You can also use this to quickly navigate to your cheatsheets directory in your shell:

```bash
cd "$(cheat -d)"
```

## Cheatsheet Format

Add YAML frontmatter tags to `.md` files to enable tag matching:

```markdown
---
tags: [git, vcs, undo]
---

# Git Undo Cheatsheet
...
```

## Search Resolution Order

1. Exact relative path or filename match
2. Frontmatter tag match
3. Full-text content search

