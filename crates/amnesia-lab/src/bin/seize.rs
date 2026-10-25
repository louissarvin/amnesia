//! seize IMAGE_DIR RECORDS_CSV [--json]
//! Runs the seizure attack on a raw directory image with an exchange withdrawal log.

use std::path::PathBuf;

use amnesia_lab::{analyzer, records};
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let image = PathBuf::from(
        args.first()
            .context("usage: seize IMAGE_DIR RECORDS_CSV [--json]")?,
    );
    let csv = std::fs::read_to_string(args.get(1).context("missing RECORDS_CSV")?)?;
    let rows = records::parse_csv(&csv);
    let report = analyzer::analyze(&image, &rows)?;
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!(
            "{}",
            analyzer::summary(&image.display().to_string(), &report)
        );
    }
    Ok(())
}
