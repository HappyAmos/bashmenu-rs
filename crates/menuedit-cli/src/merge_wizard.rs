//! Interactive CLI wizard for merging user menus with upstream vanilla templates.

use anyhow::{bail, Context, Result};
use bashmenu_core::{
    apply_merge, compute_menu_diff, load_menu, save_menu, ConflictChoice, MergeDecisions,
};
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::Path;

/// Terminal styling helpers using standard ANSI escapes if stdout is a terminal.
struct Styler {
    is_tty: bool,
}

impl Styler {
    fn new() -> Self {
        Self {
            is_tty: io::stdout().is_terminal(),
        }
    }

    fn cyan(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;36m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    fn green(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;32m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    fn yellow(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1;33m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }

    fn bold(&self, text: &str) -> String {
        if self.is_tty {
            format!("\x1b[1m{}\x1b[0m", text)
        } else {
            text.to_string()
        }
    }
}

/// Reads a single trimmed line from standard input.
fn prompt_line(prompt_text: &str) -> Result<String> {
    print!("{}", prompt_text);
    io::stdout().flush().context("Failed to flush stdout")?;
    let mut line = String::new();
    let stdin = io::stdin();
    stdin
        .lock()
        .read_line(&mut line)
        .context("Failed to read from stdin")?;
    Ok(line.trim().to_string())
}

/// Executes the interactive menu merge wizard.
pub fn run_merge_wizard(
    user_menu_path: &Path,
    vanilla_menu_path: &Path,
    output_path: &Path,
    backup_path: Option<&Path>,
    force_non_interactive: bool,
) -> Result<()> {
    let styler = Styler::new();

    println!(
        "{}",
        styler.bold("===================================================================")
    );
    println!(
        "{}",
        styler.cyan("               BashMenu Interactive Menu Merge Wizard              ")
    );
    println!(
        "{}",
        styler.bold("===================================================================")
    );
    println!("  User Menu:    {}", user_menu_path.display());
    println!("  Vanilla Menu: {}", vanilla_menu_path.display());
    println!("  Target:       {}", output_path.display());
    if let Some(bak) = backup_path {
        println!("  Backup:       {}", bak.display());
    }
    println!();

    if !user_menu_path.exists() {
        println!("  [*] User menu not found. Copying vanilla template directly...");
        if !vanilla_menu_path.exists() {
            bail!(
                "Vanilla menu template not found at {}",
                vanilla_menu_path.display()
            );
        }
        let vanilla_menu = load_menu(vanilla_menu_path)?;
        save_menu(output_path, &vanilla_menu)?;
        println!(
            "  {} Copied vanilla menu -> {}",
            styler.green("[✓]"),
            output_path.display()
        );
        return Ok(());
    }

    if !vanilla_menu_path.exists() {
        bail!(
            "Vanilla menu template not found at {}",
            vanilla_menu_path.display()
        );
    }

    let user_menu = load_menu(user_menu_path)
        .with_context(|| format!("Failed to parse user menu: {}", user_menu_path.display()))?;
    let vanilla_menu = load_menu(vanilla_menu_path).with_context(|| {
        format!(
            "Failed to parse vanilla menu: {}",
            vanilla_menu_path.display()
        )
    })?;

    let plan = compute_menu_diff(&user_menu, &vanilla_menu);

    if plan.new_items.is_empty() && plan.conflicts.is_empty() {
        println!("  {} Your menu is already up-to-date with the latest upstream template. No changes needed.", styler.green("[✓]"));
        return Ok(());
    }

    let non_interactive = force_non_interactive || !io::stdin().is_terminal();
    let mut decisions = MergeDecisions::default();

    // -----------------------------------------------------------------------
    // STAGE 1: New Upstream Items
    // -----------------------------------------------------------------------
    if !plan.new_items.is_empty() {
        println!(
            "{}",
            styler.bold("-------------------------------------------------------------------")
        );
        println!(
            "  {} ({} detected)",
            styler.yellow("[STAGE 1/2] New Upstream Features & Additions"),
            plan.new_items.len()
        );
        println!(
            "{}",
            styler.bold("-------------------------------------------------------------------")
        );

        let mut accept_all = non_interactive;

        for (idx, addition) in plan.new_items.iter().enumerate() {
            let key = MergeDecisions::path_key(&addition.path);
            let item = &addition.item;

            println!(
                "\n[{}/{}] {}",
                idx + 1,
                plan.new_items.len(),
                styler.bold(&key)
            );
            println!("      Type:   {:?}", item.r#type);
            if let Some(ref target) = item
                .action
                .as_ref()
                .or(item.command.as_ref())
                .or(item.script.as_ref())
                .or(item.file.as_ref())
            {
                println!("      Target: {}", target);
            }
            if let Some(ref anchor) = addition.predecessor_anchor {
                println!("      Anchor: Placed below '{}'", anchor);
            }

            if accept_all {
                decisions.additions.insert(key, true);
                println!("      -> Accepted addition.");
                continue;
            }

            loop {
                let ans = prompt_line(
                    "      -> Add this item to your menu? [Y/n/a(all)/q(quit)] (Default: Y): ",
                )?;
                match ans.to_ascii_lowercase().as_str() {
                    "y" | "yes" | "" => {
                        decisions.additions.insert(key, true);
                        println!("      -> Accepted.");
                        break;
                    }
                    "n" | "no" => {
                        decisions.additions.insert(key, false);
                        println!("      -> Skipped.");
                        break;
                    }
                    "a" | "all" => {
                        decisions.additions.insert(key, true);
                        accept_all = true;
                        println!("      -> Accepted this and all remaining additions.");
                        break;
                    }
                    "q" | "quit" => {
                        println!("\n  [!] Merge aborted by user. No files were modified.");
                        return Ok(());
                    }
                    _ => {
                        println!("      Invalid option. Please enter Y, n, a, or q.");
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // STAGE 2: Conflicting & Modified Items
    // -----------------------------------------------------------------------
    if !plan.conflicts.is_empty() {
        println!(
            "\n{}",
            styler.bold("-------------------------------------------------------------------")
        );
        println!(
            "  {} ({} detected)",
            styler.yellow("[STAGE 2/2] Conflicting & Modified Items"),
            plan.conflicts.len()
        );
        println!(
            "{}",
            styler.bold("-------------------------------------------------------------------")
        );

        for (idx, conflict) in plan.conflicts.iter().enumerate() {
            let key = MergeDecisions::path_key(&conflict.path);
            println!(
                "\n[{}/{}] Conflict on item: {}",
                idx + 1,
                plan.conflicts.len(),
                styler.bold(&key)
            );
            println!("  Differences detected between your version and upstream vanilla:");
            for diff in &conflict.property_diffs {
                println!("    • Property '{}':", styler.cyan(&diff.property_name));
                println!("        Your Menu:        {}", diff.user_value);
                println!("        Upstream Vanilla: {}", diff.vanilla_value);
            }

            if non_interactive {
                decisions.conflicts.insert(key, ConflictChoice::KeepUser);
                println!("  [*] Non-interactive mode: Kept your version.");
                continue;
            }

            println!("\n  How would you like to resolve this item?");
            println!("    [1] Keep my version (Recommended - preserves your customization)");
            println!("    [2] Take upstream vanilla version");
            println!("    [3] Keep both (Keep yours and append upstream vanilla version)");
            println!("    [q] Abort merge");

            loop {
                let ans = prompt_line("  Select [1-3, q] (Default: 1): ")?;
                match ans.as_str() {
                    "1" | "" => {
                        decisions.conflicts.insert(key, ConflictChoice::KeepUser);
                        println!("  -> Kept your version.");
                        break;
                    }
                    "2" => {
                        decisions.conflicts.insert(key, ConflictChoice::TakeVanilla);
                        println!("  -> Selected upstream vanilla version.");
                        break;
                    }
                    "3" => {
                        decisions.conflicts.insert(key, ConflictChoice::KeepBoth);
                        println!("  -> Keeping both items.");
                        break;
                    }
                    "q" | "Q" => {
                        println!("\n  [!] Merge aborted by user. No files were modified.");
                        return Ok(());
                    }
                    _ => {
                        println!("  Invalid choice. Please select 1, 2, 3, or q.");
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // STAGE 3: Review & Confirmation
    // -----------------------------------------------------------------------
    let accepted_adds = decisions.additions.values().filter(|&&v| v).count();
    let skipped_adds = decisions.additions.values().filter(|&&v| !v).count();
    let kept_user = decisions
        .conflicts
        .values()
        .filter(|&&c| c == ConflictChoice::KeepUser)
        .count();
    let took_vanilla = decisions
        .conflicts
        .values()
        .filter(|&&c| c == ConflictChoice::TakeVanilla)
        .count();
    let kept_both = decisions
        .conflicts
        .values()
        .filter(|&&c| c == ConflictChoice::KeepBoth)
        .count();

    println!(
        "\n{}",
        styler.bold("===================================================================")
    );
    println!(
        "{}",
        styler.cyan("                     Merge Decision Summary                        ")
    );
    println!(
        "{}",
        styler.bold("===================================================================")
    );
    println!("  [+] Upstream additions accepted: {}", accepted_adds);
    if skipped_adds > 0 {
        println!("  [-] Upstream additions skipped:  {}", skipped_adds);
    }
    if !plan.conflicts.is_empty() {
        println!(
            "  [*] Conflicts resolved:          {}",
            plan.conflicts.len()
        );
        println!("        • Kept user customizations: {}", kept_user);
        println!("        • Took upstream vanilla:    {}", took_vanilla);
        println!("        • Kept both versions:       {}", kept_both);
    }
    println!(
        "{}",
        styler.bold("===================================================================")
    );

    if !non_interactive {
        let confirm = prompt_line(&format!(
            "\nApply these changes to '{}'? [Y/n] (Default: Y): ",
            output_path.display()
        ))?;
        if matches!(
            confirm.to_ascii_lowercase().as_str(),
            "n" | "no" | "q" | "quit"
        ) {
            println!("\n  [!] Merge canceled by user. No files were modified.");
            return Ok(());
        }
    }

    // Backup existing file if requested
    if let Some(bak) = backup_path {
        if user_menu_path.exists() {
            if let Some(parent) = bak.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::copy(user_menu_path, bak)
                .with_context(|| format!("Failed to create backup at {}", bak.display()))?;
            println!(
                "  {} Backed up existing menu -> {}",
                styler.green("[✓]"),
                bak.display()
            );
        }
    }

    // Synthesize and write merged menu
    let merged_menu = apply_merge(&user_menu, &vanilla_menu, &decisions);

    if let Some(parent) = output_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    save_menu(output_path, &merged_menu)
        .with_context(|| format!("Failed to save merged menu to {}", output_path.display()))?;

    println!(
        "  {} Successfully merged menu saved to {}",
        styler.green("[✓]"),
        output_path.display()
    );
    Ok(())
}
