use super::layout;
use crate::header::{Header, MetaHeader};
use anyhow::{anyhow, Result};
use base58::ToBase58;
use printpdf::{BuiltinFont, Mm, PdfDocumentReference, PdfLayerReference, Pt, Svg};
use qrcode::QrCode;

const DOTS_PER_INCH: f32 = 300.0;
const MM_PER_INCH: f32 = 25.4;
/// For courier (fixed width), points per mm.
const COURIER_GLYPH_FACTOR: f32 = 4.727;

pub(crate) struct Bounds {
    top: Mm,
    right: Mm,
    bottom: Mm,
    left: Mm,
}

impl Bounds {
    pub(crate) fn width(&self) -> Mm {
        self.right - self.left
    }
    pub(crate) fn height(&self) -> Mm {
        self.top - self.bottom
    }
}

/// Render a page
pub fn render_page(
    layout: &layout::Options,
    codes: &mut impl Iterator<Item = Svg>,
    page_num: usize,
    doc: &PdfDocumentReference,
    layer: &PdfLayerReference,
    commit: &str,
) -> Result<()> {
    let is_odd = (page_num % 2) == 0;
    let vertical_offset = if is_odd {
        Mm(0.0)
    } else {
        layout.avail_height - layout.avail_width
    };
    render_codes(vertical_offset, layout, codes, layer)?;

    let banner_bounds = Bounds {
        // For even pages, the top is smaller by margin-bottom for gutter.
        top: if is_odd {
            layout.avail_height
        } else {
            layout.avail_height - layout.avail_width
        },
        right: layout.avail_width,
        bottom: if is_odd {
            layout.avail_width + layout.margin_bottom
        } else {
            layout.margin_bottom
        },
        left: layout.margin_left,
    };
    render_banner(&banner_bounds, layout, page_num, doc, layer, commit)?;

    Ok(())
}

/// Render the QR codes on a page at the given vertical offset.
fn render_codes(
    vertical_offset: Mm,
    layout: &layout::Options,
    codes: &mut impl Iterator<Item = Svg>,
    layer: &PdfLayerReference,
) -> Result<()> {
    let shard_width = layout.module_length * layout.version.width().into();
    let quiet_offset = layout.module_length * 4.0;
    let area_width = shard_width * layout.shards_per_row as f32
        + quiet_offset * (layout.shards_per_row - 1) as f32;
    let left_offset = (layout.page_width - area_width) / 2.0;
    let chunk_offset = shard_width + quiet_offset;
    for row in 0..layout.shards_per_row {
        for col in 0..layout.shards_per_row {
            let svg = codes.next().ok_or(anyhow!("Ran out of QR codes"))?;
            // Scale factor, in dots.
            let scale_factor = layout.module_length.0 * DOTS_PER_INCH / MM_PER_INCH;
            let transform = printpdf::svg::SvgTransform {
                translate_x: Some((left_offset + chunk_offset * col as f32).into()),
                translate_y: Some(
                    (layout.margin_bottom + vertical_offset + chunk_offset * row as f32).into(),
                ),
                rotate: None,
                scale_x: Some(scale_factor),
                scale_y: Some(scale_factor),
                dpi: Some(DOTS_PER_INCH),
            };
            svg.add_to_layer(layer, transform);
        }
    }

    Ok(())
}

/// Render the banner at the given verical offset.
fn render_banner(
    bounds: &Bounds,
    layout: &layout::Options,
    page_num: usize,
    doc: &PdfDocumentReference,
    layer: &PdfLayerReference,
    commit: &str,
) -> Result<()> {
    let courier = doc.add_builtin_font(BuiltinFont::Courier)?;

    // Draw the title text: repo, page info, and document id (hash).
    let repo = format!("github.com/mook/paperpack@{commit}");
    let page_info = format!(
        "{}/{}+{}",
        page_num + 1,
        layout.data_page_count,
        layout.recovery_page_count - layout.data_page_count
    );
    let hash = layout.hash[..6].to_base58();

    let repo_font_size: f32 = 14.0;
    let repo_width = Mm(repo_font_size * repo.len() as f32 / COURIER_GLYPH_FACTOR);
    let title = format!("   {page_info}   {hash}");
    let title_width = bounds.width() - repo_width;
    let title_font_size = title_width / Mm(title.len() as f32) * COURIER_GLYPH_FACTOR;
    let max_font_size = f32::max(repo_font_size, title_font_size);

    layer.use_text(
        repo,
        repo_font_size,
        bounds.left,
        bounds.top - Pt(repo_font_size).into(),
        &courier,
    );

    layer.use_text(
        title,
        title_font_size,
        bounds.left + bounds.width() - title_width,
        bounds.top - Pt(title_font_size).into(),
        &courier,
    );

    // Draw the metadata QR codes.
    let mut buf = Vec::<u8>::with_capacity(MetaHeader::LENGTH);
    Header::Meta(MetaHeader {
        hash: layout.hash,
        original_count: u16::try_from(layout.data_shard_count)
            .map_err(|_| anyhow!("cannot render {} data chunks", layout.data_shard_count))?,
        recovery_count: u16::try_from(layout.recovery_shard_count).map_err(|_| {
            anyhow!(
                "cannot render {} recovery chunks",
                layout.recovery_shard_count
            )
        })?,
        shard_bytes: layout.data_bytes_per_shard as u64,
    })
    .write_to(&mut buf)?;
    // Similar to the recovery chunks, we need to convert to string and back to SVG.
    let svg_string = QrCode::with_error_correction_level(&buf, qrcode::EcLevel::H)?
        .render::<qrcode::render::svg::Color>()
        .quiet_zone(true)
        .module_dimensions(1, 1)
        .build();
    let svg = printpdf::svg::Svg::parse(&svg_string)?;
    let desired_svg_height = (bounds.height() - Pt(max_font_size).into()) / 2.0;
    let actual_svg_height: Mm = svg.height.into_pt(DOTS_PER_INCH).into();
    let object = svg.into_xobject(layer);
    object.clone().add_to_layer(
        layer,
        printpdf::svg::SvgTransform {
            translate_x: Some(bounds.left.into()),
            translate_y: Some(bounds.bottom.into()),
            rotate: None,
            scale_x: Some(desired_svg_height / actual_svg_height),
            scale_y: Some(desired_svg_height / actual_svg_height),
            dpi: Some(DOTS_PER_INCH),
        },
    );
    object.clone().add_to_layer(
        layer,
        printpdf::svg::SvgTransform {
            translate_x: Some((bounds.right - desired_svg_height).into()),
            translate_y: Some(bounds.bottom.into()),
            rotate: None,
            scale_x: Some(desired_svg_height / actual_svg_height),
            scale_y: Some(desired_svg_height / actual_svg_height),
            dpi: Some(DOTS_PER_INCH),
        },
    );

    Ok(())
}
