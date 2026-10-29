//! Render LibreOffice's numeric conditional-format extension. The original XML
//! remains in the ODS workbook and is written back by spreadsheet-ods.
use crate::formula;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::{io::Read, path::Path};
#[derive(Clone, Debug)]
pub struct Rule {
    pub sheet: String,
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub style: String,
    pub op: String,
    pub threshold: f64,
}
impl Rule {
    pub fn matches(&self, sheet: &str, r: u32, c: u32, value: f64) -> bool {
        self.sheet == sheet
            && r >= self.start.0
            && r <= self.end.0
            && c >= self.start.1
            && c <= self.end.1
            && match self.op.as_str() {
                ">=" => value >= self.threshold,
                "<=" => value <= self.threshold,
                "<" => value < self.threshold,
                ">" => value > self.threshold,
                "=" => value == self.threshold,
                _ => value != self.threshold,
            }
    }
}
fn attr(e: &BytesStart<'_>, name: &[u8], reader: &Reader<&[u8]>) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| {
            a.decode_and_unescape_value(reader.decoder())
                .ok()
                .map(|s| s.into_owned())
        })
}
pub fn read(path: &Path) -> Vec<Rule> {
    let result = (|| -> Result<Vec<Rule>, Box<dyn std::error::Error>> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?)?;
        let mut xml = String::new();
        zip.by_name("content.xml")?.read_to_string(&mut xml)?;
        Ok(parse(&xml))
    })();
    result.unwrap_or_default()
}
fn parse(xml: &str) -> Vec<Rule> {
    let mut reader = Reader::from_str(xml);
    let mut sheet = String::new();
    let mut range = None;
    let mut rules = vec![];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e) | Event::Empty(e)) => match e.name().as_ref() {
                b"table:table" => {
                    sheet = attr(&e, b"table:name", &reader).unwrap_or_default();
                }
                b"calcext:conditional-format" => {
                    range = attr(&e, b"calcext:target-range-address", &reader);
                }
                b"calcext:condition" => {
                    if let (Some(range), Some(value), Some(style)) = (
                        &range,
                        attr(&e, b"calcext:value", &reader),
                        attr(&e, b"calcext:apply-style-name", &reader),
                    ) {
                        let mut points = range.split(':');
                        let Some(start) = points
                            .next()
                            .and_then(|s| formula::coordinate(s.rsplit('.').next().unwrap_or(s)))
                        else {
                            continue;
                        };
                        let end = points
                            .next()
                            .and_then(|s| formula::coordinate(s.rsplit('.').next().unwrap_or(s)))
                            .unwrap_or(start);
                        for op in [">=", "<=", "!=", "<>", ">", "<", "="] {
                            if let Some(threshold) = value
                                .strip_prefix(op)
                                .and_then(|s| s.trim().parse::<f64>().ok())
                            {
                                rules.push(Rule {
                                    sheet: sheet.clone(),
                                    start,
                                    end,
                                    style,
                                    op: op.into(),
                                    threshold,
                                });
                                break;
                            }
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(e)) if e.name().as_ref() == b"calcext:conditional-format" => range = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    rules
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn libreoffice_rule() {
        let rules = parse(
            r#"<table:table table:name="Budget"><calcext:conditional-format calcext:target-range-address="Budget.D4:Budget.D8"><calcext:condition calcext:apply-style-name="ce12" calcext:value="&lt;0"/></calcext:conditional-format></table:table>"#,
        );
        assert_eq!(rules.len(), 1);
        assert!(rules[0].matches("Budget", 6, 3, -35.0));
        assert!(!rules[0].matches("Budget", 6, 3, 35.0));
    }
}
