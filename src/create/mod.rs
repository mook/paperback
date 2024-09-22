mod layout;
mod render;
use crate::{args::CreateArgs, header};
use anyhow::{anyhow, Result};
use byteorder::{ByteOrder, LittleEndian};
use chksum_sha2_512::SHA2_512;
use itertools::Itertools;
use printpdf::PdfDocument;
use qrcode::QrCode;
use rayon::prelude::*;
use reed_solomon_simd::ReedSolomonEncoder;
use std::{ffi::OsStr, fs, io::BufWriter};

pub(crate) fn create(args: &CreateArgs) -> Result<()> {
    // Read the file (into memory, for now)
    let mut data_bytes = fs::read(&args.file_path)
        .map_err(|e| anyhow!("Failed to read {:?}: {}", &args.file_path, e))?;
    let data_size = u64::try_from(data_bytes.len())
        .map_err(|e| anyhow!("{:?} is too large: {e}", &args.file_path))?;
    let mut hasher = SHA2_512::new();
    hasher.update(&data_bytes);
    hasher.update(&args.override_commit);
    let digest = hasher.digest().into_inner();

    // Calculate the layout parameters.
    let layout = layout::compute(args, data_bytes.len(), digest)?;

    // Given the QR code info, resize the data to have the actual size appended.  This is necessary
    // so that we can avoid having trailing null bytes at the end after decode.
    let buffer_size =
        (size_of::<u64>() + data_bytes.len()).next_multiple_of(layout.bytes_per_chunk);
    data_bytes.resize(buffer_size, 0);
    LittleEndian::write_u64(&mut data_bytes[buffer_size - size_of::<u64>()..], data_size);

    // Compute the reed-solomon shards.
    let mut rs_encoder = ReedSolomonEncoder::new(
        layout.data_chunk_count,
        layout.recovery_chunk_count,
        layout.bytes_per_chunk,
    )?;
    for index in 0..layout.data_chunk_count {
        rs_encoder.add_original_shard(
            &data_bytes[index * layout.bytes_per_chunk..(index + 1) * layout.bytes_per_chunk],
        )?;
    }

    // Encode the reed-solomon shards into QR codes.
    let chunks_per_page = layout.chunks_per_row * layout.chunks_per_row;
    let mut svgs = rs_encoder
        .encode()?
        .recovery_iter()
        .collect::<Vec<_>>()
        .par_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let header = header::Header::Payload(header::PayloadHeader {
                index: i.try_into()?,
                identifier: layout.hash[0..header::IDENTIFIER_LENGTH].try_into()?,
            });
            let mut buf =
                Vec::<u8>::with_capacity(header::PayloadHeader::LENGTH + layout.bytes_per_chunk);
            header.write_to(&mut buf)?;
            buf.extend_from_slice(chunk);
            // We need to convert the QR code into an SVG, and then parse it _back_ into an
            // object...
            let svg_string = QrCode::with_version(buf, layout.version, layout.level)?
                .render::<qrcode::render::svg::Color>()
                .quiet_zone(false)
                .module_dimensions(1, 1)
                .build();
            Ok(printpdf::svg::Svg::parse(&svg_string)?)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let svg_chunks = svgs.drain(..).chunks(chunks_per_page);

    // Set up the PDF document.
    let (doc, mut page_index, mut layer_index) = PdfDocument::new(
        args.file_path
            .file_name()
            .unwrap_or(OsStr::new("PaperBack"))
            .to_str()
            .unwrap_or("PaperBack"),
        layout.page_width,
        layout.page_height,
        "",
    );

    // Fill in the PDF pages.  The PDF references don't implement Send, so we can't work with them
    // in parallel here.
    for (page_num, mut page_svgs) in svg_chunks.into_iter().enumerate() {
        if page_num > 0 {
            (page_index, layer_index) = doc.add_page(layout.page_width, layout.page_height, "");
        }
        let page = doc.get_page(page_index);
        let layer = page.get_layer(layer_index);
        render::render_page(
            &layout,
            &mut page_svgs,
            page_num,
            &doc,
            &layer,
            &args.override_commit,
        )?;
    }

    doc.save(&mut BufWriter::new(fs::File::create(
        args.out_path.clone(),
    )?))?;

    Ok(())
}
