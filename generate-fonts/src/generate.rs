use super::metrics::FontMetrics;
use anyhow::Result;
use std::io::Write;
use std::{collections::HashMap, env, fs, path};
/// Generate font metrics.
pub fn generate_metrics() -> Result<()> {
    let mut metrics = HashMap::<String, FontMetrics>::new();
    fs::read_dir("generate-fonts")?
        .map(|r| r.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?
        .iter()
        .inspect(|f| println!("{}", f.path().display()))
        .filter(|entry| entry.path().extension().is_some_and(|f| f == "afm"))
        .filter(|entry| entry.file_type().is_ok_and(|f| f.is_file()))
        .map(|entry| {
            let metric = FontMetrics::from_file(entry.path())?;
            metrics.insert(metric.identifier(), metric);
            Ok(())
        })
        .collect::<Result<Vec<_>>>()?;

    let out_dir = fs::canonicalize(path::Path::new(&env::var("OUT_DIR")?))?;
    let mut out_file = fs::File::create(out_dir.join("metrics-generated.rs"))?;

    writeln!(out_file, "use crate::fonts::metrics::FontMetrics;")?;
    writeln!(out_file, "use std::collections::HashMap;")?;
    writeln!(out_file, "use std::sync::LazyLock;")?;
    writeln!(out_file)?;

    for (identifier, metric) in &metrics {
        writeln!(
            out_file,
            "static {identifier}: LazyLock<FontMetrics> = LazyLock::new(|| {{"
        )?;
        metric.to_source(&out_file)?;
        writeln!(out_file, "}});")?;
    }

    writeln!(out_file)?;
    writeln!(
        out_file,
        "pub(crate) fn from(font: printpdf::BuiltinFont) -> &'static FontMetrics {{"
    )?;
    writeln!(out_file, "  match font {{")?;
    for (identifier, metrics) in &metrics {
        writeln!(
            out_file,
            "    printpdf::BuiltinFont::{} => &{identifier},",
            metrics.name
        )?;
    }
    writeln!(out_file, "  }}")?;
    writeln!(out_file, "}}")?;

    println!("File generated to {}", out_dir.display());
    Ok(())
}
