use r8::{Value, eval};

#[test]
fn declares_and_reads_bindings() {
    for (source, expected) in [
        ("let x = 3; x + 2", 5.0),
        ("const x = 3; x * 2", 6.0),
        ("var x = 3; var x = x + 2; x", 5.0),
        ("let x = 2, y = x + 3; x * y", 10.0),
    ] {
        assert_eq!(eval(source).unwrap(), Value::Number(expected), "{source}");
    }
}

#[test]
fn block_bindings_shadow_and_var_declarations_hoist() {
    for (source, expected) in [
        ("let x = 1; { let x = 2; x; } x", 1.0),
        ("let x = 1; { const x = 2; { x + 3; } }", 5.0),
        ("var x = 1; { var x = 2; } x", 2.0),
        ("{ let x = 1; } { var x = 3; } x", 3.0),
        ("{ var x = 3; } { let x = 1; } x", 3.0),
        ("1; { 2; let x; }", 2.0),
    ] {
        assert_eq!(eval(source).unwrap(), Value::Number(expected), "{source}");
    }
    assert_eq!(eval("x; { var x = 3; }").unwrap(), Value::Undefined);
    for source in [
        "x; let x;",
        "let x = x;",
        "let x = 1; { x; let x; }",
        "{ let x = 1; } x",
    ] {
        assert_eq!(
            eval(source).unwrap_err().kind,
            r8::ErrorKind::Reference,
            "{source}"
        );
    }
}

#[test]
fn unicode_identifiers_and_escapes_use_ecmascript_id_properties() {
    for (source, expected) in [
        ("let 变量 = 3; 变量 + 2", 5.0),
        (r"let \u0078 = 3; x + 2", 5.0),
        (r"let \u{10400} = 4; 𐐀 + 1", 5.0),
        ("let $x_2 = 3; $x_2 + 1", 4.0),
        ("let ℘ = 2, ゛ = 3; ℘ + ゛", 5.0),
        ("let a\u{200c} = 3; a\u{200c}", 3.0),
        ("let a\u{200d} = 4; a\u{200d}", 4.0),
        ("let é = 1, e\u{0301} = 2; é + e\u{0301}", 3.0),
    ] {
        assert_eq!(eval(source).unwrap(), Value::Number(expected), "{source}");
    }
    for source in [
        r"let \u0030 = 1",
        r"let a\u002d = 1",
        r"let \u{110000} = 1",
        r"let \uD800 = 1",
        r"let \u{} = 1",
        r"let \x61 = 1",
        r"v\u0061r x = 1",
        r"let \u0069f = 1",
        "1变量",
        "1℘",
        "let \u{200c}x = 1",
        "let 😀 = 1",
    ] {
        assert_eq!(
            eval(source).unwrap_err().kind,
            r8::ErrorKind::Syntax,
            "{source}"
        );
    }
}

#[test]
fn declarations_have_empty_completion_and_hoisted_undefined_is_numeric_nan() {
    for source in [
        "",
        ";;;",
        "{}",
        "let x;",
        "const x = 1;",
        "var x;",
        "x; var x = 3;",
        "let x; x",
        "1; undefined;",
    ] {
        assert_eq!(eval(source).unwrap(), Value::Undefined, "{source}");
    }
    for source in [
        "1; var x = 3;",
        "1; let x;",
        "1; {}",
        "1; {let x;}",
        "1;;;",
        "let x = 1; x; var y;",
    ] {
        assert_eq!(eval(source).unwrap(), Value::Number(1.0), "{source}");
    }
    for source in [
        "let x; +x",
        "let x; -x",
        "x + 1; var x = 3;",
        "var x; x * 2",
        "undefined / 2",
    ] {
        assert!(
            matches!(eval(source).unwrap(), Value::Number(n) if n.is_nan()),
            "{source}"
        );
    }
}

#[test]
fn declaration_errors_are_early_errors_even_after_unbound_reads() {
    for source in [
        "missing; let x; let x;",
        "let x; const x = 1;",
        "var x; let x;",
        "let x; var x;",
        "{ let x; { var x; } }",
        "{ { var x; } let x; }",
        "{ var x; } const x = 1;",
        "let x; { var x; }",
        "const x;",
        "const x = 1, y;",
        "let x, x;",
        "let let = 1;",
        "let if = 1;",
        "var const = 1;",
        "var true = 1;",
        "let x = ;",
        "var x,;",
        "let = 1",
        "{ let x = 1;",
        "let x = 1; }",
        r"let x; let \u0078;",
    ] {
        let error = r8::Script::parse(source).unwrap_err();
        // Assignment to the sloppy identifier `let` belongs to S02.
        let expected = if source == "let = 1" {
            r8::ErrorKind::Unsupported
        } else {
            r8::ErrorKind::Syntax
        };
        assert_eq!(error.kind, expected, "{source}: {error}");
        assert!(source.is_char_boundary(error.offset), "{source}");
    }
}

#[test]
fn automatic_semicolons_do_not_split_expression_continuations() {
    for (source, expected) in [
        ("let x = 3\nx + 2", 5.0),
        ("let\nx = 3\nx + 2", 5.0),
        ("let x\nlet y = 2\ny", 2.0),
        ("let x = 1\n+2\nx", 3.0),
        ("let x = 1/*\n*/let y = 2\nx+y", 3.0),
        ("let x = 3\u{2028}x + 2", 5.0),
        ("let x = 3\u{2029}x + 2", 5.0),
        ("let x = 3\r\nx + 2", 5.0),
        ("let x = 1; { let y = 2\ny }", 2.0),
        ("1\n2", 2.0),
        ("var let = 2; let + 1", 3.0),
        ("var let = 2; let\n1", 1.0),
        ("let yield = 2, await = 3; yield + await", 5.0),
    ] {
        assert_eq!(eval(source).unwrap(), Value::Number(expected), "{source}");
    }
    for source in [
        "let x = 1 let y = 2",
        "let x = 1/* no newline */let y = 2",
        "1 2",
        "(1\n2)",
    ] {
        assert_eq!(
            eval(source).unwrap_err().kind,
            r8::ErrorKind::Syntax,
            "{source}"
        );
    }
    for source in [
        "let x = 1\n(2)",
        "let x = 1\nx = 2",
        "let x = 1; x++",
        "let x = (1, 2)",
    ] {
        assert_eq!(
            eval(source).unwrap_err().kind,
            r8::ErrorKind::Unsupported,
            "{source}"
        );
    }
}

#[test]
fn contexts_share_globals_but_fresh_runs_do_not() {
    let script = r8::Script::parse("let x = 3; x + 2").unwrap();
    assert_eq!(script.run().unwrap(), Value::Number(5.0));
    assert_eq!(script.run().unwrap(), Value::Number(5.0));
    let mut context = r8::Context::default();
    assert_eq!(context.run(&script).unwrap(), Value::Number(5.0));
    assert_eq!(context.eval("x + 4").unwrap(), Value::Number(7.0));
    assert_eq!(
        context.run(&script).unwrap_err().kind,
        r8::ErrorKind::Syntax
    );
    context.eval("var y = 2;").unwrap();
    context.eval("var y;").unwrap();
    assert_eq!(context.eval("y").unwrap(), Value::Number(2.0));
    assert_eq!(
        context.eval("var y = y + x; y").unwrap(),
        Value::Number(5.0)
    );
    assert_eq!(
        context.eval("let y;").unwrap_err().kind,
        r8::ErrorKind::Syntax
    );
    assert_eq!(
        context.eval("var x;").unwrap_err().kind,
        r8::ErrorKind::Syntax
    );
    assert_eq!(eval("x").unwrap_err().kind, r8::ErrorKind::Reference);
}

#[test]
fn failures_preserve_global_instantiation_and_unwind_block_scopes() {
    let mut context = r8::Context::default();
    context.eval("let existing = 3;").unwrap();
    assert_eq!(
        context
            .eval("var newName = 1; let existing;")
            .unwrap_err()
            .kind,
        r8::ErrorKind::Syntax
    );
    assert_eq!(
        context.eval("newName").unwrap_err().kind,
        r8::ErrorKind::Reference
    );
    assert_eq!(
        context
            .eval("let partial = missing; let later;")
            .unwrap_err()
            .kind,
        r8::ErrorKind::Reference
    );
    for name in ["partial", "later"] {
        assert!(
            context
                .eval(name)
                .unwrap_err()
                .message
                .contains("before initialization")
        );
    }
    assert_eq!(
        context.eval("let partial = 1").unwrap_err().kind,
        r8::ErrorKind::Syntax
    );
    assert_eq!(
        context
            .eval("{ let existing = 9; missing; }")
            .unwrap_err()
            .kind,
        r8::ErrorKind::Reference
    );
    assert_eq!(context.eval("existing").unwrap(), Value::Number(3.0));
    assert_eq!(
        context
            .eval("var newName = 3; newName = 4")
            .unwrap_err()
            .kind,
        r8::ErrorKind::Unsupported
    );
    assert_eq!(
        context.eval("newName").unwrap_err().kind,
        r8::ErrorKind::Reference
    );
}

#[test]
fn global_value_properties_are_read_only_but_can_be_shadowed_in_blocks() {
    assert_eq!(eval("undefined").unwrap(), Value::Undefined);
    assert_eq!(
        eval("var undefined = 3; undefined").unwrap(),
        Value::Undefined
    );
    assert_eq!(
        eval("var Infinity = 3; Infinity").unwrap(),
        Value::Number(f64::INFINITY)
    );
    assert!(matches!(eval("var NaN = 3; NaN").unwrap(), Value::Number(n) if n.is_nan()));
    for name in ["undefined", "NaN", "Infinity"] {
        let source = format!("let {name} = 3;");
        let script = r8::Script::parse(&source).unwrap();
        assert_eq!(script.run().unwrap_err().kind, r8::ErrorKind::Syntax);
        assert_eq!(
            eval(&format!("{{ let {name} = 3; {name}; }}")).unwrap(),
            Value::Number(3.0)
        );
    }
    assert_eq!(eval("Math").unwrap_err().kind, r8::ErrorKind::Unsupported);
    assert_eq!(
        eval("var Math; Math").unwrap_err().kind,
        r8::ErrorKind::Unsupported
    );
    assert_eq!(eval("let Math = 3; Math").unwrap(), Value::Number(3.0));
    assert_eq!(
        eval("notDefined").unwrap_err().kind,
        r8::ErrorKind::Reference
    );
}

#[test]
fn block_depth_is_bounded_and_flat_statement_lists_are_not_recursive() {
    let source = format!("{}1;{}", "{".repeat(8192), "}".repeat(8192));
    assert_eq!(
        eval(&source).unwrap_err().kind,
        r8::ErrorKind::ResourceLimit
    );
    let source = format!("{}1;{}", "{".repeat(64), "}".repeat(64));
    assert_eq!(eval(&source).unwrap(), Value::Number(1.0));
    let source = format!("{}2", "1;".repeat(10_000));
    assert_eq!(eval(&source).unwrap(), Value::Number(2.0));
}

#[test]
fn future_syntax_is_unsupported_instead_of_a_false_syntax_error() {
    for source in [
        "let point = {x: 1};",
        "const point = {};",
        "async function f() {}",
        "let f = async x => x;",
        "{ using resource = null; }",
        "let [x] = [1];",
        "const x = /x/;",
        "1, 2",
        "'use strict'; let x = 1;",
    ] {
        assert_eq!(
            eval(source).unwrap_err().kind,
            r8::ErrorKind::Unsupported,
            "{source}"
        );
    }
}

#[test]
fn generated_statement_fragments_never_panic() {
    let fragments = [
        "let ", "const ", "var ", "x", "y", "0", "1", ";", "{", "}", "=", ",", "+", "-", "(", ")",
        "\n", "/*\n*/", "变量", r"\u0078", r"\u{", " ",
    ];
    let mut state = 0x5248_0031_u64;
    for length in 0..32 {
        for _ in 0..64 {
            let mut source = String::new();
            for _ in 0..length {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                source.push_str(fragments[(state >> 32) as usize % fragments.len()]);
            }
            if let Err(error) = eval(&source) {
                assert!(source.is_char_boundary(error.offset), "{source:?}: {error}");
            }
        }
    }
}
