use crate::args::{CreateArgs, PageDimensions};
use crate::header::{PayloadHeader, Sha512Array};
use anyhow::{anyhow, Result};
use num_integer::Integer;
use printpdf::Mm;
use qrcode::EcLevel;

/// `LayoutOptions` contains the parameters used for laying out the pages.
#[derive(Debug)]
pub struct Options {
    /// Total width of the page.
    pub page_width: Mm,
    /// Total height of the page.
    pub page_height: Mm,
    pub margin_bottom: Mm,
    pub margin_left: Mm,
    /// The length of one module (pixel in a QR code).
    pub module_length: Mm,
    /// The available width, excluding margins.
    pub avail_width: Mm,
    /// The available height, excluding margins.
    pub avail_height: Mm,

    pub hash: Sha512Array,
    pub version: qrcode::Version,
    pub level: EcLevel,
    /// The number of QR codes per row / column.
    pub chunks_per_row: usize,
    /// The number of data bytes stored per QR code, excluding header.
    pub bytes_per_chunk: usize,
    /// The total number of data chunks; this is never emitted.
    pub data_chunk_count: usize,
    /// The total number of recovery chunks; this is a multiple of `chunks_per_row` squared, and is
    /// the number of chunks actually printed.
    pub recovery_chunk_count: usize,
    /// The minimum number of pages needed to recover data.
    pub data_page_count: usize,
    /// The number of total pages.
    pub recovery_page_count: usize,
}

/// Compute layout options.
pub fn compute(args: &CreateArgs, data_size: usize, data_hash: Sha512Array) -> Result<Options> {
    let page: PageDimensions = args.paper_size.into();
    let avail_width = page.width - args.margin_left - args.margin_right;
    let avail_height = page.height - args.margin_top - args.margin_bottom;
    let avail_min = std::cmp::min(avail_width, avail_height);
    // Width of a quiet zone
    let quiet_zone_width = args.module_length * 4.0;

    // Compute the best QR code parameters to use: within the constraints of the minimum number of
    // codes per row and minimum error correction level (as found in `args`), calculate the maximum
    // amount of data we can fit into one page, and pick the highest value.
    let mut best_bytes_per_page = 0;
    let mut best_version = qrcode::Version::Normal(1);
    let mut best_ec_level = EcLevel::L;
    let mut best_chunks_per_row = 0;
    let mut best_chunk_bytes = 0;
    for version_value in 1..=40 {
        let version = qrcode::Version::Normal(version_value);
        // Width per QR code, with one side of quiet zone.
        let width_per_chunk = args.module_length * (version.width() + 4).into();
        let chunks_per_row = ((avail_min + quiet_zone_width) / width_per_chunk).floor() as usize;
        if chunks_per_row < args.row_count {
            continue;
        }
        for ec_level in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            if ec_level < args.error_correction {
                continue;
            }
            // Number of bits that can be stored in the QR code.
            let Ok(bits) = qrcode::bits::Bits::new(version).max_len(ec_level) else {
                continue;
            };
            // Number of bits taken by the count.
            let char_count = match version_value {
                1..=9 => 8,
                10..=40 => 16,
                _ => continue,
            };
            // Number of bytes available for data in the QR code.
            let raw_byte_count = (bits - char_count) / 8;
            let header_size = size_of::<PayloadHeader>();
            let available_bytes = raw_byte_count - header_size;
            let num_blocks = available_bytes / 64;
            let num_bytes_per_chunk = num_blocks * 64;
            let num_bytes_per_page = num_bytes_per_chunk * chunks_per_row * chunks_per_row;
            if num_bytes_per_page > best_bytes_per_page {
                best_bytes_per_page = num_bytes_per_page;
                best_version = version;
                best_ec_level = ec_level;
                best_chunks_per_row = chunks_per_row;
                best_chunk_bytes = num_bytes_per_chunk;
            }
        }
    }

    if best_bytes_per_page == 0 {
        Err(anyhow!(
            "Could not find QR code configuration that holds enough data"
        ))
    } else {
        let bytes_per_chunk = (best_chunk_bytes - PayloadHeader::LENGTH).prev_multiple_of(&64);
        let chunks_per_page = best_chunks_per_row * best_chunks_per_row;
        // The buffer needs to be resized to append the original file size, so we need to grow it
        // a bit.
        let buffer_size = (size_of::<u64>() + data_size).next_multiple_of(bytes_per_chunk);
        let data_chunk_count = buffer_size.div_ceil(bytes_per_chunk);
        let data_page_count = data_chunk_count.div_ceil(chunks_per_page);
        let recovery_page_count = data_page_count
            + match args.recovery_factor {
                crate::args::RecoveryFactor::Pages(c) => c,
                crate::args::RecoveryFactor::Percentage(p) => {
                    ((p / 100.0 * data_chunk_count as f32) as usize).div_ceil(chunks_per_page)
                }
            };
        Ok(Options {
            page_width: page.width,
            page_height: page.height,
            margin_bottom: args.margin_bottom,
            margin_left: args.margin_left,
            module_length: args.module_length,
            avail_width: page.width - args.margin_left - args.margin_right,
            avail_height: page.height - args.margin_top - args.margin_bottom,

            hash: data_hash,
            version: best_version,
            level: best_ec_level,
            chunks_per_row: best_chunks_per_row,
            bytes_per_chunk,
            data_chunk_count,
            recovery_chunk_count: recovery_page_count * chunks_per_page,
            data_page_count,
            recovery_page_count,
        })
    }
}
