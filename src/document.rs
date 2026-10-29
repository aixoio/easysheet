use crate::formula;
use spreadsheet_ods::{
    CellStyle, Sheet, Value, WorkBook,
    condition::Condition,
    style::{StyleUse, stylemap::StyleMap},
};
use std::path::Path;

pub struct Document {
    pub book: WorkBook,
    pub sheet: usize,
    pub dirty: bool,
    undo: Vec<WorkBook>,
    redo: Vec<WorkBook>,
    imported_rules: Vec<crate::ods_rules::Rule>,
}
impl Document {
    pub fn blank() -> Self {
        let mut book = WorkBook::new_empty();
        book.push_sheet(Sheet::new("Sheet 1"));
        Self {
            book,
            sheet: 0,
            dirty: false,
            undo: vec![],
            redo: vec![],
            imported_rules: vec![],
        }
    }
    pub fn demo() -> Self {
        let mut d = Self::blank();
        d.book.sheet_mut(0).set_name("Monthly budget");
        for (r, values) in [
            vec!["October", "", "", ""],
            vec!["Budget / 2026", "", "", ""],
            vec!["Category", "Planned", "Actual", "Remaining"],
            vec!["Home", "1600", "1550", "=B4-C4"],
            vec!["Groceries", "500", "438", "=B5-C5"],
            vec!["Transport", "220", "185", "=B6-C6"],
            vec!["Good things", "180", "215", "=B7-C7"],
            vec!["Savings", "600", "600", "=B8-C8"],
            vec!["Total", "=SUM(B4:B8)", "=SUM(C4:C8)", "=SUM(D4:D8)"],
        ]
        .iter()
        .enumerate()
        {
            for (c, v) in values.iter().enumerate() {
                d.write(r as u32, c as u32, v);
            }
        }
        for r in [0, 2, 8] {
            for c in 0..4 {
                d.style_cell(r, c, |s| {
                    s.textstyle_mut().set_attr("fo:font-weight", "bold");
                    s.cellstyle_mut().set_attr(
                        "fo:background-color",
                        if r == 2 { "#e4edf9" } else { "#eff5fc" },
                    );
                });
            }
        }
        d.conditional((3, 3), (7, 3), "<", 0.0, "#ffe1de");
        d.recalculate();
        d.dirty = false;
        d.undo.clear();
        d
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut book = spreadsheet_ods::read_ods(path).map_err(|e| e.to_string())?;
        if book.num_sheets() == 0 {
            book.push_sheet(Sheet::new("Sheet 1"));
        }
        Ok(Self {
            book,
            sheet: 0,
            dirty: false,
            undo: vec![],
            redo: vec![],
            imported_rules: crate::ods_rules::read(path),
        })
    }
    pub fn checkpoint(&mut self) {
        self.undo.push(self.book.clone());
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }
    pub fn undo(&mut self) {
        if let Some(b) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.book, b));
            self.sheet = self.sheet.min(self.book.num_sheets() - 1);
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        if let Some(b) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.book, b));
            self.dirty = true;
        }
    }
    pub fn raw(&self, r: u32, c: u32) -> String {
        let s = self.book.sheet(self.sheet);
        s.formula(r, c)
            .map(|f| formula::from_odf(f))
            .unwrap_or_else(|| match s.value(r, c) {
                Value::Number(v) | Value::Currency(v, _) => v.to_string(),
                v => value_text(v),
            })
    }
    pub fn display(&self, r: u32, c: u32) -> String {
        if self.book.sheet(self.sheet).formula(r, c).is_some() {
            match formula::evaluate_value(&self.book, self.sheet, r, c) {
                Ok(v) => value_text(&v),
                Err(e) => e,
            }
        } else {
            value_text(self.book.sheet(self.sheet).value(r, c))
        }
    }
    pub fn formatted(&self, r: u32, c: u32, style: &CellStyle) -> String {
        use spreadsheet_ods::format::{FormatPartType, ValueFormatTrait};
        let Ok(evaluated) = formula::evaluate_value(&self.book, self.sheet, r, c) else {
            return self.display(r, c);
        };
        let Some(value) = evaluated.as_f64_opt() else {
            return value_text(&evaluated);
        };
        let Some(name) = style.value_format() else {
            return self.display(r, c);
        };
        if let Some((format, absolute)) = crate::currency::resolve(&self.book, name, value) {
            return crate::currency::render(format, value, absolute)
                .unwrap_or_else(|| self.display(r, c));
        }
        let (format, multiplier): (&Vec<spreadsheet_ods::format::FormatPart>, f64) =
            if let Some(f) = self.book.percentage_format(name) {
                (f.parts(), 100.0)
            } else if let Some(f) = self.book.currency_format(name) {
                (f.parts(), 1.0)
            } else if let Some(f) = self.book.number_format(name) {
                (f.parts(), 1.0)
            } else {
                return self.display(r, c);
            };
        let mut text = String::new();
        for part in format {
            match part.part_type() {
                FormatPartType::Number => {
                    let places = part
                        .attr_def("number:decimal-places", "0")
                        .parse::<usize>()
                        .unwrap_or(0)
                        .min(12);
                    let mut n = format!("{:.*}", places, value * multiplier);
                    if part.attr_def("number:grouping", "false") == "true" {
                        let split = n.find('.').unwrap_or(n.len());
                        let sign = usize::from(n.starts_with('-'));
                        let mut i = split;
                        while i > sign + 3 {
                            i -= 3;
                            n.insert(i, ',');
                        }
                    }
                    text.push_str(&n);
                }
                FormatPartType::Text | FormatPartType::CurrencySymbol => {
                    if let Some(s) = part.content() {
                        text.push_str(s)
                    }
                }
                _ => return self.display(r, c),
            }
        }
        if text.is_empty() {
            self.display(r, c)
        } else {
            text
        }
    }
    pub fn apply_currency(
        &mut self,
        start: (u32, u32),
        end: (u32, u32),
        settings: &crate::currency::CurrencySettings,
    ) {
        self.checkpoint();
        let format = settings.create_format(&mut self.book);
        for r in start.0..=end.0 {
            for c in start.1..=end.1 {
                self.style_cell(r, c, |s| s.set_value_format(&format));
            }
        }
    }
    pub fn format_color(&self, r: u32, c: u32, style: &CellStyle) -> Option<&str> {
        use spreadsheet_ods::format::ValueFormatTrait;
        let value = formula::evaluate(&self.book, self.sheet, r, c).ok()?;
        let (format, _) = crate::currency::resolve(&self.book, style.value_format()?, value)?;
        format.textstyle().attr("fo:color")
    }
    pub fn write(&mut self, r: u32, c: u32, text: &str) {
        let s = self.book.sheet_mut(self.sheet);
        if text.starts_with('=') {
            s.set_formula(r, c, formula::to_odf(text));
        } else {
            s.clear_formula(r, c);
            s.set_value(r, c, parse_value(text));
        }
    }
    pub fn set(&mut self, r: u32, c: u32, text: &str) {
        if self.raw(r, c) == text {
            return;
        }
        self.checkpoint();
        self.write(r, c, text);
        self.recalculate();
    }
    pub fn recalculate(&mut self) {
        for i in 0..self.book.num_sheets() {
            let positions: Vec<_> = self
                .book
                .sheet(i)
                .iter()
                .filter(|(_, c)| c.formula().is_some())
                .map(|(p, _)| p)
                .collect();
            let values: Vec<_> = positions
                .into_iter()
                .map(|(r, c)| ((r, c), formula::evaluate_value(&self.book, i, r, c)))
                .collect();
            for ((r, c), v) in values {
                if let Ok(v) = v {
                    self.book.sheet_mut(i).set_value(r, c, v);
                }
            }
        }
    }
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.recalculate();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let tmp = tempfile::Builder::new()
            .prefix(".easysheet-")
            .suffix(".ods")
            .tempfile_in(parent)
            .map_err(|e| e.to_string())?;
        spreadsheet_ods::write_ods(&mut self.book, tmp.path()).map_err(|e| e.to_string())?;
        tmp.as_file().sync_all().map_err(|e| e.to_string())?;
        tmp.persist(path).map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(())
    }
    pub fn base_style(&self, r: u32, c: u32) -> CellStyle {
        self.book
            .sheet(self.sheet)
            .cellstyle(r, c)
            .and_then(|n| self.book.cellstyle(n.as_str()))
            .cloned()
            .unwrap_or_else(CellStyle::new_empty)
    }
    pub fn style_cell(&mut self, r: u32, c: u32, f: impl FnOnce(&mut CellStyle)) {
        let mut style = self.base_style(r, c);
        style.set_name("");
        f(&mut style);
        let name = self.book.add_cellstyle(style);
        self.book.sheet_mut(self.sheet).set_cellstyle(r, c, &name);
    }
    pub fn conditional(
        &mut self,
        start: (u32, u32),
        end: (u32, u32),
        op: &str,
        threshold: f64,
        color: &str,
    ) {
        let mut highlight = CellStyle::new_empty();
        highlight.set_styleuse(StyleUse::Named);
        highlight.set_origin(spreadsheet_ods::style::StyleOrigin::Styles);
        highlight
            .cellstyle_mut()
            .set_attr("fo:background-color", color);
        let target = self.book.add_cellstyle(highlight);
        for r in start.0..=end.0 {
            for c in start.1..=end.1 {
                let cond = match op {
                    "<" => Condition::content_lt(threshold),
                    ">" => Condition::content_gt(threshold),
                    "<=" => Condition::content_le(threshold),
                    ">=" => Condition::content_ge(threshold),
                    "!=" => Condition::content_ne(threshold),
                    _ => Condition::content_eq(threshold),
                };
                self.style_cell(r, c, |style| {
                    style.push_stylemap(StyleMap::new(cond, target.clone().into(), None))
                });
            }
        }
    }
    pub fn visible_style(&self, r: u32, c: u32) -> CellStyle {
        let style = self.base_style(r, c);
        if let Ok(value) = formula::evaluate(&self.book, self.sheet, r, c) {
            for rule in &self.imported_rules {
                if rule.matches(self.book.sheet(self.sheet).name(), r, c, value)
                    && let Some(highlight) = self.book.cellstyle(&rule.style)
                {
                    return overlay(style, highlight);
                }
            }
        }
        if let Some(maps) = style.stylemaps() {
            let value = formula::evaluate(&self.book, self.sheet, r, c).ok();
            for m in maps {
                let text = m.condition().to_string();
                if let (Some(v), Some(expr)) = (value, text.strip_prefix("cell-content()")) {
                    for op in [">=", "<=", "!=", "<>", ">", "<", "="] {
                        if let Some(n) = expr.strip_prefix(op).and_then(|s| s.parse::<f64>().ok()) {
                            let matched = match op {
                                ">=" => v >= n,
                                "<=" => v <= n,
                                ">" => v > n,
                                "<" => v < n,
                                "=" => v == n,
                                _ => v != n,
                            };
                            if matched
                                && let Some(s) = self.book.cellstyle(m.applied_style().as_str())
                            {
                                return overlay(style.clone(), s);
                            }
                            break;
                        }
                    }
                }
            }
        }
        style
    }
}
fn number(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() >= 1e12 || v.abs() < 1e-6 {
        format!("{v:e}")
    } else {
        format!("{v:.8}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
pub fn value_text(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(v) => number(*v),
        Value::Percentage(v) => format!("{}%", number(v * 100.0)),
        Value::Currency(v, c) => format!("{c} {v:.2}"),
        Value::Boolean(v) => v.to_string().to_uppercase(),
        Value::DateTime(v) => v.format("%Y-%m-%d %H:%M:%S").to_string(),
        Value::TimeDuration(v) => format!(
            "{}:{:02}:{:02}",
            v.num_hours(),
            v.num_minutes().abs() % 60,
            v.num_seconds().abs() % 60
        ),
        Value::TextXml(tags) => {
            let mut text = String::new();
            for (i, tag) in tags.iter().enumerate() {
                if i > 0 {
                    text.push('\n');
                }
                tag.extract_text(&mut text);
            }
            text
        }
        v => v.as_string_or_default(),
    }
}
fn parse_value(s: &str) -> Value {
    if s.is_empty() {
        Value::Empty
    } else if let Some(v) = s.strip_prefix('\'') {
        Value::Text(v.into())
    } else if let Ok(v) = s.parse::<f64>() {
        if v.is_finite() {
            Value::Number(v)
        } else {
            Value::Text(s.into())
        }
    } else if let Some(v) = s.strip_suffix('%').and_then(|s| s.parse::<f64>().ok()) {
        Value::Percentage(v / 100.0)
    } else if s.eq_ignore_ascii_case("TRUE") {
        Value::Boolean(true)
    } else if s.eq_ignore_ascii_case("FALSE") {
        Value::Boolean(false)
    } else {
        Value::Text(s.into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("budget.ods");
        let mut d = Document::demo();
        d.save(&path).unwrap();
        let mut d = Document::open(&path).unwrap();
        assert_eq!(d.display(8, 1), "3100");
        assert_eq!(d.raw(3, 3), "=B4-C4");
        assert_eq!(
            d.base_style(2, 0).textstyle().attr("fo:font-weight"),
            Some("bold")
        );
        assert!(d.base_style(3, 3).stylemaps().is_some());
        d.set(3, 1, "1700");
        assert_eq!(d.display(3, 3), "150");
        d.undo();
        assert_eq!(d.display(3, 3), "50");
        d.redo();
        assert_eq!(d.display(3, 3), "150");
    }
    #[test]
    fn preserve_other_sheets() {
        let mut d = Document::blank();
        let mut s = Sheet::new("Other");
        s.set_value(4, 4, "Keep me");
        d.book.push_sheet(s);
        d.set(0, 0, "42");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ods");
        d.save(&path).unwrap();
        let d = Document::open(&path).unwrap();
        assert_eq!(
            d.book.sheet(1).value(4, 4).as_string_or_default(),
            "Keep me"
        );
    }
}

fn overlay(mut base: CellStyle, highlight: &CellStyle) -> CellStyle {
    for (k, v) in highlight.cellstyle().iter() {
        base.cellstyle_mut().set_attr(k, v);
    }
    for (k, v) in highlight.textstyle().iter() {
        base.textstyle_mut().set_attr(k, v);
    }
    for (k, v) in highlight.paragraphstyle().iter() {
        base.paragraphstyle_mut().set_attr(k, v);
    }
    base
}

#[cfg(test)]
mod interoperability {
    use super::*;
    #[test]
    fn libreoffice_saved_fixture() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libreoffice-budget.ods");
        let mut d = Document::open(&source).unwrap();
        assert_eq!(d.display(8, 1), "3100");
        assert_eq!(d.raw(3, 3), "=B4-C4");
        assert_eq!(
            d.visible_style(6, 3)
                .cellstyle()
                .attr("fo:background-color"),
            Some("#ffe1de")
        );
        d.set(6, 2, "100");
        assert_ne!(
            d.visible_style(6, 3)
                .cellstyle()
                .attr("fo:background-color"),
            Some("#ffe1de")
        );
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("edited.ods");
        d.save(&output).unwrap();
        let d = Document::open(&output).unwrap();
        assert_eq!(d.display(6, 3), "80");
        assert_eq!(d.display(8, 3), "227");
    }
    #[test]
    #[ignore = "requires an installed LibreOffice; run with --ignored"]
    fn live_libreoffice_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.ods");
        let output = dir.path().join("lo");
        std::fs::create_dir(&output).unwrap();
        let mut d = Document::demo();
        d.set(10, 0, "=IF(B4>1000;\"On track\";\"Review\")");
        d.set(11, 0, "=SUMIF(A4:A8;\"Home\";B4:B8)");
        d.set(12, 0, "=SUMIFS(C4:C8;B4:B8;\">200\";C4:C8;\"<1000\")");
        d.set(13, 0, "1234.567");
        d.set(14, 0, "-1234.567");
        d.set(16, 0, "=MEDIAN(C4:C8)");
        d.set(17, 0, "=MEDIAN(C4:C7)");
        d.apply_currency(
            (13, 0),
            (14, 0),
            &crate::currency::CurrencySettings {
                currency: 2,
                negative: crate::currency::Negative::RedParentheses,
                ..Default::default()
            },
        );
        d.save(&source).unwrap();
        let soffice = std::env::var("SOFFICE").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "/Applications/LibreOffice.app/Contents/MacOS/soffice".into()
            } else {
                "soffice".into()
            }
        });
        let status = std::process::Command::new(soffice)
            .arg(format!(
                "-env:UserInstallation=file://{}",
                dir.path().join("profile").display()
            ))
            .args(["--headless", "--convert-to", "ods", "--outdir"])
            .arg(&output)
            .arg(&source)
            .status()
            .expect("launch LibreOffice");
        assert!(status.success());
        let mut d = Document::open(&output.join("input.ods")).unwrap();
        assert_eq!(d.display(8, 1), "3100");
        assert_eq!(d.display(10, 0), "On track");
        assert_eq!(d.book.sheet(0).value(11, 0).as_f64_opt(), Some(1600.0));
        assert_eq!(d.book.sheet(0).value(12, 0).as_f64_opt(), Some(1223.0));
        assert_eq!(d.display(11, 0), "1600");
        assert_eq!(d.display(12, 0), "1223");
        assert_eq!(d.book.sheet(0).value(16, 0).as_f64_opt(), Some(438.0));
        assert_eq!(d.book.sheet(0).value(17, 0).as_f64_opt(), Some(326.5));
        assert_eq!(d.display(16, 0), "438");
        assert_eq!(d.display(17, 0), "326.5");
        assert_eq!(d.formatted(13, 0, &d.base_style(13, 0)), "€1,234.57");
        assert_eq!(d.formatted(14, 0, &d.base_style(14, 0)), "(€1,234.57)");
        assert_eq!(d.format_color(14, 0, &d.base_style(14, 0)), Some("#ff0000"));
        assert_eq!(
            d.visible_style(6, 3)
                .cellstyle()
                .attr("fo:background-color"),
            Some("#ffe1de")
        );
        d.set(3, 1, "1700");
        d.save(&dir.path().join("back.ods")).unwrap();
        let d = Document::open(&dir.path().join("back.ods")).unwrap();
        assert_eq!(d.display(3, 3), "150");
    }
}

#[cfg(test)]
mod formats {
    use super::*;
    #[test]
    fn blank_cells_and_precision() {
        let mut d = Document::blank();
        d.set(0, 0, "1.234567891234");
        assert_eq!(d.raw(0, 0), "1.234567891234");
        let mut f = spreadsheet_ods::ValueFormatNumber::new_empty();
        f.part_number().fixed_decimal_places(2).build();
        let f = d.book.add_number_format(f);
        let style = CellStyle::new("two-decimals", &f);
        assert_eq!(d.formatted(0, 0, &style), "1.23");
        assert_eq!(d.formatted(1, 0, &style), "");
    }
}

#[cfg(test)]
mod currency_roundtrip_tests {
    use super::*;
    use crate::currency::{CurrencySettings, Negative};
    #[test]
    fn currency_preserves_values_history_and_ods() {
        let mut d = Document::blank();
        d.set(0, 0, "1234.567");
        d.set(1, 0, "-1234.567");
        let settings = CurrencySettings {
            currency: 2,
            negative: Negative::RedParentheses,
            ..Default::default()
        };
        d.apply_currency((0, 0), (2, 0), &settings);
        let verify = |d: &Document| {
            assert_eq!(d.formatted(0, 0, &d.base_style(0, 0)), "€1,234.57");
            assert_eq!(d.formatted(1, 0, &d.base_style(1, 0)), "(€1,234.57)");
            assert_eq!(d.format_color(1, 0, &d.base_style(1, 0)), Some("#ff0000"));
            assert_eq!(d.formatted(2, 0, &d.base_style(2, 0)), "");
            assert_eq!(d.raw(0, 0), "1234.567");
        };
        verify(&d);
        d.undo();
        assert!(d.base_style(0, 0).value_format().is_none());
        d.redo();
        verify(&d);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("currency.ods");
        d.save(&path).unwrap();
        let d = Document::open(&path).unwrap();
        verify(&d);
    }
}

#[cfg(test)]
mod median_roundtrip_tests {
    use super::*;
    #[test]
    fn median_ods_recalculation() {
        let mut d = Document::blank();
        for (r, v) in [1, 9, 4, 20].iter().enumerate() {
            d.set(r as u32, 0, &v.to_string());
        }
        d.set(0, 1, "=MEDIAN(A1:A4)");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("median.ods");
        d.save(&path).unwrap();
        let mut d = Document::open(&path).unwrap();
        assert_eq!(d.book.sheet(0).value(0, 1).as_f64_opt(), Some(6.5));
        assert_eq!(d.raw(0, 1), "=MEDIAN(A1:A4)");
        d.set(1, 0, "3");
        assert_eq!(d.display(0, 1), "3.5");
    }
}
