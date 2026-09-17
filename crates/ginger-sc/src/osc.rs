//! OSC encoding for the handful of scsynth commands v0 lowering dispatches, and decoding of
//! the replies the engine sends back. No I/O and no transport: the caller owns the socket
//! (or, in tests, the mock endpoint), so this module runs in CI.

/// An OSC argument; v0 lowering needs only these three.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub address: String,
    pub args: Vec<Arg>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn take_str<'a>(bytes: &'a [u8], pos: &mut usize) -> Result<&'a str, DecodeError> {
    let rest = bytes.get(*pos..).ok_or(DecodeError("truncated message"))?;
    let unterminated = DecodeError("unterminated string");
    let len = rest.iter().position(|b| *b == 0).ok_or(unterminated)?;
    let s = std::str::from_utf8(&rest[..len]).map_err(|_| DecodeError("non-utf8 string"))?;
    *pos += len + 1;
    *pos += (4 - *pos % 4) % 4;
    Ok(s)
}

fn take_4(bytes: &[u8], pos: &mut usize) -> Result<[u8; 4], DecodeError> {
    let truncated = DecodeError("truncated argument");
    let slice = bytes.get(*pos..*pos + 4).ok_or(truncated)?;
    *pos += 4;
    slice.try_into().map_err(|_| truncated)
}

impl Message {
    pub fn new(address: &str, args: Vec<Arg>) -> Self {
        Message {
            address: address.to_string(),
            args,
        }
    }

    /// Encode to OSC 1.0 wire bytes (big endian, four-byte aligned).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        push_str(&mut out, &self.address);
        let mut tags = String::from(",");
        for arg in &self.args {
            tags.push(match arg {
                Arg::Int(_) => 'i',
                Arg::Float(_) => 'f',
                Arg::Str(_) => 's',
            });
        }
        push_str(&mut out, &tags);
        for arg in &self.args {
            match arg {
                Arg::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
                Arg::Str(s) => push_str(&mut out, s),
            }
        }
        out
    }

    /// Decode a single OSC 1.0 message. Bundles and argument types outside `i`, `f` and `s`
    /// are rejected rather than skipped: unread evidence must not look like read evidence.
    pub fn decode(bytes: &[u8]) -> Result<Message, DecodeError> {
        let mut pos = 0;
        let address = take_str(bytes, &mut pos)?.to_string();
        let tags = take_str(bytes, &mut pos)?.to_string();
        let mut chars = tags.chars();
        if chars.next() != Some(',') {
            return Err(DecodeError("missing type tag string"));
        }
        let mut args = Vec::new();
        for tag in chars {
            args.push(match tag {
                'i' => Arg::Int(i32::from_be_bytes(take_4(bytes, &mut pos)?)),
                'f' => Arg::Float(f32::from_be_bytes(take_4(bytes, &mut pos)?)),
                's' => Arg::Str(take_str(bytes, &mut pos)?.to_string()),
                _ => return Err(DecodeError("unsupported type tag")),
            });
        }
        Ok(Message { address, args })
    }

    pub fn int(&self, index: usize) -> Option<i32> {
        match self.args.get(index) {
            Some(Arg::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn text(&self, index: usize) -> Option<&str> {
        match self.args.get(index) {
            Some(Arg::Str(s)) => Some(s.as_str()),
            _ => None,
        }
    }
}

fn control_args(controls: &[(&str, f32)]) -> Vec<Arg> {
    controls
        .iter()
        .flat_map(|(name, value)| [Arg::Str((*name).to_string()), Arg::Float(*value)])
        .collect()
}

/// `/s_new defName nodeID addAction targetID [control value]*`
pub fn s_new(def: &str, node: i32, action: i32, target: i32, ctl: &[(&str, f32)]) -> Message {
    let ids = [node, action, target].map(Arg::Int);
    let mut args = vec![Arg::Str(def.to_string())];
    args.extend(ids);
    args.extend(control_args(ctl));
    Message::new("/s_new", args)
}

/// `/n_set nodeID [control value]*`
pub fn n_set(node_id: i32, controls: &[(&str, f32)]) -> Message {
    let mut args = vec![Arg::Int(node_id)];
    args.extend(control_args(controls));
    Message::new("/n_set", args)
}

/// `/sync id` — an ordering barrier, never operation-specific proof (design §2).
pub fn sync(id: i32) -> Message {
    Message::new("/sync", vec![Arg::Int(id)])
}

/// `/g_queryTree groupID flag`
pub fn g_query_tree(group_id: i32, with_controls: bool) -> Message {
    let args = vec![Arg::Int(group_id), Arg::Int(i32::from(with_controls))];
    Message::new("/g_queryTree", args)
}

/// A reply from scsynth, in the vocabulary the correlator reasons about.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// `/synced id` — the barrier only. Says nothing about any node.
    Synced(i32),
    /// `/n_go nodeID …` — the node exists.
    NodeStarted(i32),
    /// `/fail command message [nodeID]`
    Failed {
        command: String,
        message: String,
        node_id: Option<i32>,
    },
    /// The node ids a `/g_queryTree.reply` listed.
    QueryTreeReply(Vec<i32>),
}

impl Reply {
    /// Interpret a decoded message, or `None` when it is not a reply we model.
    pub fn parse(msg: &Message) -> Option<Reply> {
        match msg.address.as_str() {
            "/synced" => Some(Reply::Synced(msg.int(0)?)),
            "/n_go" => Some(Reply::NodeStarted(msg.int(0)?)),
            "/fail" => Some(Reply::Failed {
                command: msg.text(0)?.to_string(),
                message: msg.text(1).unwrap_or_default().to_string(),
                node_id: msg.int(2),
            }),
            "/g_queryTree.reply" => Some(Reply::QueryTreeReply(query_tree_node_ids(msg)?)),
            _ => None,
        }
    }
}

/// Walk a `/g_queryTree.reply` body, collecting the node ids it lists.
fn query_tree_node_ids(msg: &Message) -> Option<Vec<i32>> {
    let with_controls = msg.int(0)? == 1;
    let (mut ids, mut i) = (Vec::new(), 1);
    while i < msg.args.len() {
        ids.push(msg.int(i)?);
        let children = msg.int(i + 1)?;
        i += 2;
        if children == -1 {
            i += 1; // a synth: its SynthDef name, then optionally its control pairs
            if with_controls {
                i += 1 + 2 * usize::try_from(msg.int(i)?).ok()?;
            }
        }
    }
    Some(ids)
}
