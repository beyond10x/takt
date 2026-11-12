//! Adversarial cases for `takt::schedule` (wave 57, story:a-run-on-a-schedule): the slots of
//! windows at boundaries the unit's own tests do not reach.

use takt::schedule::{Schedule, Timestamp, every, window, zone};

fn at(text: &str) -> Timestamp {
    text.parse().expect("an instant")
}

fn schedule(every_text: &str, zone_name: &str, windows: &[&str]) -> Schedule {
    Schedule {
        every: every(every_text).expect("every reads"),
        zone: zone(zone_name).expect("a zone"),
        windows: windows
            .iter()
            .map(|text| window(text).expect("a window"))
            .collect(),
        until: Vec::new(),
        anchor: at("2026-01-01T00:00:00Z"),
    }
}

/// Every slot later than `from` and before `to`, each as `next_slot` gives it.
fn slots(schedule: &Schedule, from: Timestamp, to: Timestamp) -> Vec<Timestamp> {
    let mut found = Vec::new();
    let mut after = Some(from);
    while let Some((slot, _)) = schedule.next_slot(after, from) {
        if slot >= to {
            break;
        }
        found.push(slot);
        after = Some(slot);
    }
    found
}

/// `Schedule::every` holds any whole number of seconds up to `u64::MAX`; a window with such an `every`
/// is its start alone, once a day (design § 3: "the start and every `every` after it up to and
/// including its end").
#[test]
fn w57_adv_huge_every_window_is_its_start() {
    // Wave 57 fix round: `every` refuses above seven days, so the parser refuses this one; a
    // schedule built with it all the same keeps its slots.
    assert!(every("18446744073709551615s").is_err(), "refused at parse");
    let mut huge = schedule("1s", "UTC", &["Mon 00:01"]);
    huge.every = u64::MAX;
    // 2026-11-16 is a Monday.
    assert_eq!(
        huge.next_slot(None, at("2026-11-16T00:00:00Z")),
        Some((at("2026-11-16T00:01:00Z"), 0))
    );
}

/// Without windows "a slot falls every `every` from the start": the start is a slot whatever
/// `every` is, so a schedule never ends with `no-slot` before its first tick.
#[test]
fn w57_adv_huge_every_without_window_starts() {
    // Wave 57 fix round: refused at parse, as above; built with it all the same, it starts.
    assert!(every("9223372036854775808s").is_err(), "refused at parse");
    let mut plain = schedule("1s", "UTC", &[]);
    plain.every = 9_223_372_036_854_775_808;
    plain.windows.clear();
    let start = plain.anchor;
    assert_eq!(plain.next_slot(None, start), Some((start, 0)));
}

/// America/New_York, 2026-03-08: 02:00 EST becomes 03:00 EDT (07:00Z). 02:00 and 02:30 are in
/// the gap and move to 03:00, which is also a slot: one tick, not three.
#[test]
fn w57_adv_new_york_gap_and_fold() {
    let spring = schedule("30m", "America/New_York", &["Sun 01:30-03:00"]);
    assert_eq!(
        slots(
            &spring,
            at("2026-03-08T05:00:00Z"),
            at("2026-03-09T05:00:00Z")
        ),
        vec![at("2026-03-08T06:30:00Z"), at("2026-03-08T07:00:00Z")]
    );
    // 2026-11-01: 02:00 EDT becomes 01:00 EST (06:00Z); 01:00 and 01:30 are taken once, EDT.
    let autumn = schedule("30m", "America/New_York", &["Sun 01:00-01:30"]);
    assert_eq!(
        slots(
            &autumn,
            at("2026-11-01T04:00:00Z"),
            at("2026-11-02T05:00:00Z")
        ),
        vec![at("2026-11-01T05:00:00Z"), at("2026-11-01T05:30:00Z")]
    );
}

/// Two windows that name the same instant give one slot.
#[test]
fn w57_adv_overlapping_windows_one_tick() {
    let overlap = schedule("1h", "Europe/Berlin", &["Mon 09:00", "Sun-Mon 08:00-10:00"]);
    // Monday 2026-11-16 in Berlin (+01:00).
    assert_eq!(
        slots(
            &overlap,
            at("2026-11-15T23:00:00Z"),
            at("2026-11-16T23:00:00Z")
        ),
        vec![
            at("2026-11-16T07:00:00Z"),
            at("2026-11-16T08:00:00Z"),
            at("2026-11-16T09:00:00Z"),
        ]
    );
}
