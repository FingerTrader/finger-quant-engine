mod commands;

use std::{env, error::Error, process::ExitCode};

use fqe_core::{Event, InstrumentId, MarketTick, PriceTicks, SignalKind};
use fqe_ir::{IR_VERSION, Predicate, PriceExpr, Strategy};
use fqe_protocol::CURRENT_VERSION;
use fqe_runtime::Runtime;

const HELP: &str = "Finger Quant Engine (FQE) — experimental offline foundation

Usage: fqe <command>
  validate --model PATH [--params PATH]
              Compile a local fqe.event-plan/1 model (JSON result)
  run --model PATH --data PATH [--params PATH] [--run-id ID]
              Execute local fqe.market-data/1 inputs (JSON result)
  demo        Run a deterministic in-memory signal example (no orders)
  --version   Show engine and draft protocol versions
  --help      Show this help

Backtesting, simulation, workers, plugins, and Desktop IPC are planned.
No login, Flutter installation, network, or trading account is required.";

fn main() -> ExitCode {
    let raw: Vec<_> = env::args_os().skip(1).take(17).collect();
    if raw.first().is_some_and(|a| a == "validate" || a == "run") {
        if raw.len() == 2 && (raw[1] == "--help" || raw[1] == "-h") {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        return commands::dispatch(&raw);
    }
    let Ok(args) = raw
        .into_iter()
        .map(|a| a.into_string())
        .collect::<Result<Vec<_>, _>>()
    else {
        eprintln!("fqe: unsupported arguments\n\n{HELP}");
        return ExitCode::from(2);
    };
    match args.as_slice() {
        [] => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        [command] if command == "--help" || command == "-h" => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        [command] if command == "--version" || command == "-V" => {
            println!(
                "fqe {} (experimental; draft protocol {}.{})",
                env!("CARGO_PKG_VERSION"),
                CURRENT_VERSION.major,
                CURRENT_VERSION.minor
            );
            ExitCode::SUCCESS
        }
        [command] if command == "demo" => match demo() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("fqe: {error}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("fqe: unsupported arguments\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

fn demo() -> Result<(), Box<dyn Error>> {
    let instrument = InstrumentId::new("DEMO:ASSET")?;
    let strategy = Strategy::new(
        IR_VERSION,
        "price-threshold-demo",
        instrument.clone(),
        Predicate::GreaterThan {
            left: PriceExpr::Current,
            right: PriceExpr::Literal(PriceTicks::new(100)?),
        },
        SignalKind::Long,
    )?;
    let events = [(1, 1000, 90), (2, 2000, 110), (3, 3000, 105)]
        .into_iter()
        .map(|(sequence, timestamp_ns, price)| {
            Event::new(
                sequence,
                timestamp_ns,
                MarketTick {
                    instrument: instrument.clone(),
                    price: PriceTicks::new(price)?,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let report = Runtime::new(strategy).run(events)?;
    println!("FQE offline demo (experimental; no orders)");
    println!(
        "status={} events={} signals={}",
        report.state.status,
        report.state.processed_events,
        report.signals.len()
    );
    for record in report.signals {
        println!(
            "signal source_sequence={} timestamp_ns={} instrument={} kind={}",
            record.source_sequence,
            record.timestamp_ns,
            record.signal.instrument,
            record.signal.kind
        );
    }
    Ok(())
}
