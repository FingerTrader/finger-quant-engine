mod support {
    pub mod events;
}
use support::events;
fn main() {
    let observations = events::run(events::G002, &[1, 2, 3, 2, 4, 3]);
    let signals: Vec<_> = observations
        .iter()
        .flat_map(|o| &o.signals)
        .map(|s| s.source_sequence)
        .collect();
    assert_eq!(signals, vec![5]);
    let sma_signals: Vec<_> = events::run(events::G007, &[3, 2, 1, 2, 3, 4])
        .into_iter()
        .flat_map(|o| o.signals)
        .map(|s| s.source_sequence)
        .collect();
    assert_eq!(sma_signals, vec![5]);
    println!(
        "G-002 MACD/RSI: signal bar {signals:?}; G-007 SMA: signal bar {sma_signals:?}; offline event transactions OK"
    );
}
