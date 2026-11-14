# Project Charter

Authority: the single long-term product definition. See [Document authority](DOCUMENT_AUTHORITY.md) for scope and change rules and [Project status](PROJECT_STATUS.md) for delivered capabilities.

## Product

**Finger Quant Engine (FQE) — Universal Local-First Quantitative Strategy Engine.**

FQE is an independent, language-agnostic, embeddable and extensible engine for defining, debugging, backtesting, optimizing, simulating and running quantitative strategies. Rust owns core semantics. A common typed model serves visual authoring; isolated language workers serve code strategies. A professional developer can use the standalone CLI or embed the engine without a Desktop application.

This is the destination, not a claim that these capabilities exist today. The audited baseline is a small experimental rule interpreter; [Baseline audit](BASELINE_AUDIT.md) identifies its limits.

The primary product integration destination is the existing **Desk** desktop application (`finger_desk`). FQE must be bundled into Desk to deliver usable local quantitative research. The first product delivery is the historical-data **backtest and local simulation loop**: saved strategy/parameters + local data → execution → orders/fills/ledger → reports/replay and historical reopening. Live trading is a later separately accepted scope. Desk remains optional for standalone engine users; actual Desk integration is mandatory for this project's product delivery. [Desk integration](DESK_INTEGRATION_PLAN.md) defines ownership, migration, milestones and native acceptance.

## Non-negotiable boundaries

| Boundary | Product requirement |
| --- | --- |
| Local-First | Strategy computation and decisions run on the user's computer or user-controlled server. Finger Trader does not operate a cloud customer-strategy execution service. |
| Offline independence | Local validation, backtesting and simulation require neither Flutter, Finger Trader login nor a trading account. Installing tools and obtaining data are separate setup concerns. |
| Language independence | Rust core; open adapter registration via language-neutral versioned contracts. Python → Java → Go are initial independently accepted references, not a closed language list or runtime dependency chain. New languages follow the same onboarding/conformance path. |
| Runtime independence | CLI, Desktop and external programs are clients of one engine. Flutter must not implement a second strategy evaluator. |
| Financial ownership | Local simulation owns virtual accounts, orders, fills, cash, positions, costs, leverage and broker simulation rules on the user's device. No Finger Trader backend simulated account is required. |
| Explicit scope | Simulation is a versioned model with stated assumptions; supported instruments and execution rules must be declared and tested. |

## FQE and Trading SDK

The **Finger Trader Trading SDK is a separate product**. Developers can use it without installing FQE. It owns market-data APIs, account APIs, order APIs, trading status/fill feedback and WebSocket connections. FQE owns model definitions, indicators, expressions, strategy execution, historical backtests, local optimization/simulation/debugging and model state management.

The only allowed dependency direction is optional FQE adapter → Trading SDK. The SDK must never depend on FQE. Broker credentials and SDK implementations do not belong in core crates, IR, worker messages or generic reports. Live execution requires an explicit mode and authorization boundary; an observation signal is not an order. Offline simulation must never silently route to a live adapter.

## Extensibility destination

One typed IR supports No-Code Visual Strategy Builder models, indicators, custom functions and reusable strategy modules. Arbitrary Python/Java/Go programs run in workers; lossless conversion of arbitrary code to visual models is not promised. A versioned plugin registry, model packaging and marketplace compatibility allow third-party extensions after compatibility, permission, resource and fault-isolation gates. Marketplace availability is not an engine execution dependency.

The extensibility requirement is open-ended: languages beyond the initial references and newly developed financial indicators/rules must have an explicit supported integration path. Within supported types/events/capabilities, a conforming new adapter, computation provider or composed rule must not require engine-core edits/rebuilds, a new per-indicator IR enum or a dedicated Desk page. New bottom-level capabilities use versioned contract evolution. [Extensibility contract](EXTENSIBILITY_CONTRACT.md) distinguishes engine clients, whole-strategy runners and callable computation providers, with early unknown-name provider tests, a real fourth-language onboarding test and cross-language indicator/predicate graph acceptance. Actual support always requires an implemented adapter/provider and evidence.

## Delivery and success

Deliver vertical slices with model/data fixtures, executable behavior, negative tests and updated contracts. The six gates are [P1–P6](DELIVERY_PLAN.md): universal strategy foundation; stateful runtime; local backtesting/simulation; Desktop/external integration; language workers; plugin ecosystem/release. No calendar promise or production readiness follows from a package version or a design document.

Success means a user can load a versioned strategy, validate it, reproduce local results, inspect decisions, preserve/restore supported state, and extend supported interfaces without copying strategy semantics into clients. Each stage must prove its own acceptance criteria; planned capabilities remain planned until evidence exists.

Product completion also requires a supported installed Desk package to perform that offline loop using the final accepted FQE/worker versions. Library/CLI tests, a prototype UI or a development-only engine path cannot substitute for the native application, package, migration and customer-environment evidence. P4 proves the first Desk loop; P5 independently integrates supported Python, Java and Go code strategies; P6 repeats final bundled acceptance including external graph providers. Existing Desk strategies, local data and historical results need explicit preservation/compatibility rules.

P5 also proves the generic additional-language onboarding path; the reference languages each require actual Desk evidence for claimed supported combinations. Final P6 acceptance includes discovery and execution of an external indicator and custom predicate in one graph, generic parameter/port presentation and extension state/recovery/version/resource tests. Engine compatibility, language runtime support and Desk bundle availability are separate published matrices.

Changes to these boundaries require an explicit product decision and corresponding ADR. See [Architecture decisions](ARCHITECTURE_DECISIONS.md) and [Workflow](WORKFLOW.md).
