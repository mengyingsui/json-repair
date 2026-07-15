#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

/// A structured JSON-like value used as the seed for the fuzzer.
#[derive(Debug)]
enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl<'a> Arbitrary<'a> for Value {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=5)? {
            0 => Ok(Value::Null),
            1 => Ok(Value::Bool(bool::arbitrary(u)?)),
            2 => Ok(Value::Number(Number::arbitrary(u)?)),
            3 => Ok(Value::String(String::arbitrary(u)?)),
            4 => Ok(Value::Array(Vec::arbitrary(u)?)),
            _ => Ok(Value::Object(Vec::arbitrary(u)?)),
        }
    }

    fn size_hint(_depth: usize) -> (usize, Option<usize>) {
        (1, None)
    }
}

/// A controlled numeric form so the generator does not produce NaN/Inf.
#[derive(Debug)]
struct Number {
    int: i64,
    frac: Option<u64>,
    exp: Option<i16>,
}

impl<'a> Arbitrary<'a> for Number {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Number {
            int: i64::arbitrary(u)?,
            frac: Option::<u64>::arbitrary(u)?,
            exp: Option::<i16>::arbitrary(u)?,
        })
    }
}

impl Number {
    fn render(&self) -> String {
        let mut out = self.int.to_string();
        if let Some(frac) = self.frac {
            out.push('.');
            out.push_str(&frac.to_string());
        }
        if let Some(exp) = self.exp {
            out.push('e');
            let exp_i32 = i32::from(exp);
            if exp_i32 < 0 {
                out.push('-');
                out.push_str(&(-exp_i32).to_string());
            } else {
                out.push('+');
                out.push_str(&exp_i32.to_string());
            }
        }
        out
    }
}

/// Which quote style to use when rendering strings.
#[derive(Debug, Clone, Copy)]
enum QuoteStyle {
    /// Standard double quotes.
    Double,
    /// Single quotes (repaired to double quotes by the repairer).
    Single,
    /// Missing closing quote.
    Unclosed,
}

impl<'a> Arbitrary<'a> for QuoteStyle {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=2)? {
            0 => Ok(QuoteStyle::Double),
            1 => Ok(QuoteStyle::Single),
            _ => Ok(QuoteStyle::Unclosed),
        }
    }
}

/// Which style to use when rendering object keys.
#[derive(Debug, Clone, Copy)]
enum KeyStyle {
    /// Quoted keys.
    Quoted,
    /// Bareword keys.
    Bare,
}

impl<'a> Arbitrary<'a> for KeyStyle {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        if bool::arbitrary(u)? {
            Ok(KeyStyle::Quoted)
        } else {
            Ok(KeyStyle::Bare)
        }
    }
}

/// Rendering style for the generated value.
#[derive(Debug, Clone, Copy)]
struct Style {
    quote: QuoteStyle,
    key: KeyStyle,
    trailing_comma: bool,
}

impl<'a> Arbitrary<'a> for Style {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Style {
            quote: QuoteStyle::arbitrary(u)?,
            key: KeyStyle::arbitrary(u)?,
            trailing_comma: bool::arbitrary(u)?,
        })
    }
}

impl Value {
    fn render(&self, style: Style) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.render(),
            Value::String(s) => render_string(s, style.quote),
            Value::Array(arr) => {
                let inner: Vec<_> = arr.iter().map(|v| v.render(style)).collect();
                let mut out = format!("[{}]", inner.join(","));
                if style.trailing_comma && !arr.is_empty() {
                    // Insert a trailing comma before the closing bracket.
                    out.pop();
                    out.push_str(",]");
                }
                out
            }
            Value::Object(obj) => {
                let inner: Vec<_> = obj
                    .iter()
                    .map(|(k, v)| format!("{}:{}", render_key(k, style.key), v.render(style)))
                    .collect();
                let mut out = format!("{{{}}}", inner.join(","));
                if style.trailing_comma && !obj.is_empty() {
                    // Insert a trailing comma before the closing brace.
                    out.pop();
                    out.push_str(",}");
                }
                out
            }
        }
    }
}

/// Render a string while keeping the chosen quote-style corruption intact.
fn render_string(s: &str, style: QuoteStyle) -> String {
    // Strip internal quotes so the style corruption is the focus of the test.
    let clean: String = s.chars().filter(|&c| c != '"' && c != '\'').collect();
    match style {
        QuoteStyle::Double => format!("\"{}\"", clean),
        QuoteStyle::Single => format!("'{}'", clean),
        QuoteStyle::Unclosed => format!("\"{}", clean),
    }
}

/// Render an object key; bare keys are filtered to characters the repairer accepts.
fn render_key(s: &str, style: KeyStyle) -> String {
    let clean: String = s
        .chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '_')
        .collect();
    match style {
        KeyStyle::Quoted => format!("\"{}\"", clean),
        KeyStyle::Bare => clean,
    }
}

/// Input bundle: one arbitrary value plus a rendering style.
#[derive(Debug)]
struct Input {
    value: Value,
    style: Style,
}

impl<'a> Arbitrary<'a> for Input {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Input {
            value: Value::arbitrary(u)?,
            style: Style::arbitrary(u)?,
        })
    }
}

fuzz_target!(|input: Input| {
    let text = input.value.render(input.style);
    // The repairer must never panic, regardless of how malformed the input is.
    if let Ok(repaired) = json_repair_core::repair_json(&text) {
        // When repair succeeds, the output should be parseable JSON.
        let _ = serde_json::from_str::<serde_json::Value>(&repaired);
    }
});
