use super::layout;
use crate::header::{Header, MetaHeader};
use anyhow::{anyhow, Result};
use base58::ToBase58;
use printpdf::{BuiltinFont, Mm, PdfDocumentReference, PdfLayerReference, Pt, Svg};
use qrcode::QrCode;

const DOTS_PER_INCH: f32 = 300.0;
const MM_PER_INCH: f32 = 25.4;

/// Font metrics; one point is always 1000 units.
struct FontMetrics {
    /// Character width, as a fraction of a point. Undefined for non-fixed-width fonts.
    char_width: f32,
    /// Descender height.
    descender: f32,
}

const COURIER_METRICS: FontMetrics = FontMetrics {
    char_width: 600.0 / 1000.0,
    descender: 159.0 / 1000.0,
};
/*
const HELV_METRICS: FontMetrics = FontMetrics {
    char_width: f32::NAN,
    descender: 207.0 / 1000.0,
};
*/
const HELV_BOLD_METRICS: FontMetrics = FontMetrics {
    char_width: f32::NAN,
    descender: 207.0 / 1000.0,
};

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
        right: layout.margin_left + layout.avail_width,
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

/// Render the QR codes on a page at the given vertical offset
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
    let helv = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let helv_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

    // Draw the metadata QR codes.
    let mut buf = Vec::<u8>::with_capacity(MetaHeader::LENGTH);
    Header::Meta(MetaHeader {
        identifier: layout.identifier,
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
        .quiet_zone(false)
        .module_dimensions(1, 1)
        .build();
    let svg = printpdf::svg::Svg::parse(&svg_string)?;
    let desired_svg_length = bounds.height() / 2.0;
    let actual_svg_length: Mm = svg.height.into_pt(DOTS_PER_INCH).into();
    let quiet_zone_length = desired_svg_length / (svg.height.0 as f32) * 4.0;
    let object = svg.into_xobject(layer);
    object.clone().add_to_layer(
        layer,
        printpdf::svg::SvgTransform {
            translate_x: Some((bounds.left + quiet_zone_length).into()),
            translate_y: Some(bounds.bottom.into()),
            rotate: None,
            scale_x: Some(desired_svg_length / actual_svg_length),
            scale_y: Some(desired_svg_length / actual_svg_length),
            dpi: Some(DOTS_PER_INCH),
        },
    );
    object.clone().add_to_layer(
        layer,
        printpdf::svg::SvgTransform {
            translate_x: Some((bounds.right - desired_svg_length - quiet_zone_length).into()),
            translate_y: Some(bounds.bottom.into()),
            rotate: None,
            scale_x: Some(desired_svg_length / actual_svg_length),
            scale_y: Some(desired_svg_length / actual_svg_length),
            dpi: Some(DOTS_PER_INCH),
        },
    );

    // Draw the title text: repo, page info, and document id (hash).
    let label_font_size: f32 = 14.0;
    let info_font_size: f32 = 24.0;
    let description_font_size: f32 = 10.0;
    let repo_font_size: f32 = 14.0;

    let repo = format!("github.com/mook/paperpack@{commit}");
    let repo_width: Mm = Pt(repo_font_size * repo.len() as f32 * COURIER_METRICS.char_width).into();
    let repo_avail_width = bounds.width() - desired_svg_length * 2.0;
    layer.use_text(
        repo,
        repo_font_size,
        bounds.left + desired_svg_length + repo_avail_width / 2.0 - repo_width / 2.0,
        bounds.top - Pt(repo_font_size).into(),
        &courier,
    );

    let document_id = layout.hash[..6].to_base58();
    layer.use_text(
        document_id,
        info_font_size,
        bounds.left + quiet_zone_length + desired_svg_length + quiet_zone_length,
        bounds.bottom + Pt(COURIER_METRICS.descender * info_font_size).into(),
        &courier,
    );
    layer.use_text(
        "Document ID",
        label_font_size,
        bounds.left + quiet_zone_length + desired_svg_length + quiet_zone_length,
        bounds.bottom
            + Pt(info_font_size).into()
            + Pt(label_font_size * HELV_BOLD_METRICS.descender).into(),
        &helv_bold,
    );

    let page_info = format!(
        "{}/{}+{}",
        page_num + 1,
        layout.data_page_count,
        layout.recovery_page_count - layout.data_page_count
    );
    let page_info_width: Mm =
        Pt(info_font_size * page_info.len() as f32 * COURIER_METRICS.char_width).into();
    layer.use_text(
        page_info,
        info_font_size,
        bounds.right - quiet_zone_length - desired_svg_length - quiet_zone_length - page_info_width,
        bounds.bottom + Pt(COURIER_METRICS.descender * info_font_size).into(),
        &courier,
    );
    // Hard code the horizontal advance of the "Page Info" text here. It should be 4340 w/ kerning...
    let page_info_label_width: Mm = Pt(label_font_size * 4500.0 / 1000.0).into();
    layer.use_text(
        "Page Info",
        label_font_size,
        bounds.right
            - quiet_zone_length
            - desired_svg_length
            - quiet_zone_length
            - page_info_label_width,
        bounds.bottom
            + Pt(info_font_size).into()
            + Pt(label_font_size * HELV_BOLD_METRICS.descender).into(),
        &helv_bold,
    );

    // Write some descriptive text.
    layer.begin_text_section();
    layer.set_font(&helv, description_font_size);
    layer.set_text_cursor(
        bounds.left + quiet_zone_length + desired_svg_length + quiet_zone_length,
        bounds.top
            - Pt(repo_font_size).into()
            - Pt(description_font_size).into()
            - Pt(description_font_size).into(),
    );
    layer.set_line_height(description_font_size);
    for line in [
        "This is a paper backup created using the program listed above.",
        format!(
            "When {}, it can be used to restore",
            if layout.data_page_count == 1 {
                "any page is scanned".to_string()
            } else {
                format!("at least {} pages are combined", layout.data_page_count)
            }
        )
        .as_str(),
        "the original file.  More pages may be required if some QR codes",
        "fail to be decoded.  At least one copy of the QR code to the left",
        "and right of this text is required.",
    ] {
        layer.write_text(line, &helv);
        layer.add_line_break();
    }
    layer.end_text_section();

    Ok(())
}
