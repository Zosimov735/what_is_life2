//! Strict readers for the values that arrive from outside the core.
//!
//! Every command body and every stored payload is read through these helpers,
//! so one rule answers for all of them: a value of the wrong shape, outside its
//! locked range, or under a key the shape does not declare is a fault naming
//! that key. `docs/field-framework/ARCHITECTURE.md` locks canonical JSON with
//! no absent-versus-null ambiguity, so a declared field that is missing is the
//! same fault as one carrying the wrong type — [`exact_keys`] holds the other
//! half of that rule, refusing a key the shape never declared.
//!
//! The faults produced here are `validation` faults. The caller re-codes them
//! where the document names a different envelope: an import failure is
//! `import_invalid` and a stored record's failure is `save_corrupt`, both
//! keeping the detail that names what failed.

use crate::fault::{Code, Fault};
use crate::json::{is_hex, Json};

/// The value under a key, as a map. A body that is not a map at all fails
/// under the name the caller gives it.
pub fn map<'a>(value: &'a Json, key: &str) -> Result<&'a Json, Fault> {
    let found = at(value, key)?;
    if found.is_map() {
        Ok(found)
    } else {
        Err(Fault::field(key))
    }
}

/// The value under a key, whatever its shape.
pub fn at<'a>(value: &'a Json, key: &str) -> Result<&'a Json, Fault> {
    value.get(key).ok_or_else(|| Fault::field(key))
}

/// The value under a key as a map, or none where the field is declared
/// nullable. A key that is absent is missing rather than null: the shape is
/// declared, and only its value may stand as nothing.
pub fn map_or_null<'a>(value: &'a Json, key: &str) -> Result<Option<&'a Json>, Fault> {
    let found = at(value, key)?;
    match found {
        Json::Null => Ok(None),
        _ if found.is_map() => Ok(Some(found)),
        _ => Err(Fault::field(key)),
    }
}

/// An integer under a key, inside an inclusive range.
pub fn int(value: &Json, key: &str, low: i64, high: i64) -> Result<i64, Fault> {
    at(value, key)?
        .as_int()
        .filter(|number| (low..=high).contains(number))
        .ok_or_else(|| Fault::field(key))
}

/// An integer under a key, or none where the field is declared nullable.
pub fn int_or_null(value: &Json, key: &str, low: i64, high: i64) -> Result<Option<i64>, Fault> {
    match at(value, key)? {
        Json::Null => Ok(None),
        Json::Int(number) if (low..=high).contains(number) => Ok(Some(*number)),
        _ => Err(Fault::field(key)),
    }
}

/// A string under a key.
pub fn text<'a>(value: &'a Json, key: &str) -> Result<&'a str, Fault> {
    at(value, key)?.as_text().ok_or_else(|| Fault::field(key))
}

/// A fixed-width lowercase hex string under a key.
pub fn hex<'a>(value: &'a Json, key: &str, width: usize) -> Result<&'a str, Fault> {
    let found = text(value, key)?;
    if is_hex(found, width) {
        Ok(found)
    } else {
        Err(Fault::field(key))
    }
}

/// A boolean under a key.
pub fn flag(value: &Json, key: &str) -> Result<bool, Fault> {
    at(value, key)?.as_bool().ok_or_else(|| Fault::field(key))
}

/// An array under a key, held to a length.
pub fn list<'a>(value: &'a Json, key: &str, longest: usize) -> Result<&'a [Json], Fault> {
    match at(value, key)? {
        Json::List(items) if items.len() <= longest => Ok(items),
        _ => Err(Fault::field(key)),
    }
}

/// Holds a map to exactly the keys its shape declares — no key missing and no
/// key beyond them.
pub fn exact_keys(value: &Json, key: &str, declared: &[&str]) -> Result<(), Fault> {
    let Json::Map(pairs) = value else {
        return Err(Fault::field(key));
    };
    if pairs.len() != declared.len() {
        return Err(Fault::field(key));
    }
    for (name, _) in pairs {
        if !declared.contains(&name.as_str()) {
            return Err(Fault::field(name));
        }
    }
    Ok(())
}

/// A string from a closed set, returning its position in that set.
pub fn one_of(value: &Json, key: &str, closed: &[&str]) -> Result<usize, Fault> {
    let found = text(value, key)?;
    closed.iter().position(|name| *name == found).ok_or_else(|| Fault::field(key))
}

/// True when the values are strictly ascending, which every identifier-keyed
/// list of a payload is.
pub fn ascending<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

/// An ascending list of identifiers under a key.
pub fn ids(value: &Json, key: &str, longest: usize, high: i64) -> Result<Vec<u32>, Fault> {
    let items = list(value, key, longest)?;
    let mut found = Vec::with_capacity(items.len());
    for item in items {
        let number = item.as_int().filter(|number| (0..=high).contains(number));
        found.push(number.ok_or_else(|| Fault::field(key))? as u32);
    }
    if !ascending(&found) {
        return Err(Fault::field(key));
    }
    Ok(found)
}

/// Re-codes a fault under the envelope its caller answers with, keeping the
/// detail that names what failed.
pub fn recode(fault: Fault, code: Code) -> Fault {
    match fault.detail() {
        Some(detail) => Fault::detailed(code, detail.to_string()),
        None => Fault::new(code),
    }
}
