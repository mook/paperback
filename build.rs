use anyhow::Result;
use paperback_generate_fonts::generate_metrics;
use vergen_gix::{Emitter, GixBuilder};

fn run() -> Result<()> {
    let gix = GixBuilder::default()
        .describe(true, true, None)
        .sha(true)
        .build()?;

    Emitter::default().add_instructions(&gix)?.emit()?;

    generate_metrics()?;

    Ok(())
}

fn main() {
    run().unwrap();
}
