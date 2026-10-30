use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "bashmenu",
    author,
    version,
    about = "Modern terminal menu system in Rust"
)]
struct Cli {
    /// Path to menu definition file
    #[arg(short, long, default_value = "bashmenu.mnu")]
    menu: String,

    /// Path to configuration file
    #[arg(short, long, default_value = "bashmenu.yml")]
    config: String,
}

fn main() -> anyhow::Result<()> {
    bashmenu_tui::install_panic_hook();
    let _cli = Cli::parse();
    println!("BashMenu Rust initialized");
    Ok(())
}
