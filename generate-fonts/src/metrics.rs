use std::{
    collections::HashMap,
    fs,
    io::{self, BufRead},
    path::Path,
};

use anyhow::{anyhow, Result};

/// `FontMetrics` describes the metrics for a font.
#[derive(Debug, Default)]
pub struct FontMetrics {
    pub name: String,
    ascender: f32,
    descender: f32,
    widths: HashMap<u8, f32>,
    kerning: HashMap<(u8, u8), f32>,
}

impl FontMetrics {
    pub(crate) fn from_file(path: impl AsRef<Path>) -> Result<FontMetrics> {
        let file = fs::File::open(&path)?;
        let mut result = FontMetrics::default();
        let mut names = HashMap::<String, u8>::new();

        for maybe_line in io::BufReader::new(file).lines() {
            let line = maybe_line?;
            let tokens: Vec<_> = line.split_ascii_whitespace().collect();
            match tokens.first() {
                Some(&"FontName") => {
                    result.name = tokens
                        .get(1)
                        .ok_or(anyhow!(
                            "{}: failed to read font name",
                            path.as_ref().display()
                        ))?
                        .replace("-", "");
                }
                Some(&"Ascender") => {
                    let value = tokens.get(1).ok_or(anyhow!(
                        "{}: failed to read ascender",
                        path.as_ref().display()
                    ))?;
                    result.ascender = value
                        .parse::<f32>()
                        .map_err(|err| {
                            anyhow!(
                                "{}: ascender is not a float: {err}",
                                path.as_ref().display()
                            )
                        })?
                        .abs();
                }
                Some(&"Descender") => {
                    let value = tokens.get(1).ok_or(anyhow!(
                        "{}: failed to read descender",
                        path.as_ref().display()
                    ))?;
                    result.descender = value
                        .parse::<f32>()
                        .map_err(|err| {
                            anyhow!(
                                "{}: descender is not a float: {err}",
                                path.as_ref().display()
                            )
                        })?
                        .abs();
                }
                Some(&"C") => {
                    // For the set of files we have, we can assume:
                    // C <ascii?> ; WX <width> ; N <name> ; ....
                    let mut code: u8 = 0;
                    let mut width = f32::NAN;
                    let mut name = String::with_capacity(64);
                    for part in tokens.split(|v| v == &";") {
                        match part.first() {
                            Some(&"C") => {
                                if let Some(value) =
                                    part.get(1).and_then(|num| num.parse::<u8>().ok())
                                {
                                    code = value;
                                }
                            }
                            Some(&"WX") => {
                                if let Some(value) =
                                    part.get(1).and_then(|num| num.parse::<f32>().ok())
                                {
                                    width = value;
                                }
                            }
                            Some(&"N") => {
                                if let Some(value) = part.get(1) {
                                    name = value.to_string();
                                }
                            }
                            _ => (),
                        }
                    }
                    if code != 0 && !width.is_nan() {
                        result.widths.insert(code, width);
                        names.insert(name, code);
                    }
                }
                Some(&"KPX") => {
                    // KPX <from> <to> <offset>
                    let from = tokens.get(1).and_then(|n| names.get(*n));
                    let to = tokens.get(2).and_then(|n| names.get(*n));
                    let offset = tokens.get(3).and_then(|v| v.parse::<f32>().ok());
                    if let (Some(from), Some(to), Some(offset)) = (from, to, offset) {
                        result.kerning.insert((*from, *to), offset);
                    }
                }
                _ => (),
            }
        }
        Ok(result)
    }

    /// Get the identifier to use for this metric; this is the name, but in upper case.
    pub(crate) fn identifier(&self) -> String {
        // All names are in UpperCamelCase; we want to convert them to SCREAMING_SNAKE_CASE.
        self.name
            .chars()
            .rev()
            .collect::<String>()
            .split_inclusive(|c: char| c.is_ascii_uppercase())
            .map(|chars| chars.chars().rev().collect::<String>().to_ascii_uppercase())
            .rev()
            .collect::<Vec<_>>()
            .join("_")
    }

    pub(crate) fn to_source(&self, mut writer: impl io::Write) -> Result<()> {
        writeln!(writer, "FontMetrics {{")?;
        writeln!(writer, "ascender: {}.0,", self.ascender)?;
        writeln!(writer, "descender: {}.0,", self.descender)?;
        writeln!(writer, "widths: HashMap::from([")?;
        for (code, width) in &self.widths {
            writeln!(
                writer,
                "\t(unsafe {{ char::from_u32_unchecked({code}) }}, {width}.0),"
            )?;
        }
        writeln!(writer, "]),")?;
        writeln!(writer, "kerning: HashMap::from([")?;
        for ((from, to), offset) in &self.kerning {
            writeln!(writer, "\t((unsafe {{ char::from_u32_unchecked({from}) }}, unsafe {{ char::from_u32_unchecked({to}) }}), {offset}.0),")?;
        }
        writeln!(writer, "]),")?;
        writeln!(writer, "}}")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::metrics::FontMetrics;

    #[test]
    fn test_identifier() {
        let input = FontMetrics {
            name: "HelloWorld".to_string(),
            ..Default::default()
        };
        assert_eq!("HELLO_WORLD", input.identifier());
    }
}
