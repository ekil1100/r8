use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

fn run_path(path: &Path, options: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_r8-eval"))
        .arg(path)
        .args(options)
        .env("PATH", "")
        .output()
        .expect("r8 test CLI must start without external programs")
}

fn run_files(files: &[(&str, &str)], options: &[&str]) -> Output {
    let directory = tempfile::tempdir().unwrap();
    for (name, source) in files {
        let path = directory.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    run_path(directory.path(), options)
}

fn report(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid report: {error}; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn executes_raw_cases_and_preserves_real_error_phases() {
    let nested = format!(
        "/*---\nflags: [raw]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n{}1{}",
        "(".repeat(256),
        ")".repeat(256)
    );
    let output = run_files(
        &[
            ("a-complete.js", "/*---\nflags: [raw]\n---*/\n1 + 2 * 3;"),
            (
                "b-parse.js",
                "/*---\nflags: [raw]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n1 +",
            ),
            ("c-unexpected.js", "/*---\nflags: [raw]\n---*/\n1)"),
            (
                "d-no-error.js",
                "/*---\nflags: [raw]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n1 + 2",
            ),
            (
                "e-wrong-phase.js",
                "/*---\nflags: [raw]\nnegative:\n  phase: runtime\n  type: SyntaxError\n---*/\n1 +",
            ),
            ("f-unsupported.js", "/*---\nflags: [raw]\n---*/\nlet x = 1;"),
            (
                "g-unsupported-negative.js",
                "/*---\nflags: [raw]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\nlet x = 1;",
            ),
            ("h-limited.js", &nested),
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["summary"]["total"], 8);
    assert_eq!(report["summary"]["pass"], 2);
    assert_eq!(report["summary"]["fail"], 3);
    assert_eq!(report["summary"]["skip"], 2);
    assert_eq!(report["summary"]["harness-error"], 1);
    assert_eq!(report["results"][0]["actual"]["kind"], "completed");
    assert_eq!(report["results"][1]["actual"]["name"], "SyntaxError");
    assert_eq!(report["results"][4]["actual"]["phase"], "parse");
    assert!(
        report["results"][7]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("ResourceLimit")
    );
}

#[test]
fn parse_negatives_do_not_execute_helpers_or_accept_harness_failures() {
    let directory = tempfile::tempdir().unwrap();
    let harness = directory.path().join("harness");
    fs::create_dir(&harness).unwrap();
    fs::write(harness.join("assert.js"), "1 +").unwrap();
    fs::write(harness.join("sta.js"), "").unwrap();
    let output = run_files(
        &[
            (
                "a-parse.js",
                "/*---\nflags: [noStrict]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n1 +",
            ),
            (
                "b-no-error.js",
                "/*---\nflags: [noStrict]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n1 + 2",
            ),
            (
                "c-harness-error.js",
                "/*---\nflags: [noStrict]\nnegative:\n  phase: runtime\n  type: SyntaxError\n---*/\n1 + 2",
            ),
            (
                "d-missing-include.js",
                "/*---\nflags: [noStrict]\nincludes: [missing.js]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n1 +",
            ),
        ],
        &["--harness", harness.to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["summary"]["pass"], 1);
    assert_eq!(report["summary"]["fail"], 1);
    assert_eq!(report["summary"]["harness-error"], 2);
    assert_eq!(report["results"][1]["actual"]["kind"], "completed");
    assert!(
        report["results"][2]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("assert.js")
    );
    assert!(
        report["results"][3]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("missing.js")
    );
}

#[test]
fn r8_suite_uses_the_native_engine_and_keeps_pending_capabilities_visible() {
    let output = run_path(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("evals/cases/s00"),
        &[],
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = report(&output);
    assert_eq!(report["engine"], "r8");
    assert_eq!(report["files"], 22);
    assert_eq!(report["summary"]["total"], 44);
    assert_eq!(report["summary"]["pass"], 5);
    assert_eq!(report["summary"]["skip"], 39);
    let parsed_negatives = [
        "invalid-token.js",
        "missing-operand.js",
        "trailing-number.js",
        "unclosed-parenthesis.js",
        "unexpected-parenthesis.js",
    ];
    for result in report["results"].as_array().unwrap() {
        if result["variant"] == "non-strict"
            && parsed_negatives.contains(&result["id"].as_str().unwrap())
        {
            assert_eq!(result["status"], "pass");
            assert_eq!(result["actual"]["kind"], "error");
            assert_eq!(result["actual"]["phase"], "parse");
            assert_eq!(result["actual"]["name"], "SyntaxError");
        } else {
            assert_eq!(result["status"], "skip");
            assert_eq!(result["actual"]["kind"], "unsupported");
            assert!(result["diagnostic"].as_str().unwrap().contains("r8"));
        }
    }
}

#[test]
fn unsupported_capabilities_never_satisfy_positive_or_negative_tests() {
    let output = run_files(
        &[
            ("a-positive.js", "assert.sameValue(1 + 2, 3);"),
            (
                "b-parse.js",
                "/*---\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\nlet x = 1;",
            ),
            (
                "c-runtime.js",
                "/*---\nnegative:\n  phase: runtime\n  type: TypeError\n---*/\nthrow new TypeError();",
            ),
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["summary"]["pass"], 0);
    assert_eq!(report["summary"]["skip"], 6);
    assert_eq!(report["results"][2]["negative"]["type"], "SyntaxError");
    assert_eq!(report["results"][2]["actual"]["kind"], "unsupported");
}

#[test]
fn expands_execution_modes_and_keeps_unsupported_requirements_in_the_denominator() {
    let output = run_files(
        &[
            ("a-default.js", "assert(true);"),
            (
                "b-strict.js",
                "/*---\nflags: [onlyStrict]\n---*/\nassert(true);",
            ),
            (
                "c-non-strict.js",
                "/*---\nflags: [noStrict]\n---*/\nassert(true);",
            ),
            ("d-raw.js", "/*---\nflags: [raw]\n---*/\n1;"),
            (
                "e-generated.js",
                "/*---\nflags: [generated, non-deterministic]\n---*/\nassert(true);",
            ),
            ("f-async.js", "/*---\nflags: [async]\n---*/\n"),
            (
                "g-module.js",
                "/*---\nflags: [module]\nnegative:\n  phase: resolution\n  type: SyntaxError\n---*/\nimport './missing_FIXTURE.js';",
            ),
            ("h-blocking.js", "/*---\nflags: [CanBlockIsTrue]\n---*/\n"),
            ("i-unknown.js", "/*---\nflags: [futureFlag]\n---*/\n"),
            (
                "unused_FIXTURE.js",
                "throw new Error('Not a standalone test');",
            ),
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["files"], 9);
    assert_eq!(report["summary"]["total"], 14);
    assert_eq!(report["summary"]["pass"], 1);
    assert_eq!(report["summary"]["skip"], 13);
    assert_eq!(report["results"][0]["variant"], "non-strict");
    assert_eq!(report["results"][1]["variant"], "strict");
    assert_eq!(report["results"][4]["variant"], "raw");
    for result in &report["results"].as_array().unwrap()[7..] {
        assert!(result.get("actual").is_none());
        assert!(result["diagnostic"].is_string());
    }
}

#[test]
fn harness_files_are_checked_without_treating_preparation_as_execution() {
    let directory = tempfile::tempdir().unwrap();
    let harness = directory.path().join("harness");
    let tests = directory.path().join("tests");
    fs::create_dir(&harness).unwrap();
    fs::create_dir(&tests).unwrap();
    for name in ["assert.js", "sta.js"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("evals/harness")
                .join(name),
            harness.join(name),
        )
        .unwrap();
    }
    fs::write(
        harness.join("helper.js"),
        "throw new SyntaxError('Must not be claimed as executed');",
    )
    .unwrap();
    fs::write(
        directory.path().join("outside.js"),
        "throw new SyntaxError();",
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(directory.path().join("outside.js"), harness.join("link.js"))
        .unwrap();
    for (name, source) in [
        (
            "a-prepared.js",
            "/*---\ndescription: Prepare a helper without claiming execution.\nesid: sec-additive-operators\nfeatures: [Symbol]\nflags: [noStrict]\nincludes: [helper.js]\nnegative:\n  phase: runtime\n  type: SyntaxError\n---*/\n",
        ),
        (
            "b-missing.js",
            "/*---\nflags: [noStrict]\nincludes: [missing.js]\n---*/\n",
        ),
        (
            "c-escape.js",
            "/*---\nflags: [noStrict]\nincludes: [../outside.js]\n---*/\n",
        ),
        (
            "d-link.js",
            "/*---\nflags: [noStrict]\nincludes: [link.js]\n---*/\n",
        ),
    ] {
        fs::write(tests.join(name), source).unwrap();
    }
    let output = run_path(&tests, &["--harness", harness.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["summary"]["pass"], 0);
    assert_eq!(report["summary"]["skip"], 1);
    assert_eq!(report["summary"]["harness-error"], 3);
    assert_eq!(report["harness"], harness.to_str().unwrap());
    assert_eq!(report["results"][0]["esid"], "sec-additive-operators");
    assert_eq!(report["results"][0]["features"][0], "Symbol");
    assert_eq!(report["results"][0]["actual"]["kind"], "unsupported");
    #[cfg(unix)]
    assert!(
        report["results"][3]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("outside harness")
    );
}

#[test]
fn raw_tests_do_not_require_a_harness_directory() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing-harness");
    let output = run_files(
        &[(
            "raw.js",
            "/*---\nflags: [raw]\nincludes: [missing.js]\n---*/\n1;",
        )],
        &["--harness", missing.to_str().unwrap()],
    );
    assert_eq!(output.status.code(), Some(0));
    let report = report(&output);
    assert_eq!(report["summary"]["pass"], 1);
    assert_eq!(report["summary"]["skip"], 0);
    assert_eq!(report["summary"]["harness-error"], 0);
}

#[test]
fn invalid_metadata_and_empty_or_legacy_suites_are_configuration_errors() {
    for source in [
        "/*---\nflags: [onlyStrict, noStrict]\n---*/\n",
        "/*---\nnegative: null\n---*/\n",
        "/*---\nflags: onlyStrict\n---*/\n",
        "/*---\nflags: [raw, module]\n---*/\n",
        "/*---\nnegative:\n  phase: parse\n  type: ''\n---*/\n",
        "/*---\nnegative:\n  phase: typo\n  type: SyntaxError\n---*/\n",
        "/*---\nnegative:\n  phase: parse\n---*/\n",
        "/*---\nunknown: true\n---*/\n",
        "/*---\nflags: [raw]\n",
    ] {
        let output = run_files(&[("invalid.js", source)], &[]);
        assert_eq!(output.status.code(), Some(2), "source: {source}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    for files in [
        vec![],
        vec![("suite.json", r#"{"name":"legacy","cases":[]}"#)],
        vec![("only_FIXTURE.js", "0;")],
    ] {
        let output = run_files(&files, &[]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn rejects_external_engine_commands_and_invalid_cli_arguments() {
    for options in [
        &["--", "external-engine"][..],
        &["--engine", "other"],
        &["--timeout-ms", "0"],
    ] {
        let output = run_files(&[("case.js", "assert(true);")], options);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn accepts_a_single_file_without_mistaking_string_contents_for_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("single.js");
    let source = "\u{feff}// Header comment\r\nconst marker = '/*--- flags: [async] ---*/';\r\nassert.sameValue(marker[0], '/');\r\n";
    fs::write(&path, source).unwrap();
    let output = run_path(&path, &[]);
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["files"], 1);
    assert_eq!(report["summary"]["skip"], 2);
    assert_eq!(report["results"][0]["id"], "single.js");
    assert_eq!(report["results"][0]["source"], source);
    assert_eq!(report["results"][0]["actual"]["kind"], "unsupported");
}

#[cfg(unix)]
#[test]
fn distinct_unix_paths_keep_distinct_test_ids() {
    let output = run_files(
        &[
            (r"nested\case.js", "assert(true);"),
            ("nested/case.js", "assert(true);"),
        ],
        &[],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    let ids: std::collections::BTreeSet<_> = report["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| result["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        std::collections::BTreeSet::from([r"nested\case.js", "nested/case.js"])
    );
}
