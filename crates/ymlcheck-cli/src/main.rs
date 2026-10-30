use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "ymlcheck",
    author,
    version,
    about = "YAML validator and linter for BashMenu"
)]
struct Cli {
    /// File to check
    file: Option<String>,

    /// Check theme definitions
    #[arg(short, long)]
    themes: Option<String>,

    /// Check menu configuration
    #[arg(short, long)]
    config: Option<String>,

    /// Check menu items definition
    #[arg(short, long)]
    menu: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("ymlcheck initialized");
    Ok(())
}
