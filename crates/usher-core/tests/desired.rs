mod common;
use common::*;
use usher_core::{Reason, Role, Skip, Skipped, State, decide};

fn st(out: &[&str], inp: &[&str]) -> State {
    let mut s = State::default();
    s.output.order = out.iter().map(|k| key(k)).collect();
    s.input.order = inp.iter().map(|k| key(k)).collect();
    s
}

fn sk(k: &str, why: Skip) -> Skipped {
    Skipped { key: key(k), why }
}

#[test]
fn top_connected_device_is_desired_per_role() {
    let s = st(&["a", "b"], &["b"]);
    let sn = snap(vec![dev("a", 1, 0), dev("b", 1, 1)], None, None);
    let o = decide(&s, &sn, Role::Output);
    assert_eq!(o.desired, Some(key("a")));
    assert!(o.skipped.is_empty());
    let i = decide(&s, &sn, Role::Input);
    assert_eq!(i.desired, Some(key("b")));
    assert!(i.skipped.is_empty());
}

#[test]
fn top_device_leaving_falls_back_to_next() {
    let s = st(&["a", "b"], &[]);
    let sn = snap(vec![dev("b", 1, 0)], None, None);
    let r = decide(&s, &sn, Role::Output);
    assert_eq!(r.desired, Some(key("b")));
    assert_eq!(r.skipped, vec![sk("a", Skip::Disconnected)]);
}

#[test]
fn each_skip_cause_is_reported() {
    let mut s = st(&["gone", "dead", "nostream", "never", "ok"], &[]);
    s.output.never_use.insert(key("never"));
    let mut dead = dev("dead", 1, 0);
    dead.alive = false;
    let sn = snap(
        vec![
            dead,
            dev("nostream", 0, 1),
            dev("never", 1, 0),
            dev("ok", 1, 0),
        ],
        None,
        None,
    );
    let r = decide(&s, &sn, Role::Output);
    assert_eq!(r.desired, Some(key("ok")));
    assert_eq!(
        r.skipped,
        vec![
            sk("gone", Skip::Disconnected),
            sk("dead", Skip::NotAlive),
            sk("nostream", Skip::NoStreams),
            sk("never", Skip::NeverUse),
        ]
    );
}

#[test]
fn never_use_is_scoped_by_role() {
    let mut s = st(&["h"], &["h"]);
    s.output.never_use.insert(key("h"));
    let sn = snap(vec![dev("h", 1, 1)], None, None);
    let o = decide(&s, &sn, Role::Output);
    assert_eq!(o.desired, None);
    assert_eq!(o.skipped, vec![sk("h", Skip::NeverUse)]);
    assert_eq!(decide(&s, &sn, Role::Input).desired, Some(key("h")));
}

#[test]
fn nothing_qualifies_gives_none() {
    let s = st(&["a", "b"], &[]);
    let sn = snap(vec![dev("b", 0, 1)], None, None);
    let r = decide(&s, &sn, Role::Output);
    assert_eq!(r.desired, None);
    assert_eq!(
        r.skipped,
        vec![sk("a", Skip::Disconnected), sk("b", Skip::NoStreams)]
    );
}

#[test]
fn current_none_still_sets_desired() {
    let s = st(&["a"], &[]);
    let sn = snap(vec![dev("a", 1, 0)], None, None);
    let r = decide(&s, &sn, Role::Output);
    assert_eq!(r.current, None);
    assert_eq!(r.desired, Some(key("a")));
}

#[test]
fn reason_display_line() {
    let r = Reason {
        role: Role::Output,
        desired: Some(key("b")),
        current: None,
        skipped: vec![sk("a", Skip::Disconnected)],
    };
    assert_eq!(
        r.to_string(),
        "output: desired=b current=none skipped=[a: disconnected]"
    );
}
