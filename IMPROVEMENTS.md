# Engineering improvements for tuipr

Companion to [FEATURES.md](FEATURES.md) (what to build) and
[ARCHITECTURE.md](ARCHITECTURE.md) (how it is built). This file tracks how the
codebase, tooling and delivery should get better. It is not a bug list.

**Baseline (2026-09-30):** single crate, ~18 400 lines of Rust, 126 tests,
`clippy::pedantic` clean, no CI, no releases, README says "test".

Rule of thumb for everything below: **do the refactors when a feature touches
the code anyway, do the tooling now.** Nothing here is worth a refactor-only
PR except the items in *Now*.

Part B was assembled by reading [NVIDIA/OpenShell](https://github.com/NVIDIA/openshell)
(≈40 crates, agent-first Rust project with a ratatui TUI). What is borrowed is
their engineering hygiene, not their TUI: tuipr's component/store design is
already tighter than their `App` struct with 20 `pending_*` flags, and it
stays.

---

## Part A — from reviewing tuipr itself

### Now (cheap, no feature needed)

- [x] **Audit the remaining panics in non-test code.** *Done 2026-09-30.*
  Removed 12 of 16: the infallible `expect`s in `github/pagination.rs`,
  `github/cli.rs`, `bitbucket_dc/mod.rs` and `widgets/dialog.rs` were
  restructured so the impossible case cannot occur (or became a `FetchError`);
  `unreachable!` in `fetchers.rs` became an `InvalidInput` error, in
  `commands.rs` an empty arm, in `timeline.rs` a single match, in
  `pr_detail/mod.rs` a `return None`. **Four remain**, all action-routing type
  gaps: `app/mod.rs` (×2), `pr_detail/interactions.rs`, `pr_detail/mod.rs`.
  They disappear with *Split `Action`* below; do not patch them separately.
  The FEATURES.md "No-panic audit" (bad provider responses) is a different
  audit and is still open.
- [x] **Clean `target/`** (3.5 GB). *Done.* The README's *Develop* section
  notes that `cargo clean` is always safe.
- [x] **Real README.** *Done, one gap:* what it is, providers, install from
  source, usage, keys, config, develop. **Still missing: a screenshot or gif.**
  Record one against a real repo (e.g. with `vhs`); the snapshot test screens
  use placeholder data and are not representative. Install instructions are
  source-only until a release exists.

- [x] **Readable GraphQL.** *Done 2026-09-30.* Queries live in
  `providers/github/graphql.rs` as multi-line GraphQL with `<<name>>`
  placeholders (no doubled braces), and are compacted to one line when sent, so
  the wire format is unchanged; a test pins the compacted output to the
  single-line queries sent before. The long field selections are multi-line
  constants next to their use (`PR_FIELDS`, `THREAD_FIELDS`). **Possible next
  step:** move the templates into `.graphql` files pulled in with
  `include_str!`, which gives editors syntax highlighting and GraphQL
  tooling. Not done because it splits each query from the code that fills it.

### With the next feature that touches the area

- [ ] **Split `Action` into local and app-level.** Today one enum carries both
  component-local actions (`Detail`, `Diff`, `List`, `Search`, `Commits`,
  `Paste`, `HelpScroll`) and application effects (`Navigate`, `Refresh`,
  `Command`, `Loaded`, `PrLink`). `App::apply_inner` and `Ui::update` assert
  with `unreachable!` that the local ones were consumed. Make components
  return `Option<Effect>` where `Effect` holds only the app-level variants;
  local messages never reach the app. Removes five `unreachable!` and makes
  the contract visible in the signature. *Trigger:* first feature that adds an
  app-level action (agent handoff, run-a-review).
- [ ] **Keybinding table for the PR view.** `pr_detail/keys.rs` is 28
  top-level conditions in one function, each mixing key, tab, modifiers and PR
  state, with the help overlay maintained separately by hand. Replace with a
  slice of `Binding { key, mods, tabs, gate: fn(&DetailView, pr_id) -> bool,
  action, help }`; routing and help are then derived from one source, and
  `supports_action` in `view.rs` becomes the gate. *Trigger:* first feature
  that adds two or more keys to the PR view.
- [ ] **Provider trait instead of enum dispatch.** `providers/mod.rs` matches
  on `Provider` in every method; fine for two providers, four arms in fifteen
  methods with GitLab and Bitbucket Cloud. Move to `trait ProviderApi` + a
  `Box<dyn ProviderApi + Send + Sync>` (or keep the enum but implement it via
  the trait). Capabilities stay data. *Trigger:* third provider.
- [ ] **Render-context structs for diff/thread rendering.** Four
  `too_many_arguments` allowances in `diff_viewer/pane.rs` and
  `widgets/comment.rs`. Bundle the per-render inputs (theme, focus, width,
  anchors, queued comments, current user) into one borrowed struct.
  *Trigger:* AI-authored comment marker (adds one more argument otherwise).
- [ ] **Share thread assembly between providers.** `github/activities.rs`
  (161 lines) and `bitbucket_dc/activities.rs` (394 lines) both turn a flat
  event list into threads, replies and reactions. Move the assembly into
  `domain/` and have each provider produce flat `domain` events. *Trigger:*
  outdated-comment detection or review grouping, which need the same logic.
- [ ] **Suspend / resume for child processes.** Agent handoff and *open in
  `$EDITOR`* both need to hand the terminal to a child and take it back.
  OpenShell's `handle_shell_connect` is the reference sequence: cancel
  background refreshes → pause the input reader → `LeaveAlternateScreen` +
  `disable_raw_mode` → run the child with inherited stdio on
  `spawn_blocking` → `enable_raw_mode` + `EnterAlternateScreen` → `clear` and
  redraw → drain stale events → resume the reader → restart refreshes. tuipr
  uses crossterm's `EventStream`, which has no pause; drop and recreate it
  around the child, or gate it with an `AtomicBool` the way they do. Put it in
  `app/desktop.rs` next to browser/clipboard as `run_in_terminal(cmd)`.
  *Trigger:* send-to-agent or open-in-editor.
- [x] **Write down the blocking-I/O rule.** *Done: ARCHITECTURE.md "Rules for I/O and effects" and CLAUDE.md.*  GitHub spawns `gh`, Bitbucket uses
  `reqwest::blocking`, both via `spawn_blocking` with a 60 s deadline. That
  is a deliberate design; add it to ARCHITECTURE.md so agent handoff and
  run-a-review follow the same pattern instead of introducing async HTTP.

### Test coverage gaps

- [x] `domain/` has 3 tests; *Done: `domain/comment.rs` now has 10 tests (suggestion parsing edge cases, revision matching, resolution). Thread assembly has not moved there yet, so assembly tests wait for that refactor.*  `comment.rs` (109 lines of thread logic) deserves
  direct tests once thread assembly moves there.
- [x] No test drives the full `App::run` loop with a fake provider. *Done: no fake provider variant was needed; `src/app/flow_tests.rs` runs the real `App`, fetchers and provider against `FakeGh` and applies results as they return. Six tests: list load and in-flight deduplication, a failed refresh keeping stale data and then recovering, a failed first load, a comment followed by refetches of activity, list and mergeability, a failed comment with no refetch, and a second write ignored while one is pending. **Not covered:** `App::run` itself (it takes a real terminal type), and Bitbucket through the `App` (only at provider level).*  A
  `Provider::Fake(FakeConfig)` behind `#[cfg(test)]` would let the
  regression tests cover refresh, in-flight dedup and mutation-then-refetch
  end to end instead of via `App::apply` only.
- [ ] Snapshot tests (`tui/testdata/screens.txt`) cover 100×30 and 40×12 for
  the list and all five detail tabs, but with placeholder data: one PR, no
  diff, no threads. Add a snapshot with realistic content (a diff with an
  inline thread, several commits, a failing build) at both sizes, and one at
  80×24.

---

## Part B — borrowed from OpenShell

Ordered by value for a one-person project. Their multi-crate workspace,
Nix/mise toolchain, Helm charts, SBOMs and vouch system are *not* on this
list; see *Not borrowed* below.

### Now

- [x] **Stricter lint set, workspace style.** *Done.* `[lints.rust]` block
  plus `clippy::{all, pedantic, nursery}`; 153 new warnings fixed (62 elided
  lifetimes, 36 `const fn`, 19 `use_self`, 17 unnecessary qualifications, the
  rest small). Two nursery lints allowed by name with a reason:
  `option_if_let_else` and `redundant_pub_crate`. Original note: OpenShell enables
  `clippy::{all, pedantic, nursery}` plus `rust::{unsafe_code,
  rust_2018_idioms, trivial_casts, trivial_numeric_casts, unused_lifetimes,
  unused_qualifications}`, then allows the noisy ones by name. tuipr has
  pedantic only. Add the `[lints.rust]` block and try `nursery` at `warn`;
  keep `too_many_lines = "allow"`. Run with `-D warnings` in CI, not locally.
- [x] **`rust-toolchain.toml`** *Done (1.95.0, rustfmt + clippy +
  rust-analyzer; `rust-version = "1.95"`).* Pinning channel + `rustfmt`, `rust-analyzer`,
  and **`rust-version`** in `Cargo.toml`. Same build for everyone, and
  `cargo` refuses old toolchains with a clear message.
- [x] **CI on GitHub Actions** *Done, `.github/workflows/ci.yml`: fmt,
  clippy and test on ubuntu + macOS, cargo-deny; all actions pinned by SHA.
  First run on GitHub (2026-09-30) passed all six jobs, including the Linux
  tests.*
  Modeled on their `branch-checks.yml` but
  trimmed to four jobs: `cargo fmt --check`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo test --locked`, and `cargo deny
  check`. Matrix on `ubuntu-latest` + `macos-latest`. Pin action versions by
  SHA as they do. `CARGO_INCREMENTAL=0` and `concurrency.cancel-in-progress`.
- [x] **`cargo-deny` with `deny.toml`.** *Done, and it paid for itself: it
  found RUSTSEC-2026-0285 in `rustls` 0.23.40 (TLS 1.3 handshake), fixed by
  `cargo update -p rustls` to 0.23.45. Licenses: allow-list plus
  `CDLA-Permissive-2.0`; `colored` and `option-ext` are MPL-2.0 and are named
  exceptions. tuipr itself is MIT (2026-09-30, `LICENSE` and the `license` field).
  `publish = false` stays in `Cargo.toml` until the first release.* Advisories, license allow-list,
  `unknown-registry = "deny"`. Cheap, and it is the only thing that will tell
  you when `keyring` or `reqwest` pulls in something unwanted.
- [x] **Dependabot for GitHub Actions** *Done (weekly, plus monthly grouped
  Cargo updates).* (their config is five lines) and,
  optionally, for Cargo with a monthly cadence.
- [x] **`[profile.release] strip = true`** *Done.* and **`[profile.dev] debug = 1`**;
  the second one noticeably speeds up incremental builds.

### Soon

- [x] **Tag-driven release workflow.** *Done and dry-run on GitHub
  (2026-09-30): all four builds succeeded, the publish job was skipped as
  designed, and the Linux artifacts were downloaded and checked (checksums
  match, binaries are x86-64 and ARM aarch64 ELF).* `.github/workflows/release.yml`
  + `.github/scripts/package.sh`, described in RELEASING.md. **The publish
  step itself has still never run**; the first real tag is its test. Left out
  on purpose: Homebrew tap (needs its own repo), crates.io (remove
  `publish = false` first), macOS signing/notarization.
  Original note: On `v*.*.*`: build for
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`; attach tarballs + `sha256` to a GitHub
  Release; generate the changelog from commits. Then a Homebrew tap and
  `cargo install tuipr`. Their `release-tag.yml` computes versions once and
  fans out; copy the shape, not the size.
- [x] **OSC 52 clipboard.** *Done.* Helper first, OSC 52 as fallback, and
  OSC 52 first when `SSH_CONNECTION` or `SSH_TTY` is set (the helper would fill
  the remote machine's clipboard). Reported as "sent to terminal clipboard"
  because the terminal never confirms. Uses the `base64` crate; the escape
  goes to `/dev/tty`, not stdout. Original note: Their `clipboard.rs` writes
  `ESC ] 52 ; c ; <base64> BEL` straight to `/dev/tty`, so copy works over
  SSH, tmux and mosh without `pbcopy`/`xclip`/`wl-copy`. tuipr's
  `app/desktop.rs` shells out to platform helpers. Use OSC 52 first and keep
  the helpers as fallback (some terminals disable OSC 52 writes).
- [x] **Auto light/dark theme.** *Dropped (2026-09-30).* All five themes are
  dark, and `terminal` (terminal's own colours) already follows a light
  terminal, so detection would only choose between a dark palette and
  `terminal`. Revisit only if a light palette is added. Original note: They detect the terminal background with an
  OSC 11 query (`terminal-colorsaurus`) *before* entering raw mode, with
  `auto | dark | light` as the config value. tuipr's `theme = "…"` picks a
  named palette; add `auto` that maps to a light or dark default. Detection
  must run before `ratatui::init()`.
- [x] **Integration tests in `tests/`.** *Done for the CLI surface:*
  `tests/cli_integration.rs`, 9 tests, isolated HOME and git ceiling, no
  network. Not covered: config parsing (only reachable through the TUI) and
  any provider behaviour; those need the transport-mock work. Original note:
  Their crates keep unit tests inline
  and put subprocess-level tests in `tests/*_integration.rs` (for the CLI:
  `cli_help_integration.rs`, `cli_color_integration.rs`). The helper is
  small: `run_isolated(args)` runs `env!("CARGO_BIN_EXE_openshell")` with
  `HOME` and `XDG_CONFIG_HOME` pointed at a `tempfile::tempdir()`, stdin
  null, and returns `{stdout, combined, code}`. For tuipr: `tuipr --help`,
  `tuipr auth` without `gh`, config parsing from a temp `XDG_CONFIG_HOME`,
  all driven through the built binary. For in-crate tests that touch env
  vars they use an `EnvVarGuard` (restores on drop) behind a global
  `TEST_ENV_LOCK` mutex; tuipr's `config.rs` needs the same once it has tests.
- [ ] **`cargo nextest`** for the test run in CI (parallel, per-test timeouts,
  clearer failure output). Local `cargo test` stays fine.
- [ ] **Structured errors with context.** They use `thiserror` for library
  errors and `miette` for user-facing diagnostics. tuipr already has
  `FetchError::user_message()` which does the same job by hand; not worth
  switching, but the *split* between log message and user message is right
  and should be kept as new error variants are added.

### When the project has more than one contributor

- [ ] **`CONTRIBUTING.md`** with their one rule: *you must understand your
  code*, even (especially) when an agent wrote it. Plus how to run fmt / lint
  / test, and the blocking-I/O and two-views principles.
- [x] **`AGENTS.md` / `CLAUDE.md`.** *Done: `CLAUDE.md` holds the non-negotiables, commands and pointers.*  They keep one file as the primary
  instruction surface for coding agents and point it at ARCHITECTURE and
  CONTRIBUTING. tuipr's ARCHITECTURE.md already reads like one; a short
  `CLAUDE.md` that names the principles, the two-views rule, the I/O pattern
  and the test commands would make every agent session start on the same
  footing.
- [x] **A `tui-development` skill, or the same content in ARCHITECTURE.md.** *Done: ARCHITECTURE.md now has read and write lifecycles and checklists for a new provider write and a new read resource. No separate skill.* 
  Their internal skill for the TUI crate is the most useful agent doc in the
  repo. Worth copying the *shape*, not the text: a domain-object hierarchy;
  numbered "adding a new screen / event variant / RPC" checklists; one
  *lifecycle* section per async flow (start → flag → spawn → event → state →
  cancel) so an agent can see the whole path; a keybinding table per
  screen/focus; and UX conventions as rules ("destructive actions confirm",
  "truncate in list, full text in popup", "scrolling up pauses follow").
  ARCHITECTURE.md has the first and last already; the lifecycle sections and
  the add-a-thing checklists are missing.
- [ ] **PR template** with Summary / Changes / Testing, and a rule that
  user-visible changes update FEATURES.md.

### From their core crates (code, not tooling)

Read after the tooling pass: `openshell-isolation-interface`, `openshell-sdk`,
`openshell-policy`, `openshell-sandbox-backend`. tuipr is on par in most of
it; these are the places where their code is clearly better and the pattern
transfers.

- [ ] **Error classification for the caller.** Their `BackendError` keeps
  variant + message and adds `kind()` → `Invalid | Denied | Unavailable |
  Unsupported | Failed | Terminated`; `SdkError::Auth` carries
  `retryable: bool` and the original status boxed. tuipr's
  `FetchError::user_message()` covers the human text; add
  `FetchError::kind()` (`Retryable | NeedsAuth | Gone | Invalid | Unknown`)
  so the error dialog decides whether to offer *retry*, *re-login* or just
  *dismiss* from the classification instead of from prose in the message.
  *Trigger:* next change to the error dialog or a new provider.
- [ ] **Types that carry invariants.** `VerifiedBackendDescriptor` has no
  public constructor and is minted only by the registry; lifecycle states are
  consumed by value (`self: Box<Self>`) so a step cannot be skipped. tuipr
  already does this for `CommentAnchor` + `DiffRevision`. Extend it to the
  screen → app boundary: a `ResolvedCommand` that only
  `pr_detail/interactions.rs` can construct, so `app/commands.rs` never
  re-checks dialog state. Pairs with *Split `Action`* above.
- [x] **`//!` contract docs on the core modules.** *Done: added to `app/store.rs`, `app/reviews.rs`, `app/drafts.rs`, `providers/mod.rs` and `tui/component.rs`.*  Every module of theirs
  opens with what it guarantees and what it does not, often with a small
  state diagram. ARCHITECTURE.md has that content for tuipr but the modules
  are silent. Add `//!` blocks to `app/store.rs` (resource keys, in-flight
  dedup, reload-after-mutation), `app/reviews.rs` (anchors and revisions),
  `app/drafts.rs` (scope, atomic save, one writer), `providers/mod.rs`
  (capabilities, exactly-once caveats) and `tui/component.rs` (the
  contract). Keep ARCHITECTURE.md as the map; the modules hold the details.
- [ ] **Conformance test for the provider boundary.** Their
  `backend_conformance.rs` runs one scenario set against two deliberately
  different mock backends behind `dyn`, proving the abstraction holds with no
  enum over concrete types. Do the same when the provider trait lands: one
  `tests/provider_conformance.rs` over a fake GitHub and a fake Bitbucket,
  covering list → open → comment → review → merge and the error paths.
- [x] **Mock the transport, not just the payload.** *Done: `src/test_support.rs` has `FakeGh` (a scripted `gh` run under `/bin/sh`, with ordered rules, once-only rules and a call and stdin log) and `MockHttp` (a loopback server with exact-route matching, sequenced answers and a request log). `src/providers/transport_tests.rs` has 18 tests through `Provider`: GraphQL cursors, truncated label pages, exit codes and stderr, missing `gh`, malformed output, exact argument shapes for merge, close and comment (including shell metacharacters passed literally), the atomic GitHub batch on stdin, refusal of mixed-revision batches, Bitbucket offsets, token and user-agent headers, 401, 500, unreachable server, non-advancing cursor, and the non-atomic Bitbucket review including `PartialReview` counts. The fake replaces `gh` process-wide, so gh-using tests take a lock and run one at a time.*  Their SDK tests dial an
  in-process gRPC server whose `MockState` records what the mock observed and
  what it replied. tuipr's provider tests stop at JSON mapping; nothing
  exercises `run_gh` argument shapes, pagination loops or HTTP error bodies.
  Add a fake `gh` script on `PATH` for GitHub tests and a minimal HTTP mock
  (std `TcpListener` is enough) for Bitbucket DC. Ties into the *fake
  provider for `App::run`* gap in Part A.
- [ ] **`Pager<T>`.** A lazy pager built from a fetch closure and a token,
  with `next_page()` and `collect_all()`, tested with a counting closure.
  Cleaner than `github/pagination.rs` (219 lines) and the right shape for
  *Pagination / load more* in FEATURES.md.
- [ ] **Split large test modules by concern.** `src/runtime/tests/{flow_control,
  network_recovery,…}.rs` with `use super::*`. Apply to
  `tui/regression_tests.rs` when it passes ~500 lines.

What not to copy from these crates: the 40-line `pub use` facades bridging two
parallel type trees in `openshell-policy` (churn from a proto/schema split),
and the twenty-field `Mutex<Option<…>>` mock-state bags, which are the same
flag-bag pattern as their TUI `App`.

### Structure: crates, folders, files

A pass over the whole tree (38 crates, 521 Rust files). Their crate-level
structure is disciplined and worth copying in principle; their file-level
structure is worse than tuipr's and worth a written rule against.

**What they do well at crate level**

- **Role-first naming with variant suffixes.** `openshell-<role>` and
  `openshell-<role>-<variant>`: `-driver-docker`, `-driver-podman`,
  `-supervisor-network`, `-supervisor-process`, `-prover-cli`,
  `-otel-test-support`, `-server-macros`. The crate list reads as the
  architecture. If tuipr ever splits, name the pieces `tuipr-core`,
  `tuipr-provider-github`, `tuipr-provider-bitbucket-dc`, `tuipr-tui`, not
  `core`/`gh`/`bb`.
- **Four recurring crate kinds.** `-interface` (trait + types, no impl),
  `-schema` (dependency-light serde types and parsing), `-core` (shared
  helpers, 50 small files), `-test-support` (fixtures shared without
  dev-dependency cycles). Contracts never depend on implementations: server →
  interface ← driver, and the conformance test lives with the interface.
- **Per-crate README about the runtime model,** not the API: what gets
  created, what runs as non-root, which volume carries what. 13 of 38 crates
  have one. ARCHITECTURE.md already plays that role for tuipr.

- [ ] **Shape `providers/` as domain + trait + leaves.** When the provider
  trait lands: `domain/` and the trait in the middle, `github/` and
  `bitbucket_dc/` as leaves that depend on it, never on each other, and the
  conformance test next to the trait. Same as their server → interface ←
  driver rule, inside one crate.
- [x] **Shared test fixtures in one module.** *Done: `src/test_support.rs`, with the GitHub payload builders alongside the two doubles. `EnvVarGuard` is not needed yet.*  They keep `test_utils.rs` /
  `test_support.rs` per crate rather than `#[cfg(test)]` helpers scattered
  through files. tuipr has `tui/testdata/` for snapshots; add
  `src/test_support.rs` for the fake provider, `EnvVarGuard` and any
  transport mocks when those arrive (see *Mock the transport*).

**What they do badly at file level**

| File | Size |
|---|---|
| `server/src/grpc/policy.rs` | 835 KB |
| `server/src/grpc/provider.rs` | 575 KB |
| `server/src/compute/mod.rs` | 574 KB |
| `supervisor-network/src/proxy.rs` | 553 KB |
| `driver-docker/src/lib.rs` | 237 KB, 6 600 lines, 217 fns, tests in a sibling file |

39 files over 100 KB. `too_many_lines = "allow"` in the workspace lints, used
in full. It is culture, not scale: `driver-podman` does the same job as
`driver-docker` in eleven named files (`client`, `config`, `container`,
`driver`, `watcher`, `socket_discovery`). Three sibling-test conventions
coexist (`tests.rs`, `<module>_tests.rs`, `src/**/tests/*.rs`), and `mod.rs`
is often the main body rather than an index. The CLI's `main.rs` (242 KB) and
`run.rs` (271 KB) hold the command logic while `commands/` has four files.

tuipr is an order of magnitude smaller: its largest non-test files are
`widgets/comment.rs` (683 lines), `diff_viewer/pane.rs` (581), `pr_list/mod.rs`
(552) and `timeline.rs` (520), and two test files pass 1 000 lines
(`tui/regression_tests.rs` 1 308, `app/tests.rs` 1 154). `mod.rs` files
compose and named files hold the parts, and `cli.rs` dispatches to
`preflight` and `auth`. Keep those two habits; the four files above are
where the size rule is already broken.

- [x] **Write the file-size rule down.** *Done: the rule is in CLAUDE.md. `too_many_lines` stays `allow`. Four non-test files and two test files already exceed it (measured 2026-09-30, see above); split them when next touched substantially, not in a refactor-only change.*  In CONTRIBUTING (or CLAUDE.md until
  one exists): modules stay under ~500 lines, `mod.rs` composes and does not
  implement, tests for a module live inline or in exactly one sibling
  `tests.rs`, never both conventions. Consider `too_many_lines` at `warn`
  with a threshold instead of `allow`; it is cheaper to keep than to
  reintroduce.

### From their AI reviewer ("gator")

OpenShell runs an autonomous PR reviewer in a sandbox. None of its code is
reusable here, but the *contract* it enforces is exactly what tuipr will read
and display once AI reviews are first-class threads (FEATURES.md §2). Design
the domain model against it:

- [ ] **Marker-based AI detection, not just bot accounts.** Every gator
  comment starts with a first-line marker (`> **gator-agent**`); other skills
  use their own (`🔒 security-review-agent`). Detecting AI authorship needs a
  configurable list of first-line markers *and* account names, not one or the
  other.
- [ ] **One disposition per head SHA.** A review is one batched GitHub review
  (summary + inline comments) that names the head SHA it reviewed. tuipr
  should show *reviewed SHA vs. current head* on the AI summary line; a
  review of an older SHA is stale, not wrong.
- [ ] **Stable finding IDs across rounds.** Findings carry
  `GATOR-<sha8>-<nn>` and are carried, resolved or waived across later
  commits; a maintainer's "won't fix" reply is a waiver, an author's "fixed"
  is a claim to verify. The open/fixed/waived state per finding is what a
  reviewer wants on a glance, and it is derivable from thread resolution +
  resolver identity + the marker.
- [ ] **Severity and evidence.** Findings are `Critical | Warning |
  Suggestion`, and only ones with a full evidence record (base behavior, head
  behavior, observable impact, reproducer, changed location) count as
  blockers; the rest are "hypotheses". Suggestions never block. If tuipr's
  own *run a review* command emits structured output, use this split: it
  gives the reviewer a defensible "N blockers, M suggestions" header instead
  of a wall of comments.
- [ ] **Concern format for the review prompt.** Their `review-github-pr`
  skill requires every concern as "Before this PR, `<persona>` experienced
  `<old>`. With this PR, `<new>`, so `<impact>`." with file:line only as
  evidence. That is a good default prompt for tuipr's run-a-review.
- [ ] **Convergence rules worth copying into the display.** After three
  finding-bearing rounds the reviewer goes `critical_only`; rebase-equivalent
  patches (same patch-id) are not re-reviewed. Show the round count and
  "unchanged since last review" so a human knows when the AI has stopped
  adding value.

### Not borrowed (and why)

- **Cargo workspace with many crates.** tuipr is 18 k lines with clear
  module boundaries; a workspace adds compile-unit overhead and manifest
  churn without a consumer for the split crates. Revisit only if a `tuipr-core`
  becomes a dependency for something else (a Neovim plugin, an MCP server).
- **mise / Nix toolchain management.** A `rust-toolchain.toml` plus a
  `justfile` (or `Makefile`) with `fmt`, `lint`, `test`, `ci` targets covers
  a one-language project.
- **SPDX headers, CODEOWNERS, SECURITY.md, vouch system, DCO.** Corporate
  open-source process; nothing to gain until there are outside contributors.
- **Their TUI structure.** Mouse capture, a splash screen, 20 `pending_*`
  booleans polled after each key, ratatui 0.26. tuipr's typed
  `Action`/`Component` design with `ratatui::init()` is the better pattern.
- **OpenTelemetry / OCSF logging.** `tracing` + env-filter is enough for a
  local TUI.
- **The gator agent itself** (sandboxed reviewer, label state machine,
  ledger scripts). tuipr *displays* reviews; it does not run an autonomous
  reviewer. The contract above is what to read, not what to build.
