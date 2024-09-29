use crate::args::{CreateArgs, PageDimensions};
use crate::header::{Identifier, PayloadHeader, Sha512Array};
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

    pub identifier: Identifier,
    pub hash: Sha512Array,
    pub version: qrcode::Version,
    pub level: EcLevel,
    /// The number of QR codes per row / column.
    pub shards_per_row: usize,
    /// The number of data bytes stored per QR code, excluding header.
    pub data_bytes_per_shard: usize,
    /// The total number of data shards; this is never emitted.
    pub data_shard_count: usize,
    /// The total number of recovery shards; this is a multiple of `shards_per_row` squared, and is
    /// the number of shards actually printed.
    pub recovery_shard_count: usize,
    /// The minimum number of pages needed to recover data.
    pub data_page_count: usize,
    /// The number of total pages.
    pub recovery_page_count: usize,
}

/// Compute layout options.
pub fn compute(
    args: &CreateArgs,
    data_size: usize,
    identifier: Identifier,
    data_hash: Sha512Array,
) -> Result<Options> {
    let page: PageDimensions = args.paper_size.into();
    let avail_width = page.width - args.margin_left - args.margin_right;
    let avail_height = page.height - args.margin_top - args.margin_bottom;
    let avail_min = std::cmp::min(avail_width, avail_height);
    // Width of a quiet zone
    let quiet_zone_width = args.module_length * 4.0;

    // Compute the best QR code parameters to use: within the constraints of the minimum number of
    // codes per row and minimum error correction level (as found in `args`), calculate the maximum
    // amount of data we can fit into one page, and pick the highest value.
    let mut best_data_bytes_per_page = 0;
    let mut best_version = qrcode::Version::Normal(1);
    let mut best_ec_level = EcLevel::L;
    let mut best_shards_per_row = 0;
    let mut best_data_bytes_per_shard = 0;
    for version_value in 1..=40 {
        let version = qrcode::Version::Normal(version_value);
        // Width per QR code, with one side of quiet zone.
        let width_per_shard = args.module_length * (version.width() + 4).into();
        let shards_per_row = ((avail_min - quiet_zone_width) / width_per_shard).floor() as usize;
        if shards_per_row < args.row_count {
            continue;
        }
        // Try for the most error correction first, if we end up with the same number of bytes
        // per page.
        for ec_level in [EcLevel::H, EcLevel::Q, EcLevel::M, EcLevel::L] {
            if ec_level < args.error_correction {
                continue;
            }
            // Number of bits that can be stored in the QR code.
            let Ok(bits) = qrcode::bits::Bits::new(version).max_len(ec_level) else {
                continue;
            };
            // Number of bits taken by the mode indicator.
            let mode_indicator_length = 4;
            // Number of bits taken by the character count indicator.
            let char_count_length = match version_value {
                1..=9 => 8,
                10..=40 => 16,
                _ => continue,
            };
            // Number of bytes available for data in the QR code.
            let raw_byte_count = (bits - mode_indicator_length - char_count_length) / 8;
            let data_bytes_per_shard = raw_byte_count - PayloadHeader::LENGTH;
            let data_bytes_per_page =
                data_bytes_per_shard.prev_multiple_of(&64) * shards_per_row * shards_per_row;
            if data_bytes_per_page > best_data_bytes_per_page {
                best_data_bytes_per_page = data_bytes_per_page;
                best_version = version;
                best_ec_level = ec_level;
                best_shards_per_row = shards_per_row;
                best_data_bytes_per_shard = data_bytes_per_shard;
            }
        }
    }

    if best_data_bytes_per_shard < 64 + PayloadHeader::LENGTH {
        Err(anyhow!(
            "Could not find QR code configuration that holds enough data; try lowering row-count"
        ))
    } else {
        let data_bytes_per_shard = best_data_bytes_per_shard.prev_multiple_of(&64);
        let shards_per_page = best_shards_per_row * best_shards_per_row;
        // The buffer needs to be resized to append the original file size, so we need to grow it
        // a bit.
        let buffer_size = (size_of::<u64>() + data_size).next_multiple_of(data_bytes_per_shard);
        let data_shard_count = buffer_size.div_ceil(data_bytes_per_shard);
        let data_page_count = data_shard_count.div_ceil(shards_per_page);
        let recovery_page_count = data_page_count
            + match args.recovery_factor {
                crate::args::RecoveryFactor::Pages(c) => c,
                crate::args::RecoveryFactor::Percentage(p) => {
                    ((p / 100.0 * data_shard_count as f32) as usize).div_ceil(shards_per_page)
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

            identifier,
            hash: data_hash,
            version: best_version,
            level: best_ec_level,
            shards_per_row: best_shards_per_row,
            data_bytes_per_shard,
            data_shard_count,
            recovery_shard_count: recovery_page_count * shards_per_page,
            data_page_count,
            recovery_page_count,
        })
    }
}
