use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub filename: String,
    pub source: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub filename: String,
    pub source: String,
    pub harness: Vec<Script>,
    pub parse_only: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Completed {},
    Error {
        phase: Phase,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        message: String,
    },
    Unsupported {
        reason: String,
    },
    HarnessError {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Parse,
    Resolution,
    Runtime,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Variant {
    NonStrict,
    Strict,
    Raw,
    Module,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Negative {
    pub phase: Phase,
    #[serde(rename = "type")]
    pub name: String,
}

#[derive(Serialize)]
pub struct Case {
    pub id: String,
    pub source: String,
    pub variant: Variant,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub includes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locale: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub esid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative: Option<Negative>,
}

impl Case {
    pub fn matches(&self, actual: &Outcome) -> bool {
        match (&self.negative, actual) {
            (None, Outcome::Completed {}) => true,
            (
                Some(expected),
                Outcome::Error {
                    phase,
                    name: Some(name),
                    ..
                },
            ) => expected.phase == *phase && expected.name == *name,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    Skip,
    Timeout,
    Crash,
    HarnessError,
}

#[derive(Serialize)]
pub struct CaseResult {
    #[serde(flatten)]
    pub case: Case,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<Outcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
    pub duration_ms: u128,
}

#[derive(Default, Serialize)]
pub struct Summary {
    pub total: usize,
    pub pass: usize,
    pub fail: usize,
    pub skip: usize,
    pub timeout: usize,
    pub crash: usize,
    #[serde(rename = "harness-error")]
    pub harness_error: usize,
}

impl Summary {
    pub fn record(&mut self, status: Status) {
        self.total += 1;
        match status {
            Status::Pass => self.pass += 1,
            Status::Fail => self.fail += 1,
            Status::Skip => self.skip += 1,
            Status::Timeout => self.timeout += 1,
            Status::Crash => self.crash += 1,
            Status::HarnessError => self.harness_error += 1,
        }
    }
}

#[derive(Serialize)]
pub struct Report {
    pub suite: String,
    pub files: usize,
    pub harness: String,
    pub engine: &'static str,
    pub timeout_ms: u64,
    pub summary: Summary,
    pub results: Vec<CaseResult>,
}
