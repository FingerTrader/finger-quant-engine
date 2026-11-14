use fqe_core::{Event, InstrumentId, MarketTick, PriceTicks, SignalKind};
use fqe_ir::{IR_VERSION, Predicate, PriceExpr, Strategy};
use fqe_runtime::{RunStatus, Runtime, RuntimeError};

fn runtime() -> Runtime {
    Runtime::new(
        Strategy::new(
            IR_VERSION,
            "threshold",
            InstrumentId::new("DEMO").unwrap(),
            Predicate::GreaterThan {
                left: PriceExpr::Current,
                right: PriceExpr::Literal(PriceTicks::new(100).unwrap()),
            },
            SignalKind::Long,
        )
        .unwrap(),
    )
}

fn event(sequence: u64, timestamp_ns: u64, price: u64, instrument: &str) -> Event {
    Event::new(
        sequence,
        timestamp_ns,
        MarketTick {
            instrument: InstrumentId::new(instrument).unwrap(),
            price: PriceTicks::new(price).unwrap(),
        },
    )
    .unwrap()
}

#[test]
fn repeated_inputs_produce_identical_reports_and_signal_provenance() {
    let inputs = vec![
        event(1, 1000, 90, "DEMO"),
        event(2, 2000, 110, "DEMO"),
        event(3, 3000, 105, "DEMO"),
    ];
    let first = runtime().run(inputs.clone()).unwrap();
    let second = runtime().run(inputs).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.state.status, RunStatus::Completed);
    assert_eq!(first.state.processed_events, 3);
    assert_eq!(first.state.last_sequence, Some(3));
    assert_eq!(first.state.last_timestamp_ns, Some(3000));
    assert_eq!(first.signals.len(), 2);
    assert_eq!(first.signals[0].source_sequence, 2);
    assert_eq!(first.signals[0].timestamp_ns, 2000);
    assert_eq!(first.signals[0].signal.kind, SignalKind::Long);
    assert_eq!(first.signals[1].source_sequence, 3);
}

#[test]
fn duplicate_or_backward_sequence_fails_without_consuming_invalid_event() {
    for received in [1, 2] {
        let mut runtime = runtime();
        let error = runtime
            .run([
                event(2, 1000, 110, "DEMO"),
                event(received, 2000, 110, "DEMO"),
            ])
            .unwrap_err();
        assert_eq!(
            error,
            RuntimeError::NonIncreasingSequence {
                previous: 2,
                received,
            }
        );
        assert_eq!(runtime.state().status, RunStatus::Failed);
        assert_eq!(runtime.state().processed_events, 1);
        assert_eq!(runtime.state().last_sequence, Some(2));
        assert_eq!(
            runtime.run([]),
            Err(RuntimeError::AlreadyStarted(RunStatus::Failed))
        );
    }
}

#[test]
fn timestamps_cannot_move_backward() {
    let mut runtime = runtime();
    assert_eq!(
        runtime.run([event(1, 10, 90, "DEMO"), event(2, 9, 110, "DEMO")]),
        Err(RuntimeError::BackwardTimestamp {
            previous: 10,
            received: 9,
        })
    );
    assert_eq!(runtime.state().status, RunStatus::Failed);
    assert_eq!(runtime.state().last_timestamp_ns, Some(10));
    assert_eq!(runtime.state().processed_events, 1);
}

#[test]
fn equal_times_and_sequence_gaps_are_allowed() {
    let report = runtime()
        .run([event(3, 0, 110, "DEMO"), event(u64::MAX, 0, 110, "DEMO")])
        .unwrap();
    assert_eq!(report.state.processed_events, 2);
    assert_eq!(report.signals.len(), 2);
}

#[test]
fn unrelated_instruments_are_counted_but_emit_no_signal() {
    let report = runtime()
        .run([event(1, 0, 110, "OTHER"), event(2, 1, 110, "DEMO")])
        .unwrap();
    assert_eq!(report.state.processed_events, 2);
    assert_eq!(report.signals.len(), 1);
    assert_eq!(report.signals[0].source_sequence, 2);
}

#[test]
fn completed_runs_are_one_shot_including_empty_input() {
    let mut runtime = runtime();
    assert_eq!(runtime.state().status, RunStatus::Ready);
    let report = runtime.run([]).unwrap();
    assert_eq!(report.state.status, RunStatus::Completed);
    assert_eq!(report.state.processed_events, 0);
    assert_eq!(report.state.last_sequence, None);
    assert_eq!(report.state.last_timestamp_ns, None);
    assert!(report.signals.is_empty());
    assert_eq!(
        runtime.run([]),
        Err(RuntimeError::AlreadyStarted(RunStatus::Completed))
    );
}
