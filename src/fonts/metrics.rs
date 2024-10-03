use anyhow::{anyhow, Result};
use printpdf::Mm;

use super::data;
use std::collections::{HashMap, VecDeque};

pub(crate) struct Bounds {
    pub(crate) top: Mm,
    pub(crate) right: Mm,
    pub(crate) bottom: Mm,
    pub(crate) left: Mm,
}

impl Bounds {
    pub(crate) fn width(&self) -> Mm {
        self.right - self.left
    }
    pub(crate) fn height(&self) -> Mm {
        self.top - self.bottom
    }
}

/// `FontMetrics` describes the metrics for a font.
pub struct FontMetrics {
    pub ascender: f32,
    pub descender: f32,
    pub widths: HashMap<char, f32>,
    pub kerning: HashMap<(char, char), f32>,
}

impl FontMetrics {
    pub(crate) fn measure(&self, text: impl AsRef<str>) -> f32 {
        let mut last: char = '\x00';
        let mut sum: f32 = 0.;
        for ch in text.as_ref().chars() {
            sum += self.widths.get(&ch).unwrap_or(&0.);
            sum += self.kerning.get(&(last, ch)).unwrap_or(&0.);
            last = ch;
        }
        sum / 1000.
    }
}

impl From<printpdf::font::BuiltinFont> for &'static FontMetrics {
    fn from(value: printpdf::font::BuiltinFont) -> Self {
        data::from(value)
    }
}
impl TryFrom<printpdf::Font> for &'static FontMetrics {
    type Error = anyhow::Error;

    fn try_from(value: printpdf::Font) -> Result<Self, Self::Error> {
        if let printpdf::Font::BuiltinFont(font) = value {
            Ok(data::from(font))
        } else {
            Err(anyhow!("cannot get metrics for external font"))
        }
    }
}

pub struct SizedFont<'a> {
    pub font: printpdf::IndirectFontRef,
    pub size: printpdf::Pt,
    pub metrics: &'a FontMetrics,
}
pub enum Alignment {
    Left,
    Right,
    Center,
}

impl SizedFont<'_> {
    pub(crate) fn new(
        doc: &printpdf::PdfDocumentReference,
        font: printpdf::font::BuiltinFont,
        size: printpdf::Pt,
    ) -> Result<Self> {
        Ok(SizedFont {
            font: doc.add_builtin_font(font)?,
            metrics: font.into(),
            size,
        })
    }

    /// Measure a line of text, returning its width.
    pub(crate) fn measure(&self, text: impl AsRef<str>) -> printpdf::Pt {
        self.size * self.metrics.measure(text)
    }
    /// Get the height of the descender.
    pub(crate) fn descender(&self) -> printpdf::Pt {
        self.size * self.metrics.descender / 1000.
    }
    /// Write a line of text (without any line breaks).
    pub(crate) fn write(
        &self,
        layer: &printpdf::PdfLayerReference,
        text: impl AsRef<str>,
        x: Mm,
        y: Mm,
        alignment: &Alignment,
    ) {
        let final_x = match alignment {
            Alignment::Left => x,
            Alignment::Right => x - self.measure(&text).into(),
            Alignment::Center => x - (self.measure(&text) / 2.).into(),
        };
        layer.use_text(text.as_ref(), self.size.0, final_x, y, &self.font);
    }
    /// Write some space-separated text over multiple lines.  This currently ignores the bottm bound
    /// and will happily write text too far down.
    pub(crate) fn write_section<'a>(
        &self,
        layer: &printpdf::PdfLayerReference,
        words: impl Iterator<Item = &'a str>,
        bounds: &Bounds,
        alignment: &Alignment,
    ) {
        layer.begin_text_section();
        layer.set_font(&self.font, self.size.0);
        layer.set_line_height(self.size.0);
        // Move the cursor as absolute coordinates.  All moves are relative after.
        layer.set_text_cursor(bounds.left, bounds.top - self.size.into());

        // Split the words into lines by first approximating how many we can fit in a line.
        let mut word_vec: VecDeque<_> = words.collect();
        let mut line = String::with_capacity(4096);

        while let Some(word) = word_vec.pop_front() {
            let line_length = line.len();
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            if bounds.width() < self.measure(&line).into() {
                word_vec.push_front(word);
                self.write_line(layer, &line[..line_length], alignment, bounds.width());
                line.clear();
            }
        }
        if !line.is_empty() {
            self.write_line(layer, &line, alignment, bounds.width());
        }

        layer.end_text_section();
    }

    /// Write a single line of text, for use by `write_section`.  Use `write` for writing a line of
    /// text at a given position.
    fn write_line(
        &self,
        layer: &printpdf::PdfLayerReference,
        line: &str,
        alignment: &Alignment,
        width: Mm,
    ) {
        let actual_length = self.measure(line);
        let x_offset = match alignment {
            Alignment::Left => Mm(0.),
            Alignment::Right => width - actual_length.into(),
            Alignment::Center => width / 2. - (actual_length / 2.).into(),
        };
        layer.set_text_cursor(x_offset, Mm(0.));
        layer.write_text(line, &self.font);
        layer.set_text_cursor(Mm(0.) - x_offset, Mm(0.) - self.size.into());
    }
}

#[cfg(test)]
mod test {
    use crate::fonts::metrics::FontMetrics;
    use anyhow::Result;

    #[test]
    fn test_measure_courier() -> Result<()> {
        let font: &FontMetrics = printpdf::BuiltinFont::Courier.into();
        assert_eq!(
            font.measure("hello") * 1000.,
            // h    e      l      l      o
            600. + 600. + 600. + 600. + 600.
        );
        Ok(())
    }

    #[test]
    fn test_measure_helv() -> Result<()> {
        let font: &FontMetrics = printpdf::BuiltinFont::Helvetica.into();
        assert_eq!(
            font.measure("Kerning") * 1000.,
            // K   Ke     e      r     rn     n      i      n      g
            667. - 40. + 556. + 333. + 25. + 556. + 222. + 556. + 556.
        );
        Ok(())
    }
}
