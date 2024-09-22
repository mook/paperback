use anyhow::Result;
use clap::{builder::TypedValueParser, value_parser, Parser, ValueEnum};
use clap::{Args, Subcommand};
use printpdf::Mm;
use qrcode::EcLevel;
use std::path::PathBuf;
use std::str::FromStr;

/// `ECLevel` is a wrapper for [`qrcode::EcLevel`]; this is only needed to let [`clap`] build the
/// necessary parsers.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ECLevel {
    L,
    M,
    Q,
    H,
}

impl From<ECLevel> for qrcode::EcLevel {
    fn from(val: ECLevel) -> Self {
        match val {
            ECLevel::L => qrcode::EcLevel::L,
            ECLevel::M => qrcode::EcLevel::M,
            ECLevel::Q => qrcode::EcLevel::Q,
            ECLevel::H => qrcode::EcLevel::H,
        }
    }
}

/// `mm_value_parser` is used for implementing [`clap::builder::TypedValueParser`] into lengths.
fn mm_value_parser(s: &str) -> Result<Mm> {
    Ok(Mm(f32::from_str(s)?))
}

/// Paper size options.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum PaperSize {
    A4,
    Letter,
}

/// Describe the dimensions of a sheet of paper.
pub(crate) struct PageDimensions {
    pub width: Mm,
    pub height: Mm,
}

impl From<PaperSize> for PageDimensions {
    fn from(value: PaperSize) -> Self {
        match value {
            PaperSize::A4 => PageDimensions {
                width: Mm(210.0),
                height: Mm(297.0),
            },
            PaperSize::Letter => PageDimensions {
                width: Mm(215.9),
                height: Mm(279.4),
            },
        }
    }
}

/// How much recovery to generate, so that we do not need the whole set of pages to restore.
#[derive(Clone, Debug)]
pub(crate) enum RecoveryFactor {
    /// Recovery factor as a percentage, e.g. "33%" or "500%".
    Percentage(f32),
    /// Recovery factor as a number of pages, e.g. "3".
    Pages(usize),
}

impl FromStr for RecoveryFactor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Some(percent) = s.strip_suffix("%") {
            Ok(Self::Percentage(f32::from_str(percent)?))
        } else if let Some(multiple) = s.strip_suffix("x") {
            Ok(Self::Percentage(f32::from_str(multiple)? * 100.0))
        } else {
            Ok(Self::Pages(usize::from_str(s)?))
        }
    }
}

/// Arguments for creating documents.
#[derive(Args, Debug)]
pub(crate) struct CreateArgs {
    /// File to encode.
    #[arg(value_hint=clap::ValueHint::FilePath)]
    pub file_path: PathBuf,

    /// Output file to write to.
    pub out_path: PathBuf,

    /// Minimum number of QR codes per row.
    #[arg(short, long, default_value = "3", help_heading = "Layout")]
    pub row_count: usize,

    /// How much extra recovery data to generate; may be one of:
    /// - A percentage relative to required data, e.g. "50%"
    /// - A positive integer followed by "x" (e.g. "3x") to mean that multiple of required data
    /// - A positive integer, giving a number of pages independent of input.
    #[arg(short = 'R', long, default_value = "50%", help_heading = "Layout")]
    pub recovery_factor: RecoveryFactor,

    /// Minimim QR code error correction level.
    #[arg(short, long, value_parser=value_parser!(ECLevel).map(|v| Into::<EcLevel>::into(v)), default_value = "q", help_heading = "Layout")]
    pub error_correction: EcLevel,

    /// Width of one module (pixel) in a QR code; larger values are easier to read.
    #[arg(short, long, value_parser=mm_value_parser, default_value="1.0", help_heading="Layout")]
    pub module_length: Mm,

    /// Paper size to emit.
    #[arg(
        short,
        long,
        value_enum,
        default_value = "a4",
        help_heading = "Page Setup"
    )]
    pub paper_size: PaperSize,

    /// Paper top margin.
    #[arg(long, value_parser=mm_value_parser, default_value="4.32", help_heading="Page Setup")]
    pub margin_top: Mm,
    /// Paper right margin.
    #[arg(long, value_parser=mm_value_parser, default_value="4.32", help_heading="Page Setup")]
    pub margin_right: Mm,
    /// Paper bottom margin.
    #[arg(long, value_parser=mm_value_parser, default_value="4.32", help_heading="Page Setup")]
    pub margin_bottom: Mm,
    /// Paper left margin.
    #[arg(long, value_parser=mm_value_parser, default_value="4.32", help_heading="Page Setup")]
    pub margin_left: Mm,

    /// Override the commit ID displayed in the document.  This is used to ensure we can get
    /// reproducible output for the sample PDF.
    #[arg(long, hide=true, default_value=match env!("VERGEN_GIT_DESCRIBE") {
        "" => env!("VERGEN_GIT_SHA"),
        v => v,
    })]
    pub override_commit: String,
}

/// Arguments for restoring documents.
#[derive(Args, Debug)]
pub(crate) struct RestoreArgs {
    /// Output file to write to.
    pub output_path: PathBuf,

    /// Input files to restore from.  They must be images, but can contain multiple QR codes per
    /// image.
    #[arg(value_hint=clap::ValueHint::FilePath)]
    pub input_path: Vec<PathBuf>,

    /// Overwrite any existing output file.
    #[arg(long, short)]
    pub force: bool,

    /// Override the commit ID displayed in the document.  This is used to ensure we can get
    /// reproducible output for the sample PDF.
    #[arg(long, hide=true, default_value=match env!("VERGEN_GIT_DESCRIBE") {
        "" => env!("VERGEN_GIT_SHA"),
        v => v,
    })]
    pub override_commit: String,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Create PDFs from an input file.
    Create(CreateArgs),
    /// Restore a file from scanned PDFs.
    Restore(RestoreArgs),
}

#[derive(Parser)]
#[command(version)]
pub(crate) struct TopLevelArgs {
    #[command(subcommand)]
    pub(crate) command: Commands,
}
