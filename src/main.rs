mod args;
mod create;
mod fonts;
mod header;
mod restore;
use args::Commands;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    match args::TopLevelArgs::parse().command {
        Commands::Create(args) => {
            create::create(&args)?;
        }
        Commands::Restore(args) => {
            restore::restore(&args)?;
        }
    }

    Ok(())
}
