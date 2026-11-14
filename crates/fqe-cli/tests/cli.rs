use std::process::Command;

fn fqe() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fqe"))
}

#[test]
fn demo_runs_offline_and_is_deterministic() {
    let first = fqe().arg("demo").output().unwrap();
    let second = fqe().arg("demo").output().unwrap();
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stderr.is_empty());
    let stdout = String::from_utf8(first.stdout).unwrap();
    assert!(stdout.contains("status=completed events=3 signals=2"));
    assert!(
        stdout
            .contains("signal source_sequence=2 timestamp_ns=2000 instrument=DEMO:ASSET kind=long")
    );
    assert!(
        stdout
            .contains("signal source_sequence=3 timestamp_ns=3000 instrument=DEMO:ASSET kind=long")
    );
}

#[test]
fn unknown_or_extra_arguments_fail_instead_of_running_a_strategy() {
    for args in [vec!["backtest"], vec!["demo", "--live"]] {
        let output = fqe().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("unsupported arguments")
        );
    }
}

#[test]
fn help_and_version_expose_foundation_status() {
    for args in [vec![], vec!["--help"]] {
        let output = fqe().args(args).output().unwrap();
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("planned")
        );
    }
    let output = fqe().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("draft protocol 1.0")
    );
}
