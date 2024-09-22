use crate::{
    args::RestoreArgs,
    header::{self, Header, IDENTIFIER_LENGTH},
};
use anyhow::{anyhow, Context, Result};
use byteorder::{ByteOrder, LittleEndian};
use chksum_sha2_512::SHA2_512;
use rayon::prelude::*;
use reed_solomon_simd::ReedSolomonDecoder;
use rxing::{
    common::HybridBinarizer,
    multi::{GenericMultipleBarcodeReader, MultipleBarcodeReader},
    BarcodeFormat, BinaryBitmap, BufferedImageLuminanceSource,
    DecodeHintType::{POSSIBLE_FORMATS, TRY_HARDER},
    DecodeHintValue::{PossibleFormats, TryHarder},
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// `IntoFlatIter` is a helper to make the return type of [`read_chunks`] easier to read.
struct IntoFlatIter<T> {
    value: Vec<Vec<T>>,
}

impl<T> IntoFlatIter<T> {
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.value.iter().flatten()
    }
}

/// `read_chunks` reads the given files, returning scanned QR codes.
fn read_chunks(input_paths: &Vec<PathBuf>) -> Result<IntoFlatIter<rxing::RXingResult>> {
    let chunk_list = input_paths
        .par_iter()
        .map(|input_path| -> anyhow::Result<Vec<_>> {
            let image = image::open(input_path)?;
            let bitmap = &mut BinaryBitmap::new(HybridBinarizer::new(
                BufferedImageLuminanceSource::new(image),
            ));
            let reader = rxing::MultiUseMultiFormatReader::default();
            let mut scanner = GenericMultipleBarcodeReader::new(reader);
            let results = scanner.decode_multiple_with_hints(
                bitmap,
                &rxing::DecodingHintDictionary::from([
                    (
                        POSSIBLE_FORMATS,
                        PossibleFormats(vec![BarcodeFormat::QR_CODE].into_iter().collect()),
                    ),
                    (TRY_HARDER, TryHarder(true)),
                ]),
            )?;
            Ok(results)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(IntoFlatIter { value: chunk_list })
}

/// Given the reed-solomon recovery chunks, reconstruct the file and write it to the given name.
/// If `force` is not set, this will return an error if the file already exists.
fn write_output<P>(
    meta: &header::MetaHeader,
    payloads: &Vec<(u16, Vec<u8>)>,
    force: bool,
    output_path: P,
    commit: &str,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut rs_decoder = ReedSolomonDecoder::new(
        meta.original_count as usize,
        meta.recovery_count as usize,
        meta.shard_bytes as usize,
    )?;
    for (index, data) in payloads {
        rs_decoder.add_recovery_shard(*index as usize, data)?;
    }

    let decoder_result = rs_decoder
        .decode()
        .with_context(|| "failed to decode original data")?;
    let decoded = decoder_result
        .restored_original_iter()
        .map(|(_, chunk)| chunk)
        .collect::<Vec<_>>();
    let last_chunk = decoded.last().ok_or(anyhow!("no chunks"))?;
    let expected_size =
        LittleEndian::read_u64(&last_chunk[last_chunk.len() - size_of::<u64>()..]) as usize;
    let mut bytes_written: usize = 0;
    let mut hasher = SHA2_512::new();

    let mut out_file = fs::File::options()
        .truncate(true)
        .create_new(!force)
        .write(true)
        .open(&output_path)?;
    for chunk in decoded {
        if chunk.len() + bytes_written > expected_size {
            hasher.update(&chunk[..expected_size - bytes_written]);
            out_file.write_all(&chunk[..expected_size - bytes_written])?;
            break;
        }
        hasher.update(chunk);
        out_file.write_all(chunk)?;
        bytes_written += chunk.len();
    }
    hasher.update(commit);
    let digest = hasher.digest().into_inner();
    if digest.ne(&meta.hash) {
        Err(anyhow!(
            "failed to restore {}: checksum mismatch",
            output_path.as_ref().display()
        ))?;
    }
    println!(
        "{bytes_written} bytes written to {}",
        output_path.as_ref().display()
    );

    Ok(())
}

pub(crate) fn restore(args: &RestoreArgs) -> Result<()> {
    println!("Restoring from {} images...", args.input_path.len());
    let chunks = read_chunks(&args.input_path)?;
    let mut previous_meta: Option<header::MetaHeader> = None;
    let mut previous_identifier: Option<[u8; IDENTIFIER_LENGTH]> = None;
    let mut payloads = Vec::<(u16, Vec<u8>)>::new();

    for chunk in chunks.iter() {
        let mut bytes = chunk.getRawBytes().as_slice();
        let header = Header::read_from(&mut bytes)?;
        match header {
            Header::Meta(m) => {
                if let Some(ref meta) = previous_meta {
                    if meta.ne(&m) {
                        Err(anyhow!("meta header mismatch"))?;
                    }
                } else {
                    if let Some(ref id) = previous_identifier {
                        if !m.hash.starts_with(id) {
                            Err(anyhow!("meta does not match identifier"))?;
                        }
                    }
                    previous_meta = Some(m);
                }
            }
            Header::Payload(p) => {
                if let Some(ref m) = previous_meta {
                    if !m.hash.starts_with(&p.identifier) {
                        Err(anyhow!("payload does not match meta"))?;
                    }
                } else if let Some(ref id) = previous_identifier {
                    if id.ne(&p.identifier) {
                        Err(anyhow!("payload has incorrect identifier"))?;
                    }
                } else {
                    previous_identifier = Some(p.identifier);
                }
                let mut buf = Vec::<u8>::new();
                bytes.read_to_end(&mut buf)?;
                payloads.push((p.index, buf));
            }
        };
    }

    let meta = previous_meta.ok_or(anyhow!("could not locate any metadata chunks"))?;
    println!(
        "Data loaded: got {}/{} recovery shards",
        payloads.len(),
        meta.recovery_count
    );

    write_output(
        &meta,
        &payloads,
        args.force,
        &args.output_path,
        &args.override_commit,
    )?;

    Ok(())
}
