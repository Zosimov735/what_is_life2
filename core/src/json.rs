//! Canonical JSON — the single canonicalizer.
//!
//! `docs/field-framework/ARCHITECTURE.md` locks canonicalization to one
//! implementation in the core, and this module is it: the writer every
//! serialized payload goes through and the strict reader every incoming body
//! is read with. The ten rules it carries are, in the document's order: UTF-8
//! with no byte-order mark; compact; keys in ascending UTF-8 byte order with
//! no duplicates; every declared field present; integers only, magnitude at
//! most 2^53 - 1, no minus zero, no fraction, no exponent; wider values as
//! fixed-width lowercase hex strings; the short escape set; arrays in semantic
//! order; the canonical bytes are what is hashed.
//!
//! The reader accepts ordinary JSON — a body arriving from the worker is a
//! structured-clone object whose key order is whatever the sender built, and
//! order is not a property of it until it is written. What the reader refuses
//! is what may never appear at all: a floating-point number, a duplicate key,
//! minus zero, and an integer too wide to carry.

/// The largest magnitude canonical JSON may carry as a number. Anything wider
/// — random-state keys and counters, run keys, hashes — is a hex string.
pub const MAX_SAFE_INT: i64 = (1i64 << 53) - 1;

/// How deeply values may nest. A limit exists so a malformed payload is
/// refused rather than run the reader out of stack.
const MAX_DEPTH: u32 = 64;

/// A JSON value, as the reader produces it and the writer emits it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    List(Vec<Json>),
    Map(Vec<(String, Json)>),
}

impl Json {
    /// The value under a key, for a map.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Map(pairs) => pairs.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Json::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Json::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Json::Map(_))
    }
}

/// Reads canonical JSON, and refuses what canonical JSON may never carry.
///
/// The error is a short machine-readable reason for the `detail` of an error
/// envelope; it is developer-facing and never shown.
pub fn parse(text: &str) -> Result<Json, &'static str> {
    let mut reader = Reader { bytes: text.as_bytes(), at: 0 };
    reader.skip_space();
    let value = reader.value(0)?;
    reader.skip_space();
    if reader.at != reader.bytes.len() {
        return Err("trailing_bytes");
    }
    Ok(value)
}

/// Parses and re-emits, which is what verifying a payload's canonical form
/// means: identical bytes back is the whole check.
pub fn canonicalize(text: &str) -> Result<String, &'static str> {
    let value = parse(text)?;
    let mut out = String::with_capacity(text.len());
    write_value(&mut out, &value)?;
    Ok(out)
}

/// Writes a value in canonical form: compact, keys ascending, integers only.
pub fn write_value(out: &mut String, value: &Json) -> Result<(), &'static str> {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Int(number) => write_int(out, *number)?,
        Json::Text(text) => write_text(out, text),
        Json::List(items) => {
            out.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    out.push(',');
                }
                write_value(out, item)?;
            }
            out.push(']');
        }
        Json::Map(pairs) => {
            let mut ordered: Vec<&(String, Json)> = pairs.iter().collect();
            ordered.sort_by(|left, right| left.0.cmp(&right.0));
            out.push('{');
            for (position, (key, item)) in ordered.iter().enumerate() {
                if position > 0 {
                    out.push(',');
                }
                if !is_canonical_key(key) {
                    return Err("bad_key");
                }
                write_text(out, key);
                out.push(':');
                write_value(out, item)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// Writes an integer. A magnitude past the carry limit is a defect: the value
/// belongs in a hex string instead.
pub fn write_int(out: &mut String, value: i64) -> Result<(), &'static str> {
    if !(-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&value) {
        return Err("number_too_wide");
    }
    out.push_str(&value.to_string());
    Ok(())
}

/// Writes a string with the locked escape set: the quote, the backslash, and
/// the control characters below U+0020; everything else stays raw UTF-8.
pub fn write_text(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str("\\u");
                for shift in [12, 8, 4, 0] {
                    out.push(HEX[((control as u32 >> shift) & 0xf) as usize] as char);
                }
            }
            plain => out.push(plain),
        }
    }
    out.push('"');
}

/// A string in canonical form, for the places that build one value at a time.
pub fn text_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    write_text(&mut out, value);
    out
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// A `u64` as the locked 16 lowercase hex characters.
pub fn hex16(value: u64) -> String {
    let mut out = String::with_capacity(16);
    for shift in (0..16).rev() {
        out.push(HEX[((value >> (shift * 4)) & 0xf) as usize] as char);
    }
    out
}

/// A `u128` as the locked 32 lowercase hex characters.
pub fn hex32(value: u128) -> String {
    let mut out = String::with_capacity(32);
    for shift in (0..32).rev() {
        out.push(HEX[((value >> (shift * 4)) & 0xf) as usize] as char);
    }
    out
}

/// Bytes as lowercase hex — 32 bytes make the locked 64 characters of a hash.
pub fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

/// True for the fixed-width lowercase hex string of `width` characters that
/// the hex rule requires.
pub fn is_hex(value: &str, width: usize) -> bool {
    value.len() == width && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// True for a key of the shape the naming rule requires: lowercase ASCII
/// words joined by underscores. A word opens with a letter in the first word
/// and may carry digits after it — `run_id`, `steer_x`, `payload_sha256`, `q`
/// — and no key is empty, capitalized, doubly underscored, or edged with an
/// underscore.
pub fn is_canonical_key(key: &str) -> bool {
    let mut words = key.split('_');
    let Some(first) = words.next() else {
        return false;
    };
    let opens = matches!(first.bytes().next(), Some(b'a'..=b'z'));
    let word_is_plain =
        |word: &str| !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    opens && word_is_plain(first) && words.all(word_is_plain)
}

/// Builds one object, in ascending key order.
///
/// Keys are written in the order the caller names them, and the order is what
/// the rule requires: the builder holds the previous key and refuses a
/// descending one, so a mis-ordered payload fails in the test run rather than
/// reaching a hash.
pub struct Obj<'a> {
    out: &'a mut String,
    previous: Option<String>,
}

impl<'a> Obj<'a> {
    pub fn new(out: &'a mut String) -> Self {
        out.push('{');
        Obj { out, previous: None }
    }

    /// Names the next key.
    ///
    /// Order and shape are held by debug assertions rather than by a returned
    /// error, deliberately: every key a writer names is a static literal
    /// written in this crate, so a violation is a defect in the source and
    /// not a fault in any input, and `cargo test` builds with debug
    /// assertions on and serializes the whole payload. An input's keys are a
    /// different matter and are checked by the reader, which returns
    /// `bad_key`.
    fn key(&mut self, key: &str) {
        debug_assert!(
            self.previous.as_deref().is_none_or(|earlier| earlier < key),
            "canonical JSON needs ascending keys",
        );
        debug_assert!(is_canonical_key(key), "canonical JSON needs lowercase underscored keys");
        if self.previous.is_some() {
            self.out.push(',');
        }
        self.previous = Some(key.to_string());
        write_text(self.out, key);
        self.out.push(':');
    }

    /// Writes an integer under a key.
    ///
    /// The hazard this panic path names: every integer written today comes
    /// from a bounded field — counters, identifiers, `Frac` and `Fx` values
    /// the caps hold below 2^28 raw — so none can reach the carry limit. A
    /// later shape that carries an unbounded `i64`, an `ObjectiveState`
    /// target among them, could, and the intended answer then is to thread
    /// the writer's `Result` out to the caller rather than to widen this
    /// expectation. Not refactored now, because a `Result` on every field of
    /// every writer buys nothing while no field can fail.
    pub fn int(&mut self, key: &str, value: i64) -> &mut Self {
        self.key(key);
        write_int(self.out, value).expect("a state integer is inside the carry limit");
        self
    }

    pub fn bool(&mut self, key: &str, value: bool) -> &mut Self {
        self.key(key);
        self.out.push_str(if value { "true" } else { "false" });
        self
    }

    pub fn text(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        write_text(self.out, value);
        self
    }

    pub fn null(&mut self, key: &str) -> &mut Self {
        self.key(key);
        self.out.push_str("null");
        self
    }

    /// Writes an already-canonical value under a key.
    pub fn raw(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        self.out.push_str(value);
        self
    }

    /// An integer when the option holds one, and null when it does not.
    pub fn int_or_null(&mut self, key: &str, value: Option<i64>) -> &mut Self {
        match value {
            Some(number) => self.int(key, number),
            None => self.null(key),
        }
    }

    pub fn object(&mut self, key: &str) -> Obj<'_> {
        self.key(key);
        Obj::new(self.out)
    }

    pub fn list(&mut self, key: &str) -> Arr<'_> {
        self.key(key);
        Arr::new(self.out)
    }

    pub fn end(self) {
        self.out.push('}');
    }
}

/// Builds one array, in semantic order.
pub struct Arr<'a> {
    out: &'a mut String,
    empty: bool,
}

impl<'a> Arr<'a> {
    pub fn new(out: &'a mut String) -> Self {
        out.push('[');
        Arr { out, empty: true }
    }

    fn comma(&mut self) {
        if !self.empty {
            self.out.push(',');
        }
        self.empty = false;
    }

    pub fn int(&mut self, value: i64) -> &mut Self {
        self.comma();
        write_int(self.out, value).expect("a state integer is inside the carry limit");
        self
    }

    pub fn text(&mut self, value: &str) -> &mut Self {
        self.comma();
        write_text(self.out, value);
        self
    }

    pub fn raw(&mut self, value: &str) -> &mut Self {
        self.comma();
        self.out.push_str(value);
        self
    }

    pub fn object(&mut self) -> Obj<'_> {
        self.comma();
        Obj::new(self.out)
    }

    pub fn end(self) {
        self.out.push(']');
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn skip_space(&mut self) {
        while let Some(byte) = self.bytes.get(self.at) {
            match byte {
                b' ' | b'\t' | b'\n' | b'\r' => self.at += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn literal(&mut self, word: &[u8]) -> bool {
        if self.bytes[self.at..].starts_with(word) {
            self.at += word.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: u32) -> Result<Json, &'static str> {
        if depth > MAX_DEPTH {
            return Err("too_deep");
        }
        match self.peek().ok_or("unexpected_end")? {
            b'n' if self.literal(b"null") => Ok(Json::Null),
            b't' if self.literal(b"true") => Ok(Json::Bool(true)),
            b'f' if self.literal(b"false") => Ok(Json::Bool(false)),
            b'"' => self.text().map(Json::Text),
            b'[' => self.list(depth),
            b'{' => self.map(depth),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err("unexpected_byte"),
        }
    }

    fn list(&mut self, depth: u32) -> Result<Json, &'static str> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::List(items));
        }
        loop {
            self.skip_space();
            items.push(self.value(depth + 1)?);
            self.skip_space();
            match self.peek().ok_or("unexpected_end")? {
                b',' => self.at += 1,
                b']' => {
                    self.at += 1;
                    return Ok(Json::List(items));
                }
                _ => return Err("unexpected_byte"),
            }
        }
    }

    fn map(&mut self, depth: u32) -> Result<Json, &'static str> {
        self.at += 1;
        let mut pairs: Vec<(String, Json)> = Vec::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Map(pairs));
        }
        loop {
            self.skip_space();
            if self.peek() != Some(b'"') {
                return Err("unexpected_byte");
            }
            let key = self.text()?;
            if !is_canonical_key(&key) {
                return Err("bad_key");
            }
            if pairs.iter().any(|(earlier, _)| *earlier == key) {
                return Err("duplicate_key");
            }
            self.skip_space();
            if self.peek() != Some(b':') {
                return Err("unexpected_byte");
            }
            self.at += 1;
            self.skip_space();
            let value = self.value(depth + 1)?;
            pairs.push((key, value));
            self.skip_space();
            match self.peek().ok_or("unexpected_end")? {
                b',' => self.at += 1,
                b'}' => {
                    self.at += 1;
                    return Ok(Json::Map(pairs));
                }
                _ => return Err("unexpected_byte"),
            }
        }
    }

    fn number(&mut self) -> Result<Json, &'static str> {
        let start = self.at;
        let negative = self.peek() == Some(b'-');
        if negative {
            self.at += 1;
        }
        let digits = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        if self.at == digits {
            return Err("unexpected_byte");
        }
        let text = &self.bytes[digits..self.at];
        if text.len() > 1 && text[0] == b'0' {
            return Err("leading_zero");
        }
        if negative && text == b"0" {
            return Err("minus_zero");
        }
        if matches!(self.peek(), Some(b'.') | Some(b'e') | Some(b'E')) {
            return Err("float");
        }
        let whole = std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| "unexpected_byte")?
            .parse::<i64>()
            .map_err(|_| "number_too_wide")?;
        if !(-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&whole) {
            return Err("number_too_wide");
        }
        Ok(Json::Int(whole))
    }

    fn text(&mut self) -> Result<String, &'static str> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let byte = self.peek().ok_or("unexpected_end")?;
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let escape = self.peek().ok_or("unexpected_end")?;
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.escaped_character()?),
                        _ => return Err("bad_escape"),
                    }
                }
                control if control < 0x20 => return Err("raw_control"),
                plain if plain < 0x80 => {
                    self.at += 1;
                    out.push(char::from(plain));
                }
                _ => {
                    // A sequence is at most four bytes, so the decode reads a
                    // bounded window rather than the whole remaining payload.
                    // Reading the rest would make the reader quadratic in the
                    // payload's length, and a save payload runs to megabytes.
                    let end = (self.at + 4).min(self.bytes.len());
                    let window = &self.bytes[self.at..end];
                    let decodable = match std::str::from_utf8(window) {
                        Ok(text) => text,
                        // A window that cut a sequence short still decodes
                        // whatever stands whole before the cut.
                        Err(cut) => std::str::from_utf8(&window[..cut.valid_up_to()])
                            .map_err(|_| "not_utf8")?,
                    };
                    let character = decodable.chars().next().ok_or("not_utf8")?;
                    self.at += character.len_utf8();
                    out.push(character);
                }
            }
        }
    }

    /// One `\u` escape, joining a surrogate pair when the pair is present.
    fn escaped_character(&mut self) -> Result<char, &'static str> {
        let first = self.hex_quad()?;
        if (0xd800..0xdc00).contains(&first) {
            if !self.literal(b"\\u") {
                return Err("bad_escape");
            }
            let second = self.hex_quad()?;
            if !(0xdc00..0xe000).contains(&second) {
                return Err("bad_escape");
            }
            let joined = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
            return char::from_u32(joined).ok_or("bad_escape");
        }
        char::from_u32(first).ok_or("bad_escape")
    }

    fn hex_quad(&mut self) -> Result<u32, &'static str> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.peek().ok_or("unexpected_end")?;
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a') + 10,
                b'A'..=b'F' => u32::from(byte - b'A') + 10,
                _ => return Err("bad_escape"),
            };
            value = value * 16 + digit;
            self.at += 1;
        }
        Ok(value)
    }
}
