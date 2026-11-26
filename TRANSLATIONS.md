# BashMenu-RS Translation & Localization Quick-Start Guide

Welcome to the **BashMenu-RS** localization guide! This guide explains how anyone—regardless of whether you know Rust or programming—can translate BashMenu and all of its companion tools into another language.

---

## Overview

All user-facing strings across the entire BashMenu suite (navigation, dialogs, button labels, modal windows, error notices, status bars, and inspector labels) are organized into structured YAML files inside the [`locales/`](locales/) directory:

- **Canonical Template (Source of Truth):** [`locales/en_us.yml`](locales/en_us.yml) (English / US)
- **Reference Example:** [`locales/es_es.yml`](locales/es_es.yml) (Spanish / Spain)

Translating BashMenu requires **no Rust coding**. You only edit text strings in a single YAML file, test it live in your terminal, validate it using the built-in validator, and submit a Pull Request!

---

## Step-by-Step Translation Workflow

### Step 1: Coordinate on GitHub
Before starting, check the [GitHub Issues](https://github.com/HappyAmos/bashmenu-rs/issues) and [Pull Requests](https://github.com/HappyAmos/bashmenu-rs/pulls) to make sure someone else isn't already translating your language.
- Open an issue using our [New Translation Issue Template](.github/ISSUE_TEMPLATE/translation_request.yml) to let the community know you are working on it.

### Step 2: Choose File Name and Copy the Template
Locale files use lowercase ISO language and country codes: `locales/<language>_<country>.yml`.

Examples:
- French (France): `locales/fr_fr.yml`
- German (Germany): `locales/de_de.yml`
- Italian (Italy): `locales/it_it.yml`
- Japanese (Japan): `locales/ja_jp.yml`
- Portuguese (Brazil): `locales/pt_br.yml`
- Chinese (Simplified): `locales/zh_cn.yml`

Copy the canonical English file to create your new locale:
```bash
cp locales/en_us.yml locales/fr_fr.yml
```

### Step 3: Translate the Strings
Open your new file in any text editor (VS Code, Nano, Vim, or `bashedit`).

Translate the values on the right side of each colon (`:`).

#### What to Translate:
```yaml
# English (en_us.yml):
common:
  buttons:
    save: "Save"
    cancel: "Cancel"
    ok: "OK"

# French (fr_fr.yml):
common:
  buttons:
    save: "Enregistrer"
    cancel: "Annuler"
    ok: "OK"
```

#### What NOT to Touch:
1. **Never change the keys (the left-hand side names):**
   ```yaml
   # CORRECT:
   save: "Enregistrer"

   # WRONG (will break the program):
   enregistrer: "Enregistrer"
   ```
2. **Preserve interpolation placeholders (`{...}`):**
   Dynamic placeholders enclosed in curly brackets (such as `{name}`, `{theme}`, `{file}`, `{line}`, `{version}`) are replaced at runtime with real data. Keep them intact in your translation:
   ```yaml
   # English:
   saved_file: "Saved {file}"

   # French:
   saved_file: "{file} enregistré"
   ```
3. **Preserve glyph and icon macros (`{nf:...}`, `{ascii:...}`):**
   If a string includes a Nerd Font glyph or CP437 character macro, leave the macro unchanged:
   ```yaml
   indicator: "{nf:W:f059f:🌐} Web"
   ```

---

## Step 4: Interactive Live Testing in Terminal

You can test your translation live immediately without building from scratch if you have `bashmenu` installed, or using `cargo`:

### Testing the Main Menu
```bash
# If installed on your system:
bashmenu --locale locales/fr_fr.yml

# Or if testing from source with Cargo:
cargo run -p bashmenu-cli -- --locale locales/fr_fr.yml
```

### Testing Companion Tools
All companion tools in the BashMenu suite accept the `--locale <path>` flag:
- **Menu Editor:** `menuedit --locale locales/fr_fr.yml`
- **Theme Editor:** `themeedit --locale locales/fr_fr.yml`
- **Text Editor:** `bashedit --locale locales/fr_fr.yml`
- **Terminal Pager:** `bmpager --locale locales/fr_fr.yml README.md`
- **Cheatsheet Viewer:** `cheat --locale locales/fr_fr.yml`

Verify that:
- Accented and multi-byte characters display cleanly without misaligning borders.
- Modal dialogs wrap properly and buttons remain readable.

---

## Step 5: Automated Validation with `ymlcheck`

BashMenu includes an automated locale verification tool (`ymlcheck`) that checks your translation file against `locales/en_us.yml` to guarantee 100% key parity and valid YAML formatting:

```bash
# Run with installed binary:
ymlcheck -l locales/fr_fr.yml

# Or run from Cargo workspace:
cargo run -p ymlcheck-cli -- -l locales/fr_fr.yml
```

When successful, `ymlcheck` will output:
```text
Validating locale string table: locales/fr_fr.yml...
SUCCESS: 'locales/fr_fr.yml' is a valid locale string table with 100% key parity (311 keys validated)!
```

If any keys are missing or spelled incorrectly, `ymlcheck` will point out the exact keys you need to fix.

---

## Step 6: Submitting Your Translation

Once `ymlcheck -l` passes with 100% parity:

1. Commit your new file:
   ```bash
   git add locales/<language>_<country>.yml
   git commit -m "feat(i18n): add <Language> (<code_code>) translation"
   ```
2. Push your branch and open a Pull Request on GitHub.
3. Our GitHub Actions CI will automatically test and verify your file. Once reviewed, it will be merged into the project release!

---

## Translation FAQ

**Q: Do I need to translate documentation files or comments?**
A: No! Only the strings inside your `locales/<language>_<country>.yml` file are required.

**Q: What if a translated word is much longer than the English word?**
A: BashMenu dialogs and menus feature adaptive layout and automatic marqueeing for overflowing text. However, keeping button labels and menu items concise helps maintain a clean terminal look.

**Q: How do I test in a specific system locale automatically?**
A: When no `--locale` parameter is specified, BashMenu checks the `LANG` environment variable (e.g. `LANG=fr_FR.UTF-8`) and looks for matching locale files.

Thank you for contributing to making terminal tools accessible to users worldwide!
