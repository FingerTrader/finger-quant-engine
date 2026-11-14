# P1-03 Dependency Review

Status on 2026-11-14: **owner-approved scope; reviewed and locally verified**. The owner approved the proposed packages, reviewed transitive dependencies/build scripts and public-only advisory queries on 2026-11-14. Direct and full-closure inspection preceded first execution. Locked fetch, offline builds and acceptance passed as recorded below. Approval alone is not advisory clearance. Progress authority: [Project status](PROJECT_STATUS.md).

## Purpose and alternatives

P1-03 needs a bounded language-neutral external model decoder and a standard content digest. Selected experimental design (ADR-014): strict JSON decoding with duplicate-key rejection, explicit full-width numeric strings, canonical serialization of normalized declarations and SHA-256 content identity. Keep source-byte identity separate from canonical model identity. Canonicalization is FQE-specific and versioned, not an unsupported claim of RFC 8785 compatibility.

The standard library contains no JSON decoder or SHA-256 implementation. Handwriting either would introduce avoidable parser/cryptography maintenance and verification work. Protobuf remains a candidate for later boundaries; the current price-only draft cannot encode the universal model, and code generators/dependencies would also require review. A custom netstring/TLV codec is feasible but creates a separate ecosystem/adapter burden without evidence that JSON is insufficient. ADR-014 accepts this model-artifact codec after the implemented fixtures/checks; future transport remains undecided.

## Direct candidates inspected

Official crates.io version metadata and published source archives were read without executing them. Release age exceeds one year for each selected version as of the review date. All three identify MIT OR Apache-2.0 licensing and are not yanked in the inspected metadata. Recorded archive checksums were independently verified from downloaded public archives against Cargo.lock, followed by locked Cargo fetch/build.

| Package/version | Official repository / authors | Published UTC | Registry checksum |
| --- | --- | --- | --- |
| serde 1.0.228 | [serde-rs/serde](https://github.com/serde-rs/serde); Erick Tryzelaar / David Tolnay | 2025-09-27 | 9a8e94ea7f378bd32cbbd37198a4a91436180c5bb472411e48b5ec2e2124ae9e |
| serde_json 1.0.145 | [serde-rs/json](https://github.com/serde-rs/json); Erick Tryzelaar / David Tolnay | 2025-09-14 | 402a6f66d8c709116cf22f558eab210f5a50187f702eb4d7e5ef38d9a7f1c79c |
| sha2 0.10.9 | [RustCrypto/hashes](https://github.com/RustCrypto/hashes); RustCrypto Developers | 2025-04-30 | a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283 |

Version sources: [serde metadata](https://crates.io/api/v1/crates/serde/1.0.228), [serde_json metadata](https://crates.io/api/v1/crates/serde_json/1.0.145), [sha2 metadata](https://crates.io/api/v1/crates/sha2/0.10.9). Author metadata and repositories are provenance signals, not proof of complete maintainer-account history or absence of compromise. Current owners and public advisory results are recorded below; complete historical ownership/compromise absence is not established.

## Build and runtime scope

- Serde's published build.rs invokes the configured rustc to inspect its version and writes a private compatibility module under OUT_DIR. The inspected script does not contain an installer or network download. This is nevertheless build-time code generation, requiring explicit approval under the repository's supply-chain skill.
- serde_json's inspected build.rs reads target architecture/pointer-width environment values and emits compiler configuration. Its published package also depends on serde_core and ordinary formatting/parser support crates.
- sha2 is a runtime SHA-256 library. Do not enable its optional asm/sha2-asm feature. The selected feature uses its software implementation; target-specific transitive dependencies are included in the closure review below. A disabled assembly feature does not by itself prove a build has no target-specific code.
- Do not enable Serde derive/procedural-macro features; use a manually implemented bounded visitor to preserve duplicate detection and decoding limits. Dependency build scripts are distinct from Serde derive and were covered by the owner approval.
- Decoder/hash calls stay local with no network, credentials, shell launch, plugin discovery, runtime download or customer-installed interpreter. These source observations do not grant filesystem/credential privileges to an unreviewed dependency.

## Authorization and execution boundary

Owner approval now covers adding/building the candidate libraries and reviewed transitive dependencies, including build scripts, and querying public dependency names/versions against RustSec/OSV. Do not upload private source, manifests, local reports or dependency graphs. Public-only OSV advisory queries occurred after approval; public package registry/documentation reads alone are not vulnerability checks.

The resolved closure was pinned and reviewed before first execution. Cargo.lock is aligned with manifests. No unfamiliar installer, Protobuf generator or private-data scanner was run. New dependency changes still require the same workflow.

## Resolved closure, inspected before execution

2026-11-14: all 21 public registry archive SHA-256 checksums matched Cargo.lock; none was yanked. Individual OSV API queries containing only public package name/version returned no matching advisories for all 21. This is a dated database observation, not proof of absence of vulnerabilities or compromise. Metadata repositories/authors and current crates.io owner accounts were inspected; historical account transfers were not independently established.

| Package | Published UTC | License | Repository | Current owners |
| --- | --- | --- | --- | --- |
| block-buffer 0.10.4 | 2023-03-09 | MIT OR Apache-2.0 | [source](https://github.com/RustCrypto/utils) | newpavlov, github:rustcrypto:utils |
| cfg-if 1.0.4 | 2025-10-15 | MIT OR Apache-2.0 | [source](https://github.com/rust-lang/cfg-if) | rust-lang-owner |
| cpufeatures 0.2.17 | 2025-01-25 | MIT OR Apache-2.0 | [source](https://github.com/RustCrypto/utils) | tarcieri, github:rustcrypto:utils |
| crypto-common 0.1.6 | 2022-07-16 | MIT OR Apache-2.0 | [source](https://github.com/RustCrypto/traits) | newpavlov, github:rustcrypto:traits |
| digest 0.10.7 | 2023-05-19 | MIT OR Apache-2.0 | [source](https://github.com/RustCrypto/traits) | newpavlov, github:rustcrypto:traits |
| generic-array 0.14.7 | 2023-03-28 | MIT | [source](https://github.com/fizyk20/generic-array.git) | fizyk20, novacrazy |
| itoa 1.0.15 | 2025-03-03 | MIT OR Apache-2.0 | [source](https://github.com/dtolnay/itoa) | dtolnay |
| libc 0.2.177 | 2025-10-09 | MIT OR Apache-2.0 | [source](https://github.com/rust-lang/libc) | rust-lang-owner |
| memchr 2.7.6 | 2025-09-25 | Unlicense OR MIT | [source](https://github.com/BurntSushi/memchr) | BurntSushi |
| proc-macro2 1.0.101 | 2025-08-16 | MIT OR Apache-2.0 | [source](https://github.com/dtolnay/proc-macro2) | dtolnay |
| quote 1.0.41 | 2025-09-29 | MIT OR Apache-2.0 | [source](https://github.com/dtolnay/quote) | dtolnay |
| ryu 1.0.20 | 2025-03-04 | Apache-2.0 OR BSL-1.0 | [source](https://github.com/dtolnay/ryu) | dtolnay |
| serde 1.0.228 | 2025-09-27 | MIT OR Apache-2.0 | [source](https://github.com/serde-rs/serde) | dtolnay, github:serde-rs:publish |
| serde_core 1.0.228 | 2025-09-27 | MIT OR Apache-2.0 | [source](https://github.com/serde-rs/serde) | dtolnay, github:serde-rs:publish |
| serde_derive 1.0.228 | 2025-09-27 | MIT OR Apache-2.0 | [source](https://github.com/serde-rs/serde) | dtolnay, github:serde-rs:publish |
| serde_json 1.0.145 | 2025-09-14 | MIT OR Apache-2.0 | [source](https://github.com/serde-rs/json) | dtolnay, github:serde-rs:publish |
| sha2 0.10.9 | 2025-04-30 | MIT OR Apache-2.0 | [source](https://github.com/RustCrypto/hashes) | tarcieri, newpavlov, github:rustcrypto:hashes |
| syn 2.0.106 | 2025-08-16 | MIT OR Apache-2.0 | [source](https://github.com/dtolnay/syn) | dtolnay |
| typenum 1.19.0 | 2025-10-02 | MIT OR Apache-2.0 | [source](https://github.com/paholg/typenum) | paholg |
| unicode-ident 1.0.19 | 2025-09-10 | (MIT OR Apache-2.0) AND Unicode-3.0 | [source](https://github.com/dtolnay/unicode-ident) | dtolnay |
| version_check 0.9.5 | 2024-07-25 | MIT/Apache-2.0 | [source](https://github.com/SergioBenitez/version_check) | SergioBenitez |

All selected releases are older than one year. Transitive versions were deliberately locked to established releases. Serde derive is disabled: proc-macro2, quote, syn, unicode-ident and serde_derive remain lockfile-only resolutions and must be distinguished from the active build graph. sha2 uses force-soft with no asm feature. Generic-array uses version_check to query rustc compatibility; Serde/serde_core write private compatibility modules in OUT_DIR; serde_json emits target cfg; typenum generates type-level test source in OUT_DIR. libc probes rustc/target configuration and may invoke emcc -dumpversion and (CI/FreeBSD configuration) freebsd-version. Proc-macro2 probes rustc and removes its own OUT_DIR probe subdirectory; quote probes compiler version; neither is enabled in this build. No inspected build script downloads binaries or runs an installer. Builds still execute trusted dependency code with local process privileges.

Sources: [crates.io API](https://crates.io/data-access), [OSV query API](https://google.github.io/osv.dev/api/), [RustSec advisory database](https://rustsec.org/advisories/). OSV results can include RustSec advisories; no separate exhaustive RustSec-history or malware scan is claimed. No private source, manifest, lockfile or local dependency graph was sent. Local compilation, feature graph and acceptance results are recorded at closure.

## Local checks and residual risk

`cargo fetch --locked` succeeded. The pinned Rust 1.99.0 macOS ARM64 toolchain then passed fmt/check/test/strict all-target Clippy with locked/offline dependency use: 56 behavioral tests, zero failed/ignored, zero doc tests. The legacy CLI demo, parameter example and real-file load_model example passed. `cargo tree --locked --offline -p fqe-ir -e features` showed 16 active registry packages, including target cpufeatures/libc; no serde derive/procedural-macro or sha2 asm feature. All 21 resolved registry packages, including five inactive lockfile-only macro packages, were reviewed. Cargo.lock changes are delivered together with fqe-ir's three direct dependency additions.

No additional local malware scanner was run or installed as part of this task; archive checksum checks and Rust checks are local machine evidence. OSV database coverage/lag, current-owner versus historical-ownership evidence and dependency code running with build-process privileges remain limitations. This is a dated scoped review, not a complete source audit or a guarantee against unknown vulnerabilities. Other targets/features need their own execution and compatibility evidence.
