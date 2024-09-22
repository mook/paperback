use anyhow::Result;
use vergen_gix::{Emitter, GixBuilder};

fn run() -> Result<()> {
    let gix = GixBuilder::default()
        .describe(true, true, None)
        .sha(true)
        .build()?;

    Emitter::default().add_instructions(&gix)?.emit()?;

    Ok(())
}

fn main() {
    run().unwrap();
}
