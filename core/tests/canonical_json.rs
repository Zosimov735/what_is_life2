//! The canonicalizer, which every serialized payload goes through.
//!
//! The rules under test are the ones
//! `docs/field-framework/ARCHITECTURE.md` locks: compact output, keys in
//! ascending UTF-8 byte order with no duplicates, integers only with no minus
//! zero and no value wider than the carry limit, the short escape set, and
//! fixed-width lowercase hex for anything wider than 53 bits.

use field_game_core::json::{
    canonicalize, hex16, hex32, hex_bytes, is_canonical_key, is_hex, parse, text_value, Json, Obj,
    MAX_SAFE_INT,
};

#[test]
fn keys_come_back_in_ascending_byte_order() {
    assert_eq!(
        canonicalize("{\"view\":1,\"anchors\":2,\"rng\":3,\"run_id\":4}").unwrap(),
        "{\"anchors\":2,\"rng\":3,\"run_id\":4,\"view\":1}"
    );
    // Ascending byte order, not by length: a key that is another's prefix
    // comes first.
    assert_eq!(
        canonicalize("{\"steps\":1,\"start_step\":2,\"step\":3}").unwrap(),
        "{\"start_step\":2,\"step\":3,\"steps\":1}"
    );
}

#[test]
fn keys_are_lowercase_words_joined_by_underscores() {
    for shaped in ["a", "q", "run_id", "steer_x", "t_us", "payload_sha256", "sha256", "a1_b2_c3"] {
        assert!(is_canonical_key(shaped), "{shaped} is a key");
        assert!(canonicalize(&format!("{{\"{shaped}\":1}}")).is_ok());
    }
    for refused in ["", "Step", "runId", "run-id", "_run", "run_", "run__id", "1st", "a b", "ID"] {
        assert!(!is_canonical_key(refused), "{refused} is not a key");
        assert_eq!(
            parse(&format!("{{\"{refused}\":1}}")),
            Err("bad_key"),
            "a key outside the naming rule is refused: {refused}"
        );
    }
    // Nested objects are held to the same rule.
    assert_eq!(parse("{\"a\":{\"B\":1}}"), Err("bad_key"));
    assert_eq!(parse("{\"a\":[{\"B\":1}]}"), Err("bad_key"));
}

#[test]
fn output_is_compact_and_nesting_is_ordered_throughout() {
    assert_eq!(
        canonicalize("{ \"b\" : [ 1 , { \"z\" : 0 , \"a\" : 1 } ] , \"a\" : null }\n").unwrap(),
        "{\"a\":null,\"b\":[1,{\"a\":1,\"z\":0}]}"
    );
}

#[test]
fn arrays_keep_the_order_they_were_written_in() {
    assert_eq!(canonicalize("[3,1,2]").unwrap(), "[3,1,2]");
}

#[test]
fn integers_are_the_only_numbers() {
    assert_eq!(parse("0").unwrap(), Json::Int(0));
    assert_eq!(parse("-7").unwrap(), Json::Int(-7));
    assert_eq!(canonicalize("{\"a\":-1,\"b\":0}").unwrap(), "{\"a\":-1,\"b\":0}");

    for refused in ["1.0", "0.5", "1e3", "1E3", "-2.5", "[1.5]", "{\"a\":1.0}"] {
        assert_eq!(parse(refused), Err("float"), "a float is refused: {refused}");
    }
    assert_eq!(parse("01"), Err("leading_zero"));
    assert_eq!(parse("-01"), Err("leading_zero"));
    assert_eq!(parse("NaN"), Err("unexpected_byte"));
}

#[test]
fn minus_zero_is_refused() {
    assert_eq!(parse("-0"), Err("minus_zero"));
    assert_eq!(parse("{\"a\":-0}"), Err("minus_zero"));
    assert_eq!(parse("0").unwrap(), Json::Int(0));
}

#[test]
fn the_carry_limit_is_the_hex_boundary_at_two_to_the_fifty_three() {
    assert_eq!(MAX_SAFE_INT, 9_007_199_254_740_991);
    assert_eq!(canonicalize("9007199254740991").unwrap(), "9007199254740991");
    assert_eq!(canonicalize("-9007199254740991").unwrap(), "-9007199254740991");
    assert_eq!(parse("9007199254740992"), Err("number_too_wide"));
    assert_eq!(parse("-9007199254740992"), Err("number_too_wide"));

    // What will not fit is written as fixed-width lowercase hex instead.
    assert_eq!(hex16(u64::MAX), "ffffffffffffffff");
    assert_eq!(hex16(1), "0000000000000001");
    assert_eq!(hex32(0), "00000000000000000000000000000000");
    assert_eq!(hex32(u128::from(u64::MAX) + 1), "00000000000000010000000000000000");
    assert!(is_hex(&hex16(9_007_199_254_740_992), 16));
    assert!(!is_hex("0123456789ABCDEF", 16), "hex strings are lowercase");
    assert!(!is_hex("0123", 16), "hex strings are fixed width");
}

#[test]
fn duplicate_keys_are_refused() {
    assert_eq!(parse("{\"a\":1,\"a\":2}"), Err("duplicate_key"));
}

#[test]
fn strings_carry_only_the_locked_escapes() {
    assert_eq!(text_value("still"), "\"still\"");
    assert_eq!(text_value("a\"b\\c"), "\"a\\\"b\\\\c\"");
    assert_eq!(text_value("a\nb\u{1}"), "\"a\\nb\\u0001\"");
    assert_eq!(text_value("\u{8}\u{c}\r\t"), "\"\\b\\f\\r\\t\"");
    // Everything else stays raw UTF-8, and a read escape comes back raw.
    assert_eq!(canonicalize("\"\\u00e9\"").unwrap(), "\"\u{e9}\"");
    assert_eq!(canonicalize("\"\\ud83c\\udf00\"").unwrap(), "\"\u{1f300}\"");
    assert_eq!(parse("\"a\nb\""), Err("raw_control"));
}

#[test]
fn raw_utf8_reads_back_whole_at_every_sequence_width_and_at_the_payload_edge() {
    // Two, three, and four byte sequences, raw, mixed with ASCII either side.
    for text in ["\u{e9}", "a\u{e9}b", "\u{20ac}", "\u{1f300}", "a\u{1f300}", "\u{1f300}z"] {
        let written = text_value(text);
        assert_eq!(canonicalize(&written).unwrap(), written, "{text} reads back whole");
        assert_eq!(parse(&written).unwrap(), Json::Text(text.to_string()));
    }
    // A sequence that ends the payload, so the decode window is cut by the
    // buffer rather than by the string.
    assert_eq!(parse("\"\u{1f300}\"").unwrap(), Json::Text("\u{1f300}".to_string()));
    assert_eq!(
        canonicalize("{\"a\":\"\u{1f300}\"}").unwrap(),
        "{\"a\":\"\u{1f300}\"}"
    );

    // A cut sequence never reaches the reader: its input is text, so every
    // sequence in it stands whole. What a payload carrying cut bytes turns into
    // before it arrives is the replacement character, and that reads back whole
    // like any other.
    let cut = String::from_utf8_lossy(&[b'"', 0xf0, 0x9f, b'"']).to_string();
    assert_eq!(parse(&cut).unwrap(), Json::Text("\u{fffd}".to_string()));
    let written = text_value("\u{fffd}");
    assert_eq!(canonicalize(&written).unwrap(), written);
}

#[test]
fn hashes_are_sixty_four_lowercase_hex_characters() {
    let digest = field_game_core::sha256::digest(&[]);
    let written = hex_bytes(&digest);
    assert_eq!(
        written,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "the digest of no bytes is the published one"
    );
    assert!(is_hex(&written, 64));
    assert_eq!(
        hex_bytes(&field_game_core::sha256::digest(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // A message that crosses the padding boundary and one that spans blocks.
    assert_eq!(
        hex_bytes(&field_game_core::sha256::digest(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn a_written_object_refuses_nothing_and_reads_back_identically() {
    let mut out = String::new();
    let mut object = Obj::new(&mut out);
    object.int("a", -1);
    object.bool("b", true);
    object.null("c");
    object.text("d", "one");
    let mut list = object.list("e");
    list.int(1);
    list.text("two");
    list.end();
    object.end();

    assert_eq!(out, "{\"a\":-1,\"b\":true,\"c\":null,\"d\":\"one\",\"e\":[1,\"two\"]}");
    assert_eq!(canonicalize(&out).unwrap(), out, "what the writer writes is already canonical");
}

#[test]
fn trailing_bytes_and_unclosed_values_are_refused() {
    assert_eq!(parse("{} {}"), Err("trailing_bytes"));
    assert_eq!(parse("{\"a\":1"), Err("unexpected_end"));
    assert_eq!(parse(""), Err("unexpected_end"));
}
