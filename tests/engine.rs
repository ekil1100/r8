use r8::{ErrorKind, Script, Value, eval};

fn number(source: &str) -> f64 {
    match eval(source).unwrap_or_else(|error| panic!("{source:?}: {error}")) {
        Value::Number(number) => number,
        value => panic!("Expected a Number for {source:?}, got {value:?}"),
    }
}

#[test]
fn evaluates_decimal_fractions_and_exponents() {
    for (source, expected) in [
        (".5 + 1.", 1.5),
        ("1.25e2 - .5E+1", 120.0),
        ("1.e-3 * 1000", 1.0),
        ("12.5 % 2", 0.5),
        ("0.1 + 0.2", 0.30000000000000004),
        ("9007199254740993", 9007199254740992.0),
    ] {
        assert_eq!(number(source), expected, "{source}");
    }
}

#[test]
fn respects_operator_precedence_grouping_and_left_associativity() {
    for (source, expected) in [
        ("1 + 2 * 3", 7.0),
        ("(1 + 2) * 3", 9.0),
        ("((2 + 3) * (4 - 1)) / 5", 3.0),
        ("8 / 4 / 2", 1.0),
        ("10 - 3 - 2", 5.0),
        ("20 % 6 * 2", 4.0),
        ("-2 * -3 + +4", 10.0),
        ("1 + +2", 3.0),
        ("1 - -2", 3.0),
        ("1 + 2;", 3.0),
        ("1\n+2", 3.0),
    ] {
        assert_eq!(number(source), expected, "{source}");
    }
    assert_eq!(eval("").unwrap(), Value::Undefined);
    let script = Script::parse(&String::from("(1 + 2) * 3")).unwrap();
    assert_eq!(script.run().unwrap(), Value::Number(9.0));
    assert_eq!(script.run().unwrap(), Value::Number(9.0));
}

#[test]
fn preserves_ieee754_numbers_including_signed_zero() {
    for (source, expected) in [
        ("0", 0.0_f64),
        ("-0", -0.0),
        ("+(-0)", -0.0),
        ("-(-0)", 0.0),
        ("0 + -0", 0.0),
        ("-0 + -0", -0.0),
        ("-0 - 0", -0.0),
        ("-0 - -0", 0.0),
        ("0 * -1", -0.0),
        ("-0 * -1", 0.0),
        ("0 / -7", -0.0),
        ("-6 % 3", -0.0),
        ("-0 % 3", -0.0),
        ("-5.5 % 2", -1.5),
        ("5.5 % -2", 1.5),
        ("2 % (1 / 0)", 2.0),
        ("1 / 0", f64::INFINITY),
        ("1 / -0", f64::NEG_INFINITY),
        ("1e309", f64::INFINITY),
        ("1e-999", 0.0),
        ("-1e-999", -0.0),
        ("5e-324", f64::from_bits(1)),
        ("1.7976931348623157e308", f64::MAX),
    ] {
        assert_eq!(number(source).to_bits(), expected.to_bits(), "{source}");
    }
    for source in [
        "0 / 0",
        "1 % 0",
        "(1/0) - (1/0)",
        "(1/0) * 0",
        "(1/0) / (1/0)",
        "-(0/0)",
        "(1/0) % 2",
    ] {
        assert!(number(source).is_nan(), "{source}");
    }
}

#[test]
fn reports_syntax_errors_with_byte_offsets_and_consumes_all_input() {
    for (source, offset) in [
        ("1 +", 3),
        ("(1+2", 4),
        ("1)", 1),
        ("1 2", 2),
        ("1 @ 2", 2),
        ("1 +;", 3),
        ("()", 1),
        ("1 * *2", 4),
        ("1e", 2),
        ("1e+", 3),
        ("1e-", 3),
        ("1.2.3", 3),
        ("1foo", 1),
        ("1e2x", 3),
        ("/*", 0),
        ("1\0", 1),
        ("(1\n2)", 3),
        ("\u{feff}1 +", 6),
    ] {
        let error = eval(source).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Syntax, "{source}: {error}");
        assert_eq!(error.offset, offset, "{source}: {error}");
    }
}

#[test]
fn short_generated_inputs_never_panic() {
    let alphabet = [
        '0', '1', '.', 'e', '+', '-', '*', '/', '%', '(', ')', ';', ' ', '\n', '@', 'x',
        '\u{00a0}', '中',
    ];
    let mut state = 0x5248_0030_u64;
    let mut completed = 0;
    let mut rejected = 0;
    for length in 0..64 {
        for _ in 0..64 {
            let source: String = (0..length)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    alphabet[(state >> 32) as usize % alphabet.len()]
                })
                .collect();
            match eval(&source) {
                Ok(_) => completed += 1,
                Err(error) => {
                    rejected += 1;
                    assert!(source.is_char_boundary(error.offset));
                }
            }
        }
    }
    assert!(completed > 0 && rejected > 0);
}

#[test]
fn bounds_source_and_parser_recursion_without_limiting_flat_expressions() {
    let too_large = " ".repeat(1024 * 1024 + 1);
    assert_eq!(eval(&too_large).unwrap_err().kind, ErrorKind::ResourceLimit);
    assert_eq!(eval(&" ".repeat(1024 * 1024)).unwrap(), Value::Undefined);
    for source in [
        format!("{}1{}", "(".repeat(8192), ")".repeat(8192)),
        format!("{}1", "- ".repeat(8192)),
    ] {
        let error = eval(&source).unwrap_err();
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
        assert!(error.offset < source.len());
    }
    assert_eq!(
        number(&format!("{}1{}", "(".repeat(64), ")".repeat(64))),
        1.0
    );
    assert_eq!(number(&format!("{}1", "1+".repeat(10_000))), 10_001.0);
}

#[test]
fn handles_ecmascript_whitespace_and_comments_without_joining_tokens() {
    let whitespace = "\t\u{000b}\u{000c} \u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{202f}\u{205f}\u{3000}\u{feff}\n\r\u{2028}\u{2029}";
    assert_eq!(eval(whitespace).unwrap(), Value::Undefined);
    assert_eq!(
        number(&format!(
            "{whitespace}1{whitespace}+{whitespace}2;{whitespace}"
        )),
        3.0
    );
    assert_eq!(number("/* header */ 1 +/* gap */+2; // end"), 3.0);
    assert_eq!(number("// header\u{2028}2 * (3 /* 中 */ + 1)"), 8.0);
    assert_eq!(eval("/* empty */ // end").unwrap(), Value::Undefined);
    assert_eq!(eval("1/* gap */2").unwrap_err().kind, ErrorKind::Syntax);
    assert_eq!(eval("1/*\n*/2").unwrap(), Value::Number(2.0));
    assert_eq!(eval("/* unterminated").unwrap_err().kind, ErrorKind::Syntax);
    for character in ['\u{0085}', '\u{180e}', '\u{200b}', '\0'] {
        assert!(eval(&format!("1{character}+2")).is_err());
    }
}

#[test]
fn reports_unsupported_syntax_without_executing_a_valid_prefix() {
    for source in [
        "let x = 1; x = 2",
        "1++2",
        "1--2",
        "1**2",
        "1+=2",
        "1-=2",
        "1*=2",
        "1/=2",
        "1%=2",
        "true",
        "'2' + 1",
        "1(2)",
        "[1]",
        "/x/",
        "0x10",
        "0b10",
        "0o10",
        "00",
        "1_000",
        "1n",
    ] {
        let error = eval(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::Unsupported, "{source}: {error}");
    }
}
