//! Deterministic, offline formula evaluation. Unsupported formulas return an error.
use spreadsheet_ods::{Value, WorkBook};
use std::collections::HashSet;

pub const FUNCTIONS: &[(&str, &str, &str)] = &[
    ("SUM", "Add a list or range of numbers", "SUM(A1:A10)"),
    (
        "SUMIF",
        "Add numbers that meet one condition",
        "SUMIF(A1:A10;\"Food\";B1:B10)",
    ),
    (
        "SUMIFS",
        "Add numbers that meet every condition",
        "SUMIFS(C1:C10;A1:A10;\"Food\";B1:B10;\">=100\")",
    ),
    ("AVERAGE", "Find the average", "AVERAGE(A1:A10)"),
    (
        "MEDIAN",
        "Find the middle value (average the middle two for an even count)",
        "MEDIAN(A1:A10)",
    ),
    ("MIN", "Find the smallest number", "MIN(A1:A10)"),
    ("MAX", "Find the largest number", "MAX(A1:A10)"),
    ("COUNT", "Count numeric cells", "COUNT(A1:A10)"),
    ("IF", "Choose a value using a condition", "IF(A1>100;1;0)"),
    (
        "ROUND",
        "Round to a number of decimal places",
        "ROUND(A1;2)",
    ),
    ("ABS", "Get the positive magnitude", "ABS(A1)"),
    (
        "IFERROR",
        "Use a fallback when a formula has an error",
        "IFERROR(A1/B1;0)",
    ),
    ("COUNTA", "Count non-empty cells", "COUNTA(A1:A10)"),
    ("PRODUCT", "Multiply a range of numbers", "PRODUCT(A1:A10)"),
    ("SQRT", "Find the square root", "SQRT(A1)"),
    ("POWER", "Raise a number to a power", "POWER(A1;2)"),
    ("MOD", "Find the remainder after division", "MOD(A1;2)"),
    (
        "AND",
        "Check whether all conditions are true",
        "AND(A1>0;B1>0)",
    ),
    ("OR", "Check whether any condition is true", "OR(A1>0;B1>0)"),
    ("NOT", "Reverse a logical condition", "NOT(A1>0)"),
    ("CONCAT", "Join text together", "CONCAT(A1;B1)"),
    ("LEN", "Count characters in text", "LEN(A1)"),
];

pub fn address(row: u32, col: u32) -> String {
    let mut n = col + 1;
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.insert(0, (b'A' + (n % 26) as u8) as char);
        n /= 26;
    }
    format!("{s}{}", row + 1)
}
pub fn coordinate(s: &str) -> Option<(u32, u32)> {
    let s = s.trim().trim_start_matches('.').replace('$', "");
    let split = s.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = s.split_at(split);
    if letters.is_empty() || !letters.bytes().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut col = 0u32;
    for c in letters.bytes() {
        col = col
            .checked_mul(26)?
            .checked_add((c.to_ascii_uppercase() - b'A' + 1) as u32)?;
    }
    Some((
        digits.parse::<u32>().ok()?.checked_sub(1)?,
        col.checked_sub(1)?,
    ))
}
// Convert only unquoted A1 references, keeping string literals intact.
pub fn to_odf(input: &str) -> String {
    let mut out = String::from("of:=");
    let chars: Vec<char> = input.trim_start_matches('=').chars().collect();
    let mut i = 0;
    let mut quoted = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            quoted = !quoted;
            out.push(c);
            i += 1;
            continue;
        }
        if !quoted && c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && matches!(chars[i], 'e' | 'E') {
                out.push(chars[i]);
                i += 1;
                if i < chars.len() && matches!(chars[i], '+' | '-') {
                    out.push(chars[i]);
                    i += 1;
                }
                while i < chars.len() && chars[i].is_ascii_digit() {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }
        if !quoted && (c.is_ascii_alphabetic() || c == '$') {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            if coordinate(&token).is_some() && chars.get(i) != Some(&'(') {
                out.push_str("[.");
                out.push_str(&token);
                if i < chars.len() && chars[i] == ':' {
                    i += 1;
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '$') {
                        i += 1;
                    }
                    out.push_str(":.");
                    out.extend(chars[start..i].iter());
                }
                out.push(']');
            } else {
                out.push_str(&token);
            }
        } else {
            out.push(if !quoted && c == ',' { ';' } else { c });
            i += 1;
        }
    }
    out
}
pub fn from_odf(s: &str) -> String {
    let s = s
        .strip_prefix("of:=")
        .or_else(|| s.strip_prefix("oooc:="))
        .unwrap_or(s.trim_start_matches('='));
    let mut out = String::from("=");
    let chars: Vec<_> = s.chars().collect();
    let mut i = 0;
    let mut quoted = false;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            quoted = !quoted;
            out.push(c);
            i += 1;
            continue;
        }
        if !quoted && c == '[' {
            let start = i + 1;
            if let Some(end) = chars[start..].iter().position(|c| *c == ']') {
                let reference: String = chars[start..start + end].iter().collect();
                if let Some(reference) = reference.strip_prefix('.') {
                    out.push_str(&reference.replace(":.", ":"));
                    i = start + end + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

pub fn evaluate(wb: &WorkBook, sheet: usize, row: u32, col: u32) -> Result<f64, String> {
    numeric(&evaluate_value(wb, sheet, row, col)?)
}
pub fn evaluate_value(wb: &WorkBook, sheet: usize, row: u32, col: u32) -> Result<Value, String> {
    cell(wb, sheet, row, col, &mut HashSet::new())
}
fn numeric(v: &Value) -> Result<f64, String> {
    match v {
        Value::Empty => Ok(0.0),
        Value::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        _ => v.as_f64_opt().ok_or("#VALUE!".into()),
    }
}
fn cell(
    wb: &WorkBook,
    s: usize,
    r: u32,
    c: u32,
    seen: &mut HashSet<(u32, u32)>,
) -> Result<Value, String> {
    if seen.len() > 128 || !seen.insert((r, c)) {
        return Err("#CYCLE!".into());
    }
    let result = if let Some(f) = wb.sheet(s).formula(r, c) {
        let source = from_odf(f);
        (|| {
            let mut parser = Parser {
                text: &source.as_bytes()[1..],
                pos: 0,
                depth: 0,
            };
            let node = parser.expr()?;
            parser.ws();
            if parser.pos != parser.text.len() {
                return Err("#NAME?".into());
            }
            eval(&node, wb, s, seen, 0)
        })()
    } else {
        Ok(wb.sheet(s).value(r, c).clone())
    };
    seen.remove(&(r, c));
    result
}
#[derive(Debug)]
enum Node {
    Literal(Value),
    Cell(u32, u32),
    Range((u32, u32), (u32, u32)),
    Unary(u8, Box<Node>),
    Binary(String, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}
struct Parser<'a> {
    text: &'a [u8],
    pos: usize,
    depth: usize,
}
impl Parser<'_> {
    fn ws(&mut self) {
        while self.text.get(self.pos).is_some_and(u8::is_ascii_whitespace) {
            self.pos += 1
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        self.ws();
        if self.text.get(self.pos) == Some(&c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<Node, String> {
        self.depth += 1;
        if self.depth > 128 || self.text.len() > 32_768 {
            return Err("#LIMIT!".into());
        }
        let mut node = self.add()?;
        let op = if self.eat(b'>') {
            Some(if self.eat(b'=') { ">=" } else { ">" })
        } else if self.eat(b'<') {
            Some(if self.eat(b'=') {
                "<="
            } else if self.eat(b'>') {
                "<>"
            } else {
                "<"
            })
        } else if self.eat(b'=') {
            Some("=")
        } else {
            None
        };
        if let Some(op) = op {
            node = Node::Binary(op.into(), Box::new(node), Box::new(self.add()?));
        }
        self.depth -= 1;
        Ok(node)
    }
    fn add(&mut self) -> Result<Node, String> {
        let mut n = self.mul()?;
        loop {
            let op = if self.eat(b'+') {
                "+"
            } else if self.eat(b'-') {
                "-"
            } else if self.eat(b'&') {
                "&"
            } else {
                break;
            };
            n = Node::Binary(op.into(), Box::new(n), Box::new(self.mul()?));
        }
        Ok(n)
    }
    fn mul(&mut self) -> Result<Node, String> {
        let mut n = self.power()?;
        loop {
            let op = if self.eat(b'*') {
                "*"
            } else if self.eat(b'/') {
                "/"
            } else {
                break;
            };
            n = Node::Binary(op.into(), Box::new(n), Box::new(self.power()?));
        }
        Ok(n)
    }
    fn power(&mut self) -> Result<Node, String> {
        let n = self.atom()?;
        if self.eat(b'^') {
            self.depth += 1;
            if self.depth > 128 {
                return Err("#LIMIT!".into());
            }
            let rhs = self.power()?;
            self.depth -= 1;
            Ok(Node::Binary("^".into(), Box::new(n), Box::new(rhs)))
        } else {
            Ok(n)
        }
    }
    fn atom(&mut self) -> Result<Node, String> {
        self.depth += 1;
        if self.depth > 128 {
            return Err("#LIMIT!".into());
        }
        let mut n = if self.eat(b'-') {
            Node::Unary(b'-', Box::new(self.atom()?))
        } else if self.eat(b'+') {
            self.atom()?
        } else if self.eat(b'(') {
            let n = self.expr()?;
            if !self.eat(b')') {
                return Err("#SYNTAX!".into());
            }
            n
        } else if self.eat(b'"') {
            let mut bytes = vec![];
            loop {
                let c = *self.text.get(self.pos).ok_or("#SYNTAX!")?;
                self.pos += 1;
                if c == b'"' {
                    if self.text.get(self.pos) == Some(&b'"') {
                        self.pos += 1;
                        bytes.push(c)
                    } else {
                        break;
                    }
                } else {
                    bytes.push(c)
                }
            }
            Node::Literal(Value::Text(
                String::from_utf8(bytes).map_err(|_| "#SYNTAX!")?,
            ))
        } else {
            self.ws();
            let start = self.pos;
            if self
                .text
                .get(self.pos)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                while self
                    .text
                    .get(self.pos)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
                {
                    self.pos += 1;
                }
                if self
                    .text
                    .get(self.pos)
                    .is_some_and(|c| *c == b'e' || *c == b'E')
                {
                    self.pos += 1;
                    if self
                        .text
                        .get(self.pos)
                        .is_some_and(|c| *c == b'+' || *c == b'-')
                    {
                        self.pos += 1;
                    }
                    while self.text.get(self.pos).is_some_and(u8::is_ascii_digit) {
                        self.pos += 1;
                    }
                }
                let num = std::str::from_utf8(&self.text[start..self.pos])
                    .unwrap()
                    .parse::<f64>()
                    .map_err(|_| "#SYNTAX!")?;
                Node::Literal(Value::Number(num))
            } else {
                while self
                    .text
                    .get(self.pos)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'$' || *c == b'_')
                {
                    self.pos += 1;
                }
                let name = std::str::from_utf8(&self.text[start..self.pos])
                    .unwrap()
                    .to_uppercase();
                if name.is_empty() {
                    return Err("#NAME?".into());
                }
                if self.eat(b'(') {
                    let mut args = vec![];
                    if !self.eat(b')') {
                        loop {
                            args.push(self.expr()?);
                            if self.eat(b')') {
                                break;
                            }
                            if !self.eat(b';') && !self.eat(b',') {
                                return Err("#SYNTAX!".into());
                            }
                        }
                    }
                    Node::Call(name, args)
                } else if name == "TRUE" || name == "FALSE" {
                    Node::Literal(Value::Boolean(name == "TRUE"))
                } else {
                    let (r, c) = coordinate(&name).ok_or("#NAME?")?;
                    if self.eat(b':') {
                        self.ws();
                        let start = self.pos;
                        while self
                            .text
                            .get(self.pos)
                            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'$')
                        {
                            self.pos += 1;
                        }
                        let end =
                            coordinate(std::str::from_utf8(&self.text[start..self.pos]).unwrap())
                                .ok_or("#REF!")?;
                        Node::Range((r, c), end)
                    } else {
                        Node::Cell(r, c)
                    }
                }
            }
        };
        if self.eat(b'%') {
            n = Node::Unary(b'%', Box::new(n));
        }
        self.depth -= 1;
        Ok(n)
    }
}
fn eval(
    n: &Node,
    wb: &WorkBook,
    s: usize,
    seen: &mut HashSet<(u32, u32)>,
    depth: usize,
) -> Result<Value, String> {
    if depth > 128 {
        return Err("#LIMIT!".into());
    }
    let get = |n: &Node, seen: &mut HashSet<(u32, u32)>| eval(n, wb, s, seen, depth + 1);
    let v = match n {
        Node::Literal(v) => v.clone(),
        Node::Cell(r, c) => cell(wb, s, *r, *c, seen)?,
        Node::Range(..) => return Err("#VALUE!".into()),
        Node::Unary(op, n) => {
            let v = numeric(&get(n, seen)?)?;
            Value::Number(if *op == b'-' { -v } else { v / 100.0 })
        }
        Node::Binary(op, a, b) => {
            let a = get(a, seen)?;
            let b = get(b, seen)?;
            if op == "&" {
                Value::Text(format!("{}{}", scalar_text(&a), scalar_text(&b)))
            } else if ["=", "<>", "<", ">", "<=", ">="].contains(&op.as_str()) {
                let cmp = if matches!(a, Value::Text(_)) || matches!(b, Value::Text(_)) {
                    scalar_text(&a)
                        .to_lowercase()
                        .partial_cmp(&scalar_text(&b).to_lowercase())
                } else {
                    numeric(&a)?.partial_cmp(&numeric(&b)?)
                };
                use std::cmp::Ordering::*;
                let result = match op.as_str() {
                    "=" => cmp == Some(Equal),
                    "<>" => cmp != Some(Equal),
                    "<" => cmp == Some(Less),
                    ">" => cmp == Some(Greater),
                    "<=" => matches!(cmp, Some(Less | Equal)),
                    _ => matches!(cmp, Some(Greater | Equal)),
                };
                Value::Boolean(result)
            } else {
                let a = numeric(&a)?;
                let b = numeric(&b)?;
                Value::Number(match op.as_str() {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" => {
                        if b == 0.0 {
                            return Err("#DIV/0!".into());
                        }
                        a / b
                    }
                    "^" => a.powf(b),
                    _ => return Err("#NAME?".into()),
                })
            }
        }
        Node::Call(name, args) => {
            if name == "MEDIAN" {
                return median(args, wb, s, seen, depth + 1);
            }
            if name == "SUMIF" || name == "SUMIFS" {
                return conditional_sum(name, args, wb, s, seen, depth + 1);
            }
            if name == "IF" {
                if args.len() != 3 {
                    return Err("#VALUE!".into());
                }
                let condition = numeric(&get(&args[0], seen)?)?;
                return get(&args[if condition != 0.0 { 1 } else { 2 }], seen);
            }
            if name == "IFERROR" {
                if args.len() != 2 {
                    return Err("#VALUE!".into());
                }
                return get(&args[0], seen).or_else(|_| get(&args[1], seen));
            }
            let mut values = vec![];
            for arg in args {
                if let Node::Range((r1, c1), (r2, c2)) = arg {
                    if r2 < r1
                        || c2 < c1
                        || (*r2 - *r1 + 1) as u64 * (*c2 - *c1 + 1) as u64 > 100_000
                    {
                        return Err("#REF!".into());
                    }
                    for r in *r1..=*r2 {
                        for c in *c1..=*c2 {
                            values.push(cell(wb, s, r, c, seen)?);
                        }
                    }
                } else {
                    values.push(get(arg, seen)?);
                }
            }
            let nums: Vec<_> = values.iter().filter_map(Value::as_f64_opt).collect();
            let argnum = |i: usize| values.get(i).ok_or("#VALUE!".to_string()).and_then(numeric);
            match name.as_str() {
                "SUM" => Value::Number(nums.iter().sum()),
                "PRODUCT" => Value::Number(nums.iter().product()),
                "COUNT" => Value::Number(nums.len() as f64),
                "COUNTA" => Value::Number(
                    values.iter().filter(|v| !matches!(v, Value::Empty)).count() as f64,
                ),
                "AVERAGE" => {
                    if nums.is_empty() {
                        return Err("#DIV/0!".into());
                    }
                    Value::Number(nums.iter().sum::<f64>() / nums.len() as f64)
                }
                "MIN" => Value::Number(nums.into_iter().reduce(f64::min).unwrap_or(0.0)),
                "MAX" => Value::Number(nums.into_iter().reduce(f64::max).unwrap_or(0.0)),
                "ABS" if values.len() == 1 => Value::Number(argnum(0)?.abs()),
                "ROUND" if values.len() == 2 => {
                    let p = 10f64.powf(argnum(1)?.trunc());
                    Value::Number((argnum(0)? * p).round() / p)
                }
                "SQRT" if values.len() == 1 => Value::Number(argnum(0)?.sqrt()),
                "POWER" if values.len() == 2 => Value::Number(argnum(0)?.powf(argnum(1)?)),
                "MOD" if values.len() == 2 => {
                    let a = argnum(0)?;
                    let b = argnum(1)?;
                    if b == 0.0 {
                        return Err("#DIV/0!".into());
                    }
                    Value::Number(a - b * (a / b).floor())
                }
                "AND" | "OR" if !values.is_empty() => {
                    let b: Result<Vec<_>, _> = values.iter().map(numeric).collect();
                    let b = b?;
                    Value::Boolean(if name == "AND" {
                        b.iter().all(|v| *v != 0.0)
                    } else {
                        b.iter().any(|v| *v != 0.0)
                    })
                }
                "NOT" if values.len() == 1 => Value::Boolean(argnum(0)? == 0.0),
                "CONCAT" | "CONCATENATE" => Value::Text(values.iter().map(scalar_text).collect()),
                "LEN" if values.len() == 1 => {
                    Value::Number(scalar_text(&values[0]).chars().count() as f64)
                }
                _ => return Err("#NAME?".into()),
            }
        }
    };
    if v.as_f64_opt().is_some_and(|v| !v.is_finite()) {
        return Err("#NUM!".into());
    }
    Ok(v)
}
fn median(
    args: &[Node],
    wb: &WorkBook,
    s: usize,
    seen: &mut HashSet<(u32, u32)>,
    depth: usize,
) -> Result<Value, String> {
    if args.is_empty() || args.len() > 255 {
        return Err("#VALUE!".into());
    }
    let mut numbers = vec![];
    for arg in args {
        if matches!(arg, Node::Range(..) | Node::Cell(..)) {
            let range = RectRange::from_node(arg)?;
            for r in 0..range.rows {
                for c in 0..range.cols {
                    let (rr, cc) = range.at(r, c)?;
                    if let Some(n) = cell(wb, s, rr, cc, seen)?.as_f64_opt() {
                        numbers.push(n);
                    }
                }
            }
        } else {
            let value = eval(arg, wb, s, seen, depth)?;
            let n = match value {
                Value::Text(text) => text.trim().parse::<f64>().map_err(|_| "#VALUE!")?,
                v => numeric(&v)?,
            };
            numbers.push(n);
        }
    }
    if numbers.is_empty() || numbers.iter().any(|n| !n.is_finite()) {
        return Err("#NUM!".into());
    }
    numbers.sort_unstable_by(f64::total_cmp);
    let middle = numbers.len() / 2;
    let result = if numbers.len().is_multiple_of(2) {
        let (a, b) = (numbers[middle - 1], numbers[middle]);
        // Avoid overflowing when averaging two large finite values.
        if (a >= 0.0 && b >= 0.0) || (a <= 0.0 && b <= 0.0) {
            a + (b - a) / 2.0
        } else {
            (a + b) / 2.0
        }
    } else {
        numbers[middle]
    };
    Ok(Value::Number(result))
}

// Conditional sums keep range coordinates rather than flattening arguments: each
// criterion must be checked against the corresponding cell in the sum range.
#[derive(Clone, Copy)]
struct RectRange {
    row: u32,
    col: u32,
    rows: u32,
    cols: u32,
}
impl RectRange {
    fn from_node(node: &Node) -> Result<Self, String> {
        let (start, end) = match node {
            Node::Cell(r, c) => ((*r, *c), (*r, *c)),
            Node::Range(a, b) => (*a, *b),
            _ => return Err("#VALUE!".into()),
        };
        let rows = end
            .0
            .checked_sub(start.0)
            .and_then(|n| n.checked_add(1))
            .ok_or("#REF!")?;
        let cols = end
            .1
            .checked_sub(start.1)
            .and_then(|n| n.checked_add(1))
            .ok_or("#REF!")?;
        if rows as u64 * cols as u64 > 100_000 {
            return Err("#LIMIT!".into());
        }
        Ok(Self {
            row: start.0,
            col: start.1,
            rows,
            cols,
        })
    }
    fn at(self, r: u32, c: u32) -> Result<(u32, u32), String> {
        Ok((
            self.row.checked_add(r).ok_or("#REF!")?,
            self.col.checked_add(c).ok_or("#REF!")?,
        ))
    }
}
fn conditional_sum(
    name: &str,
    args: &[Node],
    wb: &WorkBook,
    s: usize,
    seen: &mut HashSet<(u32, u32)>,
    depth: usize,
) -> Result<Value, String> {
    let (sum_range, conditions) = if name == "SUMIF" {
        if !(2..=3).contains(&args.len()) {
            return Err("#VALUE!".into());
        }
        let range = RectRange::from_node(&args[0])?;
        let mut sum = if args.len() == 3 {
            RectRange::from_node(&args[2])?
        } else {
            range
        };
        // Excel uses the upper-left sum cell and the dimensions of criteria_range.
        sum.rows = range.rows;
        sum.cols = range.cols;
        (
            sum,
            vec![(range, Criterion::new(eval(&args[1], wb, s, seen, depth)?))],
        )
    } else {
        if args.len() < 3 || args.len().is_multiple_of(2) || args.len() > 255 {
            return Err("#VALUE!".into());
        }
        let sum = RectRange::from_node(&args[0])?;
        let mut conditions = vec![];
        for pair in args[1..].as_chunks::<2>().0 {
            let range = RectRange::from_node(&pair[0])?;
            if range.rows != sum.rows || range.cols != sum.cols {
                return Err("#VALUE!".into());
            }
            conditions.push((range, Criterion::new(eval(&pair[1], wb, s, seen, depth)?)));
        }
        (sum, conditions)
    };
    let mut total = 0.0;
    for r in 0..sum_range.rows {
        for c in 0..sum_range.cols {
            let mut matches = true;
            for (range, criterion) in &conditions {
                let (rr, cc) = range.at(r, c)?;
                if !criterion.matches(&cell(wb, s, rr, cc, seen)?) {
                    matches = false;
                    break;
                }
            }
            if matches {
                let (rr, cc) = sum_range.at(r, c)?;
                if let Some(v) = cell(wb, s, rr, cc, seen)?.as_f64_opt() {
                    total += v;
                }
            }
        }
    }
    if !total.is_finite() {
        return Err("#NUM!".into());
    }
    Ok(Value::Number(total))
}
#[derive(Debug)]
enum Criterion {
    Number(String, f64),
    Boolean(bool),
    Text(String, String),
}
impl Criterion {
    fn new(value: Value) -> Self {
        match value {
            Value::Empty => Self::Number("=".into(), 0.0),
            Value::Boolean(b) => Self::Boolean(b),
            Value::Number(v) | Value::Currency(v, _) | Value::Percentage(v) => {
                Self::Number("=".into(), v)
            }
            value => {
                let text = scalar_text(&value);
                let (op, rhs) = ["<=", ">=", "<>", "=", "<", ">"]
                    .iter()
                    .find_map(|op| text.strip_prefix(op).map(|rhs| (*op, rhs)))
                    .unwrap_or(("=", text.as_str()));
                if let Ok(n) = rhs.trim().parse::<f64>() {
                    Self::Number(op.into(), n)
                } else {
                    Self::Text(op.into(), rhs.to_lowercase())
                }
            }
        }
    }
    fn matches(&self, value: &Value) -> bool {
        match self {
            Self::Number(op, n) => {
                let v = match value {
                    Value::Empty => Some(0.0),
                    Value::Text(s) => s.parse::<f64>().ok(),
                    v => v.as_f64_opt(),
                };
                v.map(|v| compare(op, v.partial_cmp(n)))
                    .unwrap_or(op == "<>")
            }
            Self::Boolean(b) => matches!(value,Value::Boolean(v) if v==b),
            Self::Text(op, pattern) => {
                let text = scalar_text(value).to_lowercase();
                if pattern.is_empty() {
                    let empty = matches!(value, Value::Empty)
                        || matches!(value,Value::Text(s) if s.is_empty());
                    return match op.as_str() {
                        "=" => empty,
                        "<>" => !empty,
                        _ => compare(op, text.as_str().partial_cmp(pattern.as_str())),
                    };
                }
                if op == "=" || op == "<>" {
                    let matched = matches!(value, Value::Text(_) | Value::TextXml(_))
                        && wildcard_matches(pattern, &text);
                    if op == "=" { matched } else { !matched }
                } else {
                    matches!(value, Value::Text(_) | Value::TextXml(_))
                        && compare(op, text.as_str().partial_cmp(pattern.as_str()))
                }
            }
        }
    }
}
fn compare(op: &str, order: Option<std::cmp::Ordering>) -> bool {
    use std::cmp::Ordering::*;
    match op {
        "=" => order == Some(Equal),
        "<>" => order != Some(Equal),
        "<" => order == Some(Less),
        ">" => order == Some(Greater),
        "<=" => matches!(order, Some(Less | Equal)),
        ">=" => matches!(order, Some(Greater | Equal)),
        _ => false,
    }
}
#[derive(Clone, Copy)]
enum Wildcard {
    Star,
    Any,
    Literal(char),
}
fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let mut chars = pattern.chars();
    let mut tokens = vec![];
    while let Some(c) = chars.next() {
        tokens.push(match c {
            '*' => Wildcard::Star,
            '?' => Wildcard::Any,
            '~' => Wildcard::Literal(chars.next().unwrap_or('~')),
            c => Wildcard::Literal(c),
        });
    }
    let text: Vec<_> = text.chars().collect();
    let (mut p, mut t, mut star, mut retry) = (0, 0, None, 0);
    while t < text.len() {
        match tokens.get(p) {
            Some(Wildcard::Any) => {
                p += 1;
                t += 1;
            }
            Some(Wildcard::Literal(c)) if *c == text[t] => {
                p += 1;
                t += 1;
            }
            Some(Wildcard::Star) => {
                star = Some(p);
                p += 1;
                retry = t;
            }
            _ => {
                if let Some(i) = star {
                    retry += 1;
                    t = retry;
                    p = i + 1;
                } else {
                    return false;
                }
            }
        }
    }
    while matches!(tokens.get(p), Some(Wildcard::Star)) {
        p += 1;
    }
    p == tokens.len()
}
fn scalar_text(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.into(),
        Value::Number(n) | Value::Percentage(n) | Value::Currency(n, _) => n.to_string(),
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
        _ => v.as_string_or_default(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use spreadsheet_ods::Sheet;
    fn calculate(source: &str) -> Result<Value, String> {
        let mut w = WorkBook::new_empty();
        let mut s = Sheet::new("Test");
        s.set_value(0, 0, 10);
        s.set_value(1, 0, 20);
        s.set_value(2, 0, "Text");
        s.set_formula(0, 1, to_odf(source));
        w.push_sheet(s);
        evaluate_value(&w, 0, 0, 1)
    }
    #[test]
    fn refs() {
        assert_eq!(coordinate("$AA$12"), Some((11, 26)));
        assert_eq!(address(11, 26), "AA12");
        assert_eq!(coordinate("A0"), None);
    }
    #[test]
    fn odf() {
        assert_eq!(to_odf("=SUM(A1:B3)+$C$2"), "of:=SUM([.A1:.B3])+[.$C$2]");
        assert_eq!(from_odf("of:=SUM([.A1:.B3])"), "=SUM(A1:B3)");
    }
    #[test]
    fn arithmetic() {
        assert_eq!(
            calculate("=SUM(A1:A3)*2+ROUND(1.234;2)"),
            Ok(Value::Number(61.23))
        );
        assert_eq!(calculate("=2^3^2"), Ok(Value::Number(512.0)));
        assert_eq!(calculate("=1e3+2"), Ok(Value::Number(1002.0)));
    }
    #[test]
    fn conditions() {
        assert_eq!(
            calculate("=IF(A1>5;\"Yes\";1/0)"),
            Ok(Value::Text("Yes".into()))
        );
        assert_eq!(calculate("=IFERROR(1/0;42)"), Ok(Value::Number(42.0)));
        assert_eq!(calculate("=AND(A1>5;A2<30)"), Ok(Value::Boolean(true)));
    }
    #[test]
    fn ranges() {
        assert_eq!(calculate("=COUNT(A1:A4)"), Ok(Value::Number(2.0)));
        assert_eq!(calculate("=COUNTA(A1:A4)"), Ok(Value::Number(3.0)));
        assert_eq!(calculate("=AVERAGE(A1:A4)"), Ok(Value::Number(15.0)));
    }
    #[test]
    fn median_lists_and_ranges() {
        for (source, expected) in [
            ("=MEDIAN(9;1;4)", 4.0),
            ("=MEDIAN(8,2,6,4)", 5.0),
            ("=MEDIAN(A1:A4)", 15.0),
            ("=MEDIAN(A1:A4;0)", 10.0),
            ("=MEDIAN(-3;-3;0;8)", -1.5),
            ("=MEDIAN(42)", 42.0),
            ("=MEDIAN(TRUE;\"3\";5)", 3.0),
            ("=MEDIAN(1e308;1e308)", 1e308),
            ("=MEDIAN(-1e308;1e308)", 0.0),
        ] {
            assert_eq!(calculate(source), Ok(Value::Number(expected)), "{source}");
        }
    }
    #[test]
    fn median_ignores_referenced_text_booleans_and_blanks() {
        let mut wb = WorkBook::new_empty();
        let mut sh = Sheet::new("Data");
        sh.set_value(0, 0, 0);
        sh.set_value(1, 0, 10);
        sh.set_value(2, 0, "100");
        sh.set_value(3, 0, true);
        sh.set_formula(0, 1, to_odf("=MEDIAN(A1:A5)"));
        wb.push_sheet(sh);
        assert_eq!(evaluate_value(&wb, 0, 0, 1), Ok(Value::Number(5.0)));
    }
    #[test]
    fn median_errors() {
        assert_eq!(calculate("=MEDIAN(A3:A4)"), Err("#NUM!".into()));
        assert_eq!(calculate("=MEDIAN()"), Err("#VALUE!".into()));
        assert_eq!(calculate("=MEDIAN(1;\"hello\")"), Err("#VALUE!".into()));
        assert_eq!(calculate("=MEDIAN(1;1/0)"), Err("#DIV/0!".into()));
        assert_eq!(calculate("=MEDIAN(B1)"), Err("#CYCLE!".into()));
    }
    #[test]
    fn text() {
        assert_eq!(
            calculate("=\"Hello \"&\"world\""),
            Ok(Value::Text("Hello world".into()))
        );
        assert_eq!(calculate("=LEN(\"café\")"), Ok(Value::Number(4.0)));
    }
    #[test]
    fn errors() {
        assert_eq!(calculate("=1/0"), Err("#DIV/0!".into()));
        assert_eq!(calculate("=SQRT(-1)"), Err("#NUM!".into()));
        assert_eq!(calculate("=WHAT(2)"), Err("#NAME?".into()));
        assert_eq!(calculate("=B1+1"), Err("#CYCLE!".into()));
    }
}

#[cfg(test)]
mod conditional_sum_tests {
    use super::*;
    use spreadsheet_ods::Sheet;
    fn workbook() -> WorkBook {
        let mut wb = WorkBook::new_empty();
        let mut sheet = Sheet::new("Sales");
        for (r, (category, region, amount)) in [
            ("Food", "East", 10),
            ("food", "West", 20),
            ("Rent", "East", 30),
            ("Food", "East", 40),
            ("Food*", "East", 50),
            ("", "", 60),
        ]
        .iter()
        .enumerate()
        {
            if !category.is_empty() {
                sheet.set_value(r as u32, 0, *category);
            }
            if !region.is_empty() {
                sheet.set_value(r as u32, 1, *region);
            }
            sheet.set_value(r as u32, 2, *amount);
        }
        sheet.set_value(0, 5, "Food");
        sheet.set_value(1, 5, 25);
        wb.push_sheet(sheet);
        wb
    }
    fn run(source: &str) -> Result<Value, String> {
        let mut wb = workbook();
        wb.sheet_mut(0).set_formula(0, 7, to_odf(source));
        evaluate_value(&wb, 0, 0, 7)
    }
    #[test]
    fn sumif_text_numeric_and_optional_sum_range() {
        for (source, expected) in [
            ("=SUMIF(A1:A6;\"FOOD\";C1:C6)", 70.0),
            ("=SUMIF(C1:C6;\">=30\")", 180.0),
            ("=SUMIF(A1:A6;F1;C1)", 70.0),
            ("=SUMIF(C1:C6;\">\"&F2)", 180.0),
            ("=SUMIF(A1;\"Food\";C1)", 10.0),
        ] {
            assert_eq!(run(source), Ok(Value::Number(expected)), "{source}");
        }
    }
    #[test]
    fn sumifs_multiple_conditions() {
        assert_eq!(
            run("=SUMIFS(C1:C6;A1:A6;\"food\";B1:B6;\"East\")"),
            Ok(Value::Number(50.0))
        );
        assert_eq!(
            run("=SUMIFS(C1:C6,A1:A6,\"Food*\",C1:C6,\">=20\",C1:C6,\"<50\")"),
            Ok(Value::Number(60.0))
        );
        assert_eq!(
            run("=SUMIFS(C1:C6;A1:A6;\"missing\")"),
            Ok(Value::Number(0.0))
        );
    }
    #[test]
    fn wildcards_escapes_and_blanks() {
        for (source, expected) in [
            ("=SUMIF(A1:A6;\"Food*\";C1:C6)", 120.0),
            ("=SUMIF(A1:A6;\"Food~*\";C1:C6)", 50.0),
            ("=SUMIF(A1:A6;\"F??d\";C1:C6)", 70.0),
            ("=SUMIF(A1:A6;\"\";C1:C6)", 60.0),
            ("=SUMIF(A1:A6;\"<>\";C1:C6)", 150.0),
            ("=SUMIF(A1:A6;\"<>Food*\";C1:C6)", 90.0),
            ("=SUMIF(A1:A6;0;C1:C6)", 60.0),
        ] {
            assert_eq!(run(source), Ok(Value::Number(expected)), "{source}");
        }
        assert!(wildcard_matches("a~?~*~~", "a?*~"));
        assert!(wildcard_matches("c?fé", "café"));
        assert!(!wildcard_matches("*x", "abc"));
    }
    #[test]
    fn range_dimensions_and_errors() {
        assert_eq!(run("=SUMIFS(C1:C6;A1:A5;\"Food\")"), Err("#VALUE!".into()));
        assert_eq!(run("=SUMIFS(C1:C2;A1:B1;\"Food\")"), Err("#VALUE!".into()));
        assert_eq!(run("=SUMIFS(C1:C6;A1:A6)"), Err("#VALUE!".into()));
        assert_eq!(run("=SUMIF(1;1)"), Err("#VALUE!".into()));
        assert_eq!(run("=SUMIF(A1:A6;\"Food\";H1)"), Err("#CYCLE!".into()));
    }
}
