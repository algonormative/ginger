//! A deterministic, in-process stand-in for scsynth. No sockets, no threads, no timers: a test
//! hands it encoded OSC and it hands back encoded reply packets according to a script — a normal
//! reply, a failure, a dropped packet, or one withheld until [`MockScsynth::deliver_pending`].
//! Outgoing packets are recorded as the bytes the library produced; replies are built with
//! `rosc`, an independent implementation, so neither direction is graded by the code under test.

use ginger_sc::osc::Message;
use rosc::{OscMessage, OscPacket, OscType};
use std::collections::{BTreeMap, VecDeque};

/// How the mock answers one command.
pub enum Scripted {
    Reply(Vec<Vec<u8>>),
    /// Record the command and never answer it — a dropped packet.
    Drop,
    /// Withhold these packets until `deliver_pending` is called.
    Delayed(Vec<Vec<u8>>),
}

/// The scripted engine. A command with no script left for its address is dropped.
#[derive(Default)]
pub struct MockScsynth {
    script: BTreeMap<String, VecDeque<Scripted>>,
    sent: Vec<Vec<u8>>,
    pending: Vec<Vec<u8>>,
}

impl MockScsynth {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn script(&mut self, address: &str, scripted: Scripted) -> &mut Self {
        let queue = self.script.entry(address.to_string()).or_default();
        queue.push_back(scripted);
        self
    }

    /// Send a command. Returns the reply packets available immediately, which may be none.
    pub fn send(&mut self, msg: &Message) -> Vec<Vec<u8>> {
        self.sent.push(msg.encode());
        let queue = self.script.get_mut(&msg.address);
        match queue.and_then(VecDeque::pop_front) {
            Some(Scripted::Reply(packets)) => packets,
            Some(Scripted::Delayed(packets)) => {
                self.pending.extend(packets);
                Vec::new()
            }
            Some(Scripted::Drop) | None => Vec::new(), // recorded, never answered
        }
    }

    pub fn deliver_pending(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.pending)
    }

    pub fn sent_packets(&self) -> &[Vec<u8>] {
        &self.sent
    }

    /// The outgoing packets decoded by `rosc` — the round-trip check on our encoder.
    pub fn sent_messages(&self) -> Vec<OscMessage> {
        self.sent
            .iter()
            .map(|bytes| match rosc::decoder::decode_udp(bytes) {
                Ok(([], OscPacket::Message(msg))) => msg,
                other => panic!("recorded packet is not a single OSC message: {other:?}"),
            })
            .collect()
    }
}

fn packet(address: &str, args: Vec<OscType>) -> Vec<u8> {
    let addr = address.to_string();
    rosc::encoder::encode(&OscPacket::Message(OscMessage { addr, args })).expect("encode reply")
}

/// `/synced id`
pub fn synced(id: i32) -> Vec<u8> {
    packet("/synced", vec![OscType::Int(id)])
}

/// `/n_go nodeID parentID prevID nextID isGroup`
pub fn n_go(node_id: i32) -> Vec<u8> {
    packet("/n_go", [node_id, 1, -1, -1, 0].map(OscType::Int).to_vec())
}

/// `/fail command message nodeID`
pub fn fail_node(command: &str, message: &str, node_id: i32) -> Vec<u8> {
    let text = |s: &str| OscType::String(s.to_string());
    packet(
        "/fail",
        vec![text(command), text(message), OscType::Int(node_id)],
    )
}

/// `/g_queryTree.reply` for a group holding `synths`, with controls omitted.
pub fn query_tree_reply(group_id: i32, synths: &[(i32, &str)]) -> Vec<u8> {
    let mut args = vec![
        OscType::Int(0),
        OscType::Int(group_id),
        OscType::Int(synths.len() as i32),
    ];
    for (node_id, def_name) in synths {
        args.push(OscType::Int(*node_id));
        args.push(OscType::Int(-1));
        args.push(OscType::String((*def_name).to_string()));
    }
    packet("/g_queryTree.reply", args)
}
