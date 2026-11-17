# Python hosted native execution

Status: IMPLEMENTED / locally verified, GUI and formal distribution pending (2026-11-18), owner-authorized Python-only work. This does not complete P4/P5 or add other language support.

Additive `hosted.execution/1` sessions reuse research Prepared/Broker, native indicators and unchanged financial policies. No credentials, network trading or Python formulas enter FQE. Desk owns Python callbacks, durable proposal commits, authorization and cleanup. Native candidates cannot advance while uncommitted; a market callback must complete before the next market step. Start/warmup/stop are read-only. Failures never roll back acknowledged trades.

Hosted APIs and parameter semantics remain experimental until paired MACD, margin/protection, stop/discard, persistence and actual Desktop acceptance pass. Existing graph execution and reports retain their profiles. SDK changes must be committed/pushed as authorized.

## Local evidence (2026-11-18)

Native hosted five-case suite and strict workspace Clippy PASS. Graph/hosted paired MACD tests PASS for 180 and 7200 bars, including every chart observation and committed account, fill timestamp/side/price/quantity/fee, final metrics and clean restart. The 7200-bar fixed vector yields 358 fills and fees39.38, final equity101035.62. Desktop application controls separately pass pause, single-step, stop/restart, stop during launch, log archival, verified report reopening, and no old-history loading on Play.

Initial declared indicator output fields are explicitly null before valid data; they are not absent or numeric zero. FQE reuses catalog ports. SDK main de57c9dce2a826d2785ed0c1c51c5715711a5d5a is pushed/remote-verified; Python3.10–3.14 CI test jobs and Quant contracts PASS (secrets job requires repository Gitleaks License). Paired Desk has built/signed and GUI-created/bound a Python project, but the actual GUI backtest remains pending because the Mac locked. No real orders were sent. This is local backtest evidence, not a P4/P5 phase closure or real-account Python acceptance. The native session does not manage SDK installation or presentation; see the newer live-adapter update below.


## Python live adapter and native studies

`signals.indicators/1` adds `signals.indicator({handle, study:{id,kind,parameters}})` to the existing account-free signal session. New instances must be declared before market advancement, and use the same native registry, output ports and versioned kernels as batch/backtest studies. Repeating an ID with different parameters, late declarations, unsupported parameters and mutation of authored decision graphs fail explicitly. Warmup values remain null. Desktop owns actual broker orders, recovery and account authorization; FQE receives no credentials and fabricates no live fills.

The isolated integration executes the actual Python worker and this peer, with a mocked broker only: protected whole-contract market entry/exit, broker-confirmed fees, rejection, lost response, account change and cleanup pass. Custom Python drawings are a Desktop journal sidecar, never native financial values. SDK fixed commit is now 11ae5baebd47a8b8ee726d3c962920a573cc3762;105 tests/1324 subtests,42 contract tests/416 shared Dart vectors PASS. Full native workspace445 tests and strict Clippy PASS. No new dependency or lockfile change. Actual account trading and signed independent SDK Release are not claimed.
