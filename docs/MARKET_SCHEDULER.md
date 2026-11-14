# Market Scheduler and Alignment

Status: experimental P2-02 library profile, `fqe.schedule/1`. This contract owns fixed-UTC multi-symbol/timeframe ingress, inclusive watermarks, deterministic publication and as-of market alignment. [Events](EVENT_CONTRACT.md) and [compiled events](EVENT_EXECUTION.md) retain existing tick/closed-bar validation. P1 single-trigger files and [scalar workflows](WORKFLOW_EXECUTION.md) keep their original ordering/domain rules.

## Definition and supported calendar

`fqe_ir::schedule::ScheduleDefinition::from_bytes` strictly loads these exact root fields:

| Field | Version1 meaning |
| --- | --- |
| `format`, `schema_version` | fqe.schedule, integer1 |
| `id`, `version` | Bounded opaque identifiers |
| `requirements` | Exactly `["market.schedule/1"]`; unknown/duplicate/missing capabilities fail |
| `calendar` | Exactly `utc-epoch-fixed/1`; UTC nanoseconds and fixed durations aligned to Unix epoch0 |
| `queue_capacity` | Exact decimal string1..4096 |
| `streams` | 1..64 declarations `{id,dataset_id,domain,gap_policy}` |

Each domain reuses the P1 trigger shape: `{kind:tick,instrument}` or `{kind:closed_bar,instrument,timeframe_ns,price_scale,volume_scale}`. Duration is a positive exact decimal string; price scale0..12, volume scale0..18. Dataset IDs are exact sha256 identifiers supplied by the host; the scheduler does not read or authenticate dataset bytes. Stream IDs must be unique; `(instrument,event family,timeframe)` must be unique even when scales differ. Dataset/stream/domain/scale/policy/capacity/calendar are canonical identity inputs. Declaration order is normalized, source ID hashes exact outer bytes, artifact ID hashes canonical JSON under FQE-SCHEDULE-CANONICAL/1. Definition is immutable; no factories/strategy execution occurs during loading.

The shared bounded JSON decoder rejects unknown fields, duplicate keys, floats, unsafe numeric encodings, oversize/deep inputs and malformed identifiers. `model::identifier` exposes the existing common 1..128 ASCII identifier validation without constructing a runtime; its semantics are unchanged.

Fixed-UTC bars must have `open_ns % timeframe_ns == 0`, exact one-duration intervals, exclusive close and event timestamp equal to close. A bar must already be closed and pass core OHLC/volume/unit/scale validation. The first bar may start at any supported grid point; there is no inferred history before it. This is an explicit supported calendar domain, not an exchange calendar: sessions, holidays, DST, local timezone rules, variable-length days/months and synthetic aggregation/resampling are unsupported. Unclosed higher-timeframe bars are rejected; lower bars never manufacture a higher bar.

## Ingress and availability promises

`MarketScheduler` has a run ID, immutable definition, per-stream ingress cursors, a bounded merge queue and a committed alignment view. `ingest(stream,event)` requires a configured matching domain. Within each stream source sequence strictly increases (gaps are allowed) and timestamp never decreases (equal-time ticks are allowed). Different streams may arrive in any interleaving. Duplicate/reversed sequence, backward time, overlapping bars, invalid/unclosed/off-grid/wrong-domain bars and unsupported scales fail without changing the queue, cursors or counters.

Tick gap policy must be `not-applicable`. Bar gap policy is explicit:

- `reject`: the next open must equal the preceding accepted close; missing bars reject ingress.
- `report`: later nonoverlapping opens are accepted with a DataGap containing previous close and next open. The gap is attached to that bar and becomes visible only on its publication. No missing bar is synthesized, value forward-filled as a new sample, indicator automatically reset, or count window advanced for absent intervals. The consumer must handle/reject reported gaps; a P1 continuous-bar consumer still rejects them.

`advance_watermark(stream,t)` is an inclusive source promise: no subsequent input in that stream can have event time ≤t. It is independent of latest accepted time; buffered records beyond t can exist. Watermarks are monotonic and explicit, never inferred from ingress maxima or elapsed wall time. An event at or behind its own watermark is late and rejected, even before that event time has been published globally. There is no silent drop/repair/deduplication policy. `seal(stream)` irreversibly promises end-of-input and sets the stream watermark to u64::MAX; later watermark/ingress/repeat seal fail.

Global publication watermark is the minimum of all configured stream watermarks. If any stream lacks a promise, publication stalls. Empty/quiet streams must explicitly advance or seal; absence is not proof of availability. Sources own the truth of their promises; this library cannot detect an intentionally false declaration. Changing promises does not evaluate strategies or return a batch of outputs.

A full queue returns retryable schedule_backpressure without advancing ingress sequence/counts. A caller may publish eligible earlier records and retry unchanged input. If legitimate watermark progress cannot free space, the caller must stop/rebuild with sufficient capacity or throttle upstream; advancing a false watermark to bypass capacity would make older input late. There is no unbounded overflow buffer, implicit source eviction, timeout or data loss.

## Deterministic publication and alignment

Only buffered events with time ≤global watermark are eligible. Total order is:

1. Event timestamp ascending.
2. Case-sensitive instrument ID lexically ascending.
3. Tick before closed bar for one instrument at the same timestamp.
4. Closed-bar duration ascending for one instrument/family.
5. Original per-stream source sequence ascending.

Logical domains are unique, so these keys fully order records without arrival order or stream-ID tie breaks. Published records receive a fresh contiguous global logical sequence beginning1. The delivered InputEvent uses this sequence for existing event/workflow ordering. ScheduledEvent separately retains run, schedule artifact ID, stream, dataset ID, original source sequence and timestamp; source sequence is never confused with logical sequence. A strategy signal's source_sequence refers to the delivered logical sequence; retain the ScheduledEvent provenance mapping when persisting results. Tick/bar payload values and timestamps are preserved exactly, including full-width u64 boundaries.

`dispatch(consumer)` stages one next eligible record and candidate alignment, invokes the consumer, and commits queue removal, global sequence/counts and alignment only on success. A Result error leaves that record pending and all publication state unchanged; ingress cursors already describe accepted buffered data and remain unchanged. There is no automatic retry/drop. All scheduler fallible work precedes the consumer; success performs only in-memory publication. Callback-owned effects must themselves be transactional: the scheduler cannot undo a callback that mutates external state and then returns an error. `WorkflowRuntime::step` provides the required Result-based transaction when it is the consumer's final fallible action. Process/panic/allocation failure and external side effects remain outside this guarantee. `next_ready` unconditionally publishes one record for simpler consumers.

The candidate Alignment passed to dispatch includes the current record and latest **published** record from each other stream. Queued future records and queued later records at the same timestamp are inaccessible through it. Missing history is None, never a typed zero; `age_ns` distinguishes stale published data from current data. Alignment does not create another sample from an older value. At equal time, a lower-duration stream runs before the higher-duration stream: it sees the prior higher bar (or None); the later higher-bar callback sees the current lower bar. This is intentional sequential publication, not a simultaneous timestamp-batch join. There is no hidden lookahead through an unfinished higher bar or a queued same-time later phase.

No strategy is executed by watermark advancement, and scheduler metadata/ingress cursors are diagnostics rather than strategy market inputs. Consumers should evaluate from the immutable dispatched event/alignment view. `is_finished` means all streams sealed and the queue empty; the caller separately finishes its strategy runtime. Timers, timestamp-batch joins, cross-domain graph AST nodes and fair multi-instance dispatch remain separately versioned capabilities.

## Evidence and remaining scope

[Loaded definition](../crates/fqe-ir/tests/fixtures/schedule.json) configures AAA10/AAA30/BBB10 closed bars and DEMO ticks. [Independent ordering table](../crates/fqe-runtime/tests/fixtures/p2/alignment.expected.tsv) orders nine records at times10/20/30. It is hand-authored and never regenerated by tests. [IR tests](../crates/fqe-ir/tests/schedule_contract.rs) validate strict versions/fields/capabilities/calendars/domain duplication and identity/policy/source/capacity binding. [Runtime tests](../crates/fqe-runtime/tests/schedule_execution.rs) verify different arrival interleavings against the table, inclusive/stalled watermarks, exact global/source identities, same-time family/duration ties, full-width values, hidden queued future/partial higher bars, stale age, malformed/gap/late/duplicate/out-of-order rejection, explicit gap reporting, bounded retry, failed-publication rollback and actual transactional workflow consumption.

`cargo test --locked --offline -p fqe-runtime --test schedule_execution` executes the integration, including the unchanged stateful workflow with a signal at logical9/source200. No new dependency, toolchain, manifest or lockfile is needed. This library profile is independent of Flutter/login/accounts/network. It is not a CLI scheduling file runner, historical loader, checkpoint codec, exchange calendar service, multi-domain graph format or production throughput/isolation proof. P2-03 owns windows/timers/fair instance scheduling; P2-04 complete snapshots; P2-05 replay/debug; P2-06 temporal strategy gate. Financial/Desk/worker delivery remains later tasks.
