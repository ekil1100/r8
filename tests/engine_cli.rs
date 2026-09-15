use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r8"))
        .args(arguments)
        .env("PATH", "")
        .output()
        .expect("r8 must start without external programs")
}

#[test]
fn executes_a_script_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("arithmetic.js");
    std::fs::write(&path, "(1 + 2) * 3").unwrap();
    let output = run(&[path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"9\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn evaluates_arithmetic_through_the_cli() {
    let output = run(&["-e", "1 + 2 * 3"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"7\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn separates_execution_failures_from_cli_argument_errors() {
    for (source, kind) in [
        ("1 +", "SyntaxError"),
        ("(1+2", "SyntaxError"),
        ("1 2", "SyntaxError"),
        ("1 @ 2", "SyntaxError"),
        ("let x = 1; x = 2", "Unsupported"),
        ("1++2", "Unsupported"),
    ] {
        let output = run(&["-e", source]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with(kind),
            "{source}"
        );
    }
    let nested = format!("{}1{}", "(".repeat(4096), ")".repeat(4096));
    let output = run(&["-e", &nested]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("ResourceLimit"));
    for arguments in [
        vec![],
        vec!["-e"],
        vec!["script.js", "-e", "1"],
        vec!["one.js", "two.js"],
        vec!["--unknown"],
        vec!["-e", "1", "2"],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn accepts_negative_sources_and_keeps_undefined_completion_silent() {
    for source in ["", " \n\t", "/* empty */"] {
        let output = run(&["-e", source]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    let output = run(&["-e", "-2 - -3"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"1\n");
    assert!(output.stderr.is_empty());
    let output = run(&["--eval", "(1 + 2) * 3"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"9\n");
}

#[test]
fn prints_ecmascript_number_strings() {
    for (source, expected) in [
        ("0 / 0", "NaN\n"),
        ("1 / 0", "Infinity\n"),
        ("-1 / 0", "-Infinity\n"),
        ("-0", "0\n"),
        ("1e21", "1e+21\n"),
        ("1e20", "100000000000000000000\n"),
        ("1e-6", "0.000001\n"),
        ("1e-7", "1e-7\n"),
        ("5e-324", "5e-324\n"),
        ("1000000000000000128", "1000000000000000100\n"),
    ] {
        let output = run(&["-e", source]);
        assert_eq!(output.status.code(), Some(0), "{source}");
        assert_eq!(output.stdout, expected.as_bytes(), "{source}");
        assert!(output.stderr.is_empty(), "{source}");
    }
}

#[test]
fn file_and_inline_inputs_execute_stateful_scripts() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("状态 script.js");
    for (source, expected) in [
        ("let x = 3; x + 2", "5\n"),
        ("let x = 1; { const x = 3; x + 2; }", "5\n"),
        ("var x; x + 1", "NaN\n"),
        ("let x = 3;", ""),
        ("", ""),
    ] {
        std::fs::write(&path, source).unwrap();
        for arguments in [vec![path.to_str().unwrap()], vec!["-e", source]] {
            let output = run(&arguments);
            assert_eq!(output.status.code(), Some(0), "{source}");
            assert_eq!(output.stdout, expected.as_bytes(), "{source}");
            assert!(output.stderr.is_empty());
        }
    }
    let dashed = directory.path().join("-input.js");
    std::fs::write(dashed, "let x = 2; x + 3").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_r8"))
        .current_dir(directory.path())
        .args(["--", "-input.js"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"5\n");
}

#[test]
fn file_errors_and_runtime_errors_are_reported_without_results() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.js");
    for (source, kind) in [
        ("x; let x = 3;", "ReferenceError"),
        ("missing", "ReferenceError"),
        ("let x; var x;", "SyntaxError"),
        ("let x = 3; x = 4", "Unsupported"),
    ] {
        std::fs::write(&path, source).unwrap();
        let output = run(&[path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(kind));
    }
    let missing = directory.path().join("missing.js");
    for invalid in [&missing, &directory.path().to_path_buf()] {
        let output = run(&[invalid.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(invalid.to_str().unwrap()));
    }
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    let output = run(&[path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    std::fs::write(&path, vec![b' '; r8::MAX_SOURCE_BYTES + 1]).unwrap();
    let output = run(&[path.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ResourceLimit"));
}
