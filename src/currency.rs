//! Currency display options expressed as native ODF number styles.
use spreadsheet_ods::{
    ValueFormatCurrency, WorkBook,
    condition::ValueCondition,
    format::{FormatPart, FormatPartType, ValueFormatRef, ValueFormatTrait, ValueStyleMap},
};

pub const CURRENCIES: &[(&str, &str, &str)] = &[
    ("AUD", "$", "AU"),
    ("USD", "$", "US"),
    ("EUR", "€", "DE"),
    ("GBP", "£", "GB"),
    ("JPY", "¥", "JP"),
    ("CAD", "C$", "CA"),
    ("NZD", "NZ$", "NZ"),
    ("INR", "₹", "IN"),
    ("CHF", "CHF ", "CH"),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Negative {
    Minus,
    Parentheses,
    RedMinus,
    RedParentheses,
}
impl Negative {
    pub const ALL: [Self; 4] = [
        Self::Minus,
        Self::Parentheses,
        Self::RedMinus,
        Self::RedParentheses,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Minus => "Minus sign",
            Self::Parentheses => "Parentheses",
            Self::RedMinus => "Red with minus sign",
            Self::RedParentheses => "Red with parentheses",
        }
    }
    pub fn red(self) -> bool {
        matches!(self, Self::RedMinus | Self::RedParentheses)
    }
    fn parentheses(self) -> bool {
        matches!(self, Self::Parentheses | Self::RedParentheses)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrencySettings {
    pub currency: usize,
    pub decimals: u8,
    pub grouping: bool,
    pub negative: Negative,
}
impl Default for CurrencySettings {
    fn default() -> Self {
        Self {
            currency: 0,
            decimals: 2,
            grouping: true,
            negative: Negative::Minus,
        }
    }
}
impl CurrencySettings {
    pub fn from_format(book: &WorkBook, name: &str) -> Option<Self> {
        let format = book.currency_format(name)?;
        let number = format
            .parts()
            .iter()
            .find(|p| p.part_type() == FormatPartType::Number)?;
        let symbol = format
            .parts()
            .iter()
            .find(|p| p.part_type() == FormatPartType::CurrencySymbol)?;
        let currency = CURRENCIES
            .iter()
            .position(|(_, s, c)| {
                Some(*s) == symbol.content().map(String::as_str)
                    && *c == symbol.attr_def("number:country", "")
            })
            .or_else(|| {
                CURRENCIES
                    .iter()
                    .position(|(_, s, _)| Some(*s) == symbol.content().map(String::as_str))
            })?;
        let (negative, _) = resolve(book, name, -1.0)?;
        let parentheses = negative
            .parts()
            .iter()
            .any(|p| p.content().is_some_and(|s| s.contains('(')));
        let red = negative
            .textstyle()
            .attr("fo:color")
            .is_some_and(|c| c.eq_ignore_ascii_case("#ff0000"));
        Some(Self {
            currency,
            decimals: number
                .attr_def("number:decimal-places", "2")
                .parse::<u8>()
                .ok()?
                .min(6),
            grouping: number.attr_def("number:grouping", "false") == "true",
            negative: match (parentheses, red) {
                (false, false) => Negative::Minus,
                (true, false) => Negative::Parentheses,
                (false, true) => Negative::RedMinus,
                (true, true) => Negative::RedParentheses,
            },
        })
    }
    pub fn label(&self) -> String {
        let (code, symbol, _) = CURRENCIES[self.currency];
        format!("{code} ({symbol})")
    }
    pub fn create_format(&self, book: &mut WorkBook) -> ValueFormatRef {
        let mut negative = self.format(true);
        if self.negative.red() {
            negative.textstyle_mut().set_attr("fo:color", "#ff0000");
        }
        let negative = book.add_currency_format(negative);
        let mut positive = self.format(false);
        positive.push_stylemap(ValueStyleMap::new(ValueCondition::value_lt(0), negative));
        book.add_currency_format(positive)
    }
    fn format(&self, negative: bool) -> ValueFormatCurrency {
        let mut f = ValueFormatCurrency::new_empty();
        if negative {
            f.part_text(if self.negative.parentheses() {
                "("
            } else {
                "-"
            })
            .build();
        }
        let (_, symbol, country) = CURRENCIES[self.currency];
        let mut part = FormatPart::new(FormatPartType::CurrencySymbol);
        part.set_content(symbol.to_string());
        part.set_attr("number:country", country.into());
        part.set_attr("number:language", "en".into());
        f.push_part(part);
        f.part_number()
            .fixed_decimal_places(self.decimals.min(6))
            .if_then(self.grouping, |p| p.grouping())
            .min_integer_digits(1)
            .build();
        if negative && self.negative.parentheses() {
            f.part_text(")").build();
        }
        f
    }
    pub fn preview(&self, value: f64) -> String {
        let mut book = WorkBook::new_empty();
        let name = self.create_format(&mut book);
        let (format, absolute) = resolve(&book, name.as_str(), value).unwrap();
        render(format, value, absolute).unwrap()
    }
}
// A negative subformat selected by an ODF style map formats the absolute
// magnitude; its literal parts provide the minus sign or parentheses.
pub fn resolve<'a>(
    book: &'a WorkBook,
    name: &str,
    value: f64,
) -> Option<(&'a ValueFormatCurrency, bool)> {
    let mut format = book.currency_format(name)?;
    let mut mapped = false;
    for _ in 0..16 {
        let next = format
            .stylemaps()
            .and_then(|maps| {
                maps.iter()
                    .find(|m| condition_matches(&m.condition().to_string(), value))
            })
            .and_then(|m| book.currency_format(m.applied_style()));
        if let Some(next) = next {
            format = next;
            mapped = value < 0.0;
        } else {
            break;
        }
    }
    Some((format, mapped))
}
fn condition_matches(condition: &str, value: f64) -> bool {
    let Some(rhs) = condition.strip_prefix("value()") else {
        return false;
    };
    for op in ["<=", ">=", "!=", "<>", "<", ">", "="] {
        if let Some(n) = rhs
            .strip_prefix(op)
            .and_then(|s| s.trim().parse::<f64>().ok())
        {
            return match op {
                "<=" => value <= n,
                ">=" => value >= n,
                "<" => value < n,
                ">" => value > n,
                "=" => value == n,
                _ => value != n,
            };
        }
    }
    false
}
pub fn render(format: &ValueFormatCurrency, value: f64, absolute: bool) -> Option<String> {
    let mut text = String::new();
    for part in format.parts() {
        match part.part_type() {
            FormatPartType::Number => {
                let places = part
                    .attr_def("number:decimal-places", "2")
                    .parse::<usize>()
                    .ok()?
                    .min(12);
                let mut n = format!("{:.*}", places, if absolute { value.abs() } else { value });
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
                    text.push_str(s);
                }
            }
            _ => return None,
        }
    }
    Some(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip() {
        for currency in 0..CURRENCIES.len() {
            for negative in Negative::ALL {
                let s = CurrencySettings {
                    currency,
                    negative,
                    decimals: 3,
                    grouping: false,
                };
                let mut book = WorkBook::new_empty();
                let name = s.create_format(&mut book);
                assert_eq!(CurrencySettings::from_format(&book, name.as_str()), Some(s));
            }
        }
    }
    #[test]
    fn currency_options() {
        let mut s = CurrencySettings::default();
        assert_eq!(s.preview(1234.567), "$1,234.57");
        assert_eq!(s.preview(-1234.567), "-$1,234.57");
        s.negative = Negative::RedParentheses;
        assert_eq!(s.preview(-1234.567), "($1,234.57)");
        s.currency = 2;
        s.decimals = 0;
        s.grouping = false;
        assert_eq!(s.preview(1234.56), "€1235");
    }
}
