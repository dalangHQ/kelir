// see: cans/plugins/taint.md#Invariants-and-policy-(taint)
use plugin_taint::{ByteRange, TaintSet, TaintTracker, TaintTrackingService};

// see: cans/plugins/taint.md#Invariants-and-policy-(taint)
#[test]
fn taint_set_marks_and_propagates_byte_ranges() -> Result<(), String> {
    let mut set = TaintSet::default();
    set.mark(ByteRange { start: 4, end: 9 });
    set.mark(ByteRange { start: 4, end: 9 });
    assert_eq!(
        set.propagate(6),
        vec![ByteRange { start: 4, end: 6 }],
        "propagation clamps ranges to the target length"
    );
    assert!(
        set.overlaps(ByteRange { start: 8, end: 12 }),
        "overlapping range must report tainted"
    );
    assert!(
        !set.overlaps(ByteRange { start: 9, end: 12 }),
        "adjacent range must report clean"
    );
    assert!(!set.is_empty(), "marked set must not report empty");
    Ok(())
}

// see: cans/plugins/taint.md#Service-exports-(taint)
#[test]
fn taint_tracker_labels_propagate_per_source() -> Result<(), String> {
    let tracker = TaintTracker::new();
    tracker
        .mark("read_file", ByteRange { start: 0, end: 8 })
        .map_err(|err| format!("{err:?}"))?;
    assert_eq!(
        tracker
            .propagate("read_file", 5)
            .map_err(|err| format!("{err:?}"))?,
        vec![ByteRange { start: 0, end: 5 }],
        "tracker must propagate the labelled span clamped to the target"
    );
    assert!(
        tracker
            .overlaps("read_file", ByteRange { start: 7, end: 10 })
            .map_err(|err| format!("{err:?}"))?,
        "tracker must report overlap on the labelled span"
    );
    assert!(
        !tracker
            .overlaps("write_file", ByteRange { start: 0, end: 8 })
            .map_err(|err| format!("{err:?}"))?,
        "labels must not leak across sources"
    );
    Ok(())
}
