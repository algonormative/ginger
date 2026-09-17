//! Engine tests against the mock scsynth endpoint, and the golden-trace comparator.
//! No real scsynth: everything here runs in CI. Evidence label: mock.

#[path = "support/mock_scsynth.rs"]
mod mock_scsynth;
#[path = "support/trace.rs"]
mod trace;

use ginger_sc::osc::{self, Message, Reply};
use ginger_sc::readback::{Correlator, Outcome};
use mock_scsynth::{MockScsynth, Scripted};

const NODE: i32 = 1001;

fn feed(correlator: &mut Correlator, packets: Vec<Vec<u8>>) {
    for bytes in packets {
        let msg = Message::decode(&bytes).expect("reply decodes");
        let reply = Reply::parse(&msg).expect("reply is one we model");
        correlator.observe_reply(&reply);
    }
}

/// Dispatch a command through the mock, recording it as evidence-pending first.
fn dispatch(mock: &mut MockScsynth, correlator: &mut Correlator, msg: &Message) {
    correlator.observe_command(msg);
    let packets = mock.send(msg);
    feed(correlator, packets);
}

fn new_lead() -> Message {
    osc::s_new("sine", NODE, 0, 1, &[("freq", 440.0), ("amp", 0.2)])
}

#[test]
fn sync_barrier_alone_cannot_produce_verified_success() {
    let mut mock = MockScsynth::new();
    // The engine answers the barrier, but the node's readback never arrives.
    mock.script("/s_new", Scripted::Drop)
        .script("/sync", Scripted::Reply(vec![mock_scsynth::synced(7)]));
    let mut correlator = Correlator::new();

    dispatch(&mut mock, &mut correlator, &new_lead());
    dispatch(&mut mock, &mut correlator, &osc::sync(7));

    assert_eq!(correlator.barriers(), [7], "the barrier did come back");
    assert_eq!(
        correlator.outcome(NODE),
        Outcome::Unknown,
        "a /synced reply is ordering, not evidence about the node"
    );
    assert_eq!(correlator.unknown_nodes(), vec![NODE]);
    assert_eq!(mock.sent_packets().len(), 2, "both commands were recorded");
    // And a node nothing was ever observed about is unknown too, never applied.
    assert_eq!(correlator.outcome(4242), Outcome::Unknown);
}

#[test]
fn scripted_failure_marks_the_node_rejected() {
    let fail = mock_scsynth::fail_node("/s_new", "SynthDef not found", NODE);
    let mut mock = MockScsynth::new();
    mock.script("/s_new", Scripted::Reply(vec![fail]));
    let mut correlator = Correlator::new();
    dispatch(&mut mock, &mut correlator, &new_lead());
    assert_eq!(correlator.outcome(NODE), Outcome::Rejected);
    assert!(correlator.unknown_nodes().is_empty());
    // A later tree listing does not overturn an explicit failure.
    correlator.observe_reply(&Reply::QueryTreeReply(vec![NODE]));
    assert_eq!(correlator.outcome(NODE), Outcome::Rejected);
}

#[test]
fn delayed_readback_is_unknown_until_the_mock_is_advanced() {
    let mut mock = MockScsynth::new();
    mock.script("/s_new", Scripted::Delayed(vec![mock_scsynth::n_go(NODE)]))
        .script("/sync", Scripted::Reply(vec![mock_scsynth::synced(1)]));
    let mut correlator = Correlator::new();

    dispatch(&mut mock, &mut correlator, &new_lead());
    dispatch(&mut mock, &mut correlator, &osc::sync(1));
    assert_eq!(
        correlator.outcome(NODE),
        Outcome::Unknown,
        "the readback is still in flight; the barrier does not stand in for it"
    );
    let late = mock.deliver_pending();
    feed(&mut correlator, late);
    assert_eq!(correlator.outcome(NODE), Outcome::Applied);
    assert!(mock.deliver_pending().is_empty());
}

#[test]
fn tree_query_readback_marks_the_node_applied() {
    let tree = mock_scsynth::query_tree_reply(1, &[(NODE, "sine")]);
    let mut mock = MockScsynth::new();
    mock.script("/n_set", Scripted::Drop)
        .script("/g_queryTree", Scripted::Reply(vec![tree]));
    let mut correlator = Correlator::new();
    let set = osc::n_set(NODE, &[("amp", 0.4)]);
    dispatch(&mut mock, &mut correlator, &set);
    assert_eq!(correlator.outcome(NODE), Outcome::Unknown);
    dispatch(&mut mock, &mut correlator, &osc::g_query_tree(1, false));
    assert_eq!(correlator.outcome(NODE), Outcome::Applied);
}

#[test]
fn recorded_packets_round_trip_through_an_independent_decoder() {
    let mut mock = MockScsynth::new();
    for msg in [
        new_lead(),
        osc::n_set(NODE, &[("amp", 0.4)]),
        osc::sync(7),
        osc::g_query_tree(1, true),
    ] {
        mock.send(&msg);
    }
    let decoded = mock.sent_messages();
    let addresses: Vec<&str> = decoded.iter().map(|msg| msg.addr.as_str()).collect();
    assert_eq!(addresses, ["/s_new", "/n_set", "/sync", "/g_queryTree"]);
    // And our own decoder agrees with what we encoded.
    let ours = Message::decode(&mock.sent_packets()[0]).expect("decodes");
    assert_eq!(ours, new_lead());
}

const FIXTURE: &str = include_str!("../../../tests/engine/fixtures/barrier-and-readback.json");

fn fixture() -> trace::Trace {
    trace::load(FIXTURE)
}

#[test]
fn a_trace_equals_itself_and_a_copy_with_other_timestamps_and_engine_ids() {
    let expected = fixture();
    assert_eq!(expected.events.len(), 3);
    trace::compare(&expected, &fixture()).expect("a trace equals itself");

    let mut actual = fixture();
    for (index, event) in actual.events.iter_mut().enumerate() {
        event.wall_time = Some(format!("2026-09-06T11:22:33.{index:06}Z"));
        event.engine_node_id = Some(9000 + index as i32);
    }
    trace::compare(&expected, &actual).expect("transport fields are not logical content");
}

/// Mutate a copy of the fixture and return the differences the comparator reports.
fn diffs_after(mutate: impl FnOnce(&mut trace::Trace)) -> Vec<trace::Difference> {
    let mut actual = fixture();
    mutate(&mut actual);
    trace::compare(&fixture(), &actual).expect_err("a changed trace must fail")
}

#[test]
fn a_changed_logical_event_fails_the_comparison() {
    let kind = diffs_after(|t| t.events[1].kind = "voice_stop".to_string());
    assert_eq!(kind.len(), 1);
    assert_eq!(kind[0].index, Some(1));
    assert_eq!(
        kind[0].to_string(),
        r#"event[1].kind: expected "control_set", actual "voice_stop""#
    );

    let node = diffs_after(|t| t.events[0].node = "pad".to_string());
    assert_eq!(node[0].field, "node");
    let frame = diffs_after(|t| t.events[2].frame = 1921);
    assert_eq!(frame[0].field, "frame");
    let param = diffs_after(|t| drop(t.events[0].params.insert("freq".into(), "441".into())));
    assert_eq!(param[0].field, "params.freq");
    let dropped = diffs_after(|t| t.events.truncate(2));
    assert_eq!(dropped[0].field, "events.len");
}

#[test]
fn a_changed_named_state_value_fails_the_comparison() {
    let state = diffs_after(|t| drop(t.state.insert("section".into(), "verse".into())));
    assert_eq!(state.len(), 1);
    assert_eq!(state[0].index, None);
    assert_eq!(state[0].field, "state.section");
    assert_eq!(state[0].expected, "intro");
}
