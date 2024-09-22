use anyhow::Result;
use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

/// Take the LICENSE file, encode it as PDF, and restore it, checking that the result is correct.
#[test]
fn test_license() -> Result<()> {
    let work_dir = assert_fs::TempDir::new()?;
    let pdf_file = work_dir.child("output.pdf");
    let output_file = work_dir.child("output.bin");

    // Generate the PDF
    println!("Generating PDF {:?}...", pdf_file.as_os_str());
    Command::cargo_bin("paperback")?
        .arg("create")
        .arg("--module-length=0.5")
        .arg("LICENSE")
        .arg(pdf_file.as_os_str())
        .assert()
        .try_success()?;

    // Convert to PNGs
    println!("Converting PDF to PNGs...");
    Command::new("pdftocairo")
        .current_dir(work_dir.path())
        .arg("-png")
        .arg(pdf_file.as_os_str())
        .assert()
        .try_success()?;

    // Restore the output
    let images = work_dir
        .read_dir()?
        .map(|d| d.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;
    let mut image_names = images
        .iter()
        .map(|d| d.path())
        .filter(|n| n.extension().is_some_and(|ext| ext.eq("png")))
        .peekable();
    assert!(image_names.peek().is_some());
    Command::cargo_bin("paperback")?
        .arg("restore")
        .arg(output_file.path().as_os_str())
        .args(image_names)
        .assert()
        .try_success()?;

    // Check that the file is correctly restored.
    output_file.assert(predicate::path::eq_file("LICENSE"));

    Ok(())
}
