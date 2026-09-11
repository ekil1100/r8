use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

use crate::protocol::{Case, Negative, Phase, Request, Script, Variant};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Metadata {
    #[serde(deserialize_with = "deserialize_negative")]
    negative: Option<Negative>,
    flags: Vec<String>,
    includes: Vec<String>,
    features: Vec<String>,
    locale: Vec<String>,
    esid: Option<String>,
    #[serde(flatten)]
    annotations: BTreeMap<String, serde::de::IgnoredAny>,
}

fn deserialize_negative<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Negative>, D::Error> {
    Negative::deserialize(deserializer).map(Some)
}

impl Metadata {
    fn variants(&self) -> Vec<Variant> {
        if self.flags.iter().any(|flag| flag == "raw") {
            vec![Variant::Raw]
        } else if self.flags.iter().any(|flag| flag == "module") {
            vec![Variant::Module]
        } else if self.flags.iter().any(|flag| flag == "onlyStrict") {
            vec![Variant::Strict]
        } else if self.flags.iter().any(|flag| flag == "noStrict") {
            vec![Variant::NonStrict]
        } else {
            vec![Variant::NonStrict, Variant::Strict]
        }
    }
}

pub struct Suite {
    pub files: usize,
    pub cases: Vec<Case>,
}

pub fn load(path: &Path) -> Result<Suite, String> {
    let mut paths = Vec::new();
    collect(path, &mut paths)
        .map_err(|error| format!("Could not read suite {}: {error}", path.display()))?;
    paths.sort();
    if paths.is_empty() {
        return Err("Suite must contain at least one standalone .js test.".into());
    }
    let root = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(Path::new(""))
    };
    let files = paths.len();
    let mut cases = Vec::new();
    for path in paths {
        let id = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .ok_or_else(|| format!("Test path must be UTF-8: {}", path.display()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("Could not read test {id}: {error}"))?;
        let metadata =
            metadata(&source).map_err(|error| format!("Invalid metadata in {id}: {error}"))?;
        for variant in metadata.variants() {
            cases.push(Case {
                id: id.clone(),
                source: source.clone(),
                variant,
                negative: metadata.negative.clone(),
                flags: metadata.flags.clone(),
                includes: metadata.includes.clone(),
                features: metadata.features.clone(),
                locale: metadata.locale.clone(),
                esid: metadata.esid.clone(),
            });
        }
    }
    Ok(Suite { files, cases })
}

fn collect(path: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if !entry.file_type()?.is_symlink() {
                collect(&entry.path(), paths)?;
            }
        }
    } else if path.extension().is_some_and(|extension| extension == "js")
        && !path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("_FIXTURE")
    {
        paths.push(path.to_path_buf());
    }
    Ok(())
}

pub fn request(case: &Case, harness_root: &Path) -> Result<Request, String> {
    let mut names = Vec::new();
    if !matches!(case.variant, Variant::Raw) {
        names.extend(["assert.js", "sta.js"]);
        names.extend(case.includes.iter().map(String::as_str));
    }
    let harness = names
        .into_iter()
        .map(|filename| load_helper(harness_root, filename))
        .collect::<Result<Vec<_>, _>>()?;
    let source = match case.variant {
        Variant::Strict => format!("\"use strict\";\n{}", case.source),
        _ => case.source.clone(),
    };
    let parse_only = case
        .negative
        .as_ref()
        .is_some_and(|negative| negative.phase == Phase::Parse);
    Ok(Request {
        filename: case.id.clone(),
        source,
        harness,
        parse_only,
    })
}

fn load_helper(root: &Path, filename: &str) -> Result<Script, String> {
    if filename.is_empty()
        || !Path::new(filename)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(format!("Invalid harness include path: {filename:?}"));
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("Could not open harness directory: {error}"))?;
    let path = root
        .join(filename)
        .canonicalize()
        .map_err(|error| format!("Could not locate harness {filename}: {error}"))?;
    if !path.starts_with(&root) {
        return Err(format!(
            "Include points outside harness directory: {filename}"
        ));
    }
    let source = fs::read_to_string(path)
        .map_err(|error| format!("Could not read harness {filename}: {error}"))?;
    Ok(Script {
        filename: filename.into(),
        source,
    })
}

pub fn unsupported(case: &Case) -> Option<String> {
    const SUPPORTED_FLAGS: &[&str] = &[
        "onlyStrict",
        "noStrict",
        "raw",
        "generated",
        "non-deterministic",
    ];
    if let Some(flag) = case
        .flags
        .iter()
        .find(|flag| !SUPPORTED_FLAGS.contains(&flag.as_str()))
    {
        return Some(format!("Unsupported execution flag: {flag}"));
    }
    if case.features.iter().any(|feature| feature == "IsHTMLDDA") {
        return Some("The IsHTMLDDA host capability is not available.".into());
    }
    if !case.locale.is_empty() {
        return Some("Locale-specific execution is not supported yet.".into());
    }
    if case
        .negative
        .as_ref()
        .is_some_and(|negative| negative.phase == Phase::Resolution)
    {
        return Some("Module resolution is not supported yet.".into());
    }
    None
}

fn metadata(source: &str) -> Result<Metadata, String> {
    let mut header = source.trim_start_matches('\u{feff}');
    if header.starts_with("#!") {
        header = after_line(header);
    }
    loop {
        header = header.trim_start();
        if let Some(rest) = header.strip_prefix("/*---") {
            let (yaml, _) = rest
                .split_once("---*/")
                .ok_or("Unterminated metadata block.")?;
            let metadata: Metadata = if yaml.trim().is_empty() {
                Metadata::default()
            } else {
                yaml_serde::from_str(yaml).map_err(|error| error.to_string())?
            };
            const ANNOTATIONS: &[&str] = &["description", "info", "author", "es5id", "es6id"];
            if let Some(name) = metadata
                .annotations
                .keys()
                .find(|name| !ANNOTATIONS.contains(&name.as_str()))
            {
                return Err(format!("Unknown metadata field: {name}"));
            }
            let mode_flags = metadata
                .flags
                .iter()
                .filter(|flag| {
                    matches!(flag.as_str(), "onlyStrict" | "noStrict" | "raw" | "module")
                })
                .count();
            if mode_flags > 1 {
                return Err("Specify only one execution mode flag.".into());
            }
            if metadata
                .negative
                .as_ref()
                .is_some_and(|negative| negative.name.trim().is_empty())
            {
                return Err("Negative error type must not be empty.".into());
            }
            return Ok(metadata);
        }
        if header.starts_with("//") {
            header = after_line(header);
        } else if let Some(rest) = header.strip_prefix("/*") {
            match rest.split_once("*/") {
                Some((_, rest)) => header = rest,
                None => return Ok(Metadata::default()),
            }
        } else {
            return Ok(Metadata::default());
        }
    }
}

fn after_line(source: &str) -> &str {
    source
        .find(['\n', '\r', '\u{2028}', '\u{2029}'])
        .map_or("", |index| &source[index..])
}
