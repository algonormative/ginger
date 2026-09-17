//! Golden traces: an ordered list of logical events plus the named state they leave behind.
//! Per CLAUDE.md they compare logical events and state, never transport timestamps. An event may
//! carry `wall_time` and the engine-allocated `engine_node_id`; the comparator reads neither.
//! Fixtures live in `tests/engine/fixtures/`.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Debug, Deserialize)]
pub struct Trace {
    pub name: String,
    pub events: Vec<Event>,
    #[serde(default)]
    pub state: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    pub frame: u64,
    pub node: String,
    pub kind: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// Transport only; recorded for reading, never compared.
    #[serde(default)]
    pub wall_time: Option<String>,
    /// Generation-scoped, so never compared.
    #[serde(default)]
    pub engine_node_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub index: Option<usize>,
    pub field: String,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (expected, actual) = (&self.expected, &self.actual);
        match self.index {
            Some(index) => write!(f, "event[{index}].{}", self.field)?,
            None => write!(f, "{}", self.field)?,
        }
        write!(f, ": expected {expected:?}, actual {actual:?}")
    }
}

pub fn load(json: &str) -> Trace {
    serde_json::from_str(json).expect("fixture trace parses")
}

const ABSENT: &str = "<absent>";

fn diff<T: fmt::Display + PartialEq>(
    out: &mut Vec<Difference>,
    index: Option<usize>,
    field: &str,
    want: T,
    got: T,
) {
    if want != got {
        out.push(Difference {
            index,
            field: field.to_string(),
            expected: want.to_string(),
            actual: got.to_string(),
        });
    }
}

type Map = BTreeMap<String, String>;

fn diff_map(out: &mut Vec<Difference>, index: Option<usize>, prefix: &str, want: &Map, got: &Map) {
    let keys: BTreeSet<&String> = want.keys().chain(got.keys()).collect();
    for key in keys {
        let want = want.get(key).map(String::as_str).unwrap_or(ABSENT);
        let got = got.get(key).map(String::as_str).unwrap_or(ABSENT);
        diff(out, index, &format!("{prefix}.{key}"), want, got);
    }
}

/// Compare two traces on logical content alone.
pub fn compare(expected: &Trace, actual: &Trace) -> Result<(), Vec<Difference>> {
    let mut out = Vec::new();
    diff(&mut out, None, "name", &expected.name, &actual.name);
    let lens = (expected.events.len(), actual.events.len());
    diff(&mut out, None, "events.len", lens.0, lens.1);
    for (i, (want, got)) in expected.events.iter().zip(actual.events.iter()).enumerate() {
        let i = Some(i);
        diff(&mut out, i, "frame", want.frame, got.frame);
        diff(&mut out, i, "node", &want.node, &got.node);
        diff(&mut out, i, "kind", &want.kind, &got.kind);
        diff_map(&mut out, i, "params", &want.params, &got.params);
    }
    diff_map(&mut out, None, "state", &expected.state, &actual.state);
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}
