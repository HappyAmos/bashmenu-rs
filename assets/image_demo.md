# bmpager Markdown Image Demo

This document demonstrates inline Unicode half-block TrueColor image rendering in `bmpager --markdown` using local images from `assets/wallpapers/`.

## 1. PNG Image (Relative Path)

![Happy Sun (PNG)](wallpapers/happysun.png)

## 2. JPEG Landscape

![Skagway Harbor (JPEG)](wallpapers/skagway.jpg)

## 3. Autumn Canopy

![Fall Tree (JPEG)](wallpapers/fall-tree.jpg)

## 4. Fallback Examples

Missing local files and remote URLs degrade cleanly without making network requests:

![Missing Local Asset](wallpapers/does-not-exist.png)

![Remote URL Example](https://example.com/sample.png)

| Feature | Behavior |
| :--- | :--- |
| Local PNG / JPEG / WebP / GIF / BMP | Rendered inline via TrueColor `▀`/`▄` half-blocks with aspect-ratio scaling |
| `--no-color` | Falls back cleanly to `[Image: <alt> (<path>)]` |
| Missing or remote URL | Displays `[Image: <alt> (<url>)]` (clickable OSC 8 link for `http(s)://`) |
