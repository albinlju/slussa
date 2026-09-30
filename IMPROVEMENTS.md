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

- [ ] **Audit the remaining panics in non-test code.** `unreachable!` in
  `app/mod.rs`, `app/commands.rs`, `pr_detail/interactions.rs`,
  `overview/timeline.rs`, `app/fetchers.rs`; `expect("piped …")` in
  `github/cli.rs`. Each is either a type-level gap (see *Split Action*) or
  should degrade to a logged no-op. Ties into the FEATURES.md "No-panic audit".
- [ ] **Clean `target/`** (3.5 GB) and add `cargo clean` guidance or a
  `CARGO_TARGET_DIR` note to CONTRIBUTING.
- [ ] **Real README.** What it is, a screenshot/gif, install, `tuipr auth`,
  keybindings summary, providers. FEATURES.md already lists *Release* as a
  backlog item; the README is the first half of it.

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
- [ ] **Write down the blocking-I/O rule.** GitHub spawns `gh`, Bitbucket uses
  `reqwest::blocking`, both via `spawn_blocking` with a 60 s deadline. That
  is a deliberate design; add it to ARCHITECTURE.md so agent handoff and
  run-a-review follow the same pattern instead of introducing async HTTP.

### Test coverage gaps

- [ ] `domain/` has 3 tests; `comment.rs` (109 lines of thread logic) deserves
  direct tests once thread assembly moves there.
- [ ] No test drives the full `App::run` loop with a fake provider. A
  `Provider::Fake(FakeConfig)` behind `#[cfg(test)]` would let the
  regression tests cover refresh, in-flight dedup and mutation-then-refetch
  end to end instead of via `App::apply` only.
- [ ] Snapshot tests (`tui/testdata/screens.txt`) cover the happy layouts;
  add narrow-terminal (≤ 80×24) snapshots since the principles promise it.

---

## Part B — borrowed from OpenShell

Ordered by value for a one-person project. Their multi-crate workspace,
Nix/mise toolchain, Helm charts, SBOMs and vouch system are *not* on this
list; see *Not borrowed* below.

### Now

- [ ] **Stricter lint set, workspace style.** OpenShell enables
  `clippy::{all, pedantic, nursery}` plus `rust::{unsafe_code,
  rust_2018_idioms, trivial_casts, trivial_numeric_casts, unused_lifetimes,
  unused_qualifications}`, then allows the noisy ones by name. tuipr has
  pedantic only. Add the `[lints.rust]` block and try `nursery` at `warn`;
  keep `too_many_lines = "allow"`. Run with `-D warnings` in CI, not locally.
- [ ] **`rust-toolchain.toml`** pinning channel + `rustfmt`, `rust-analyzer`,
  and **`rust-version`** in `Cargo.toml`. Same build for everyone, and
  `cargo` refuses old toolchains with a clear message.
- [ ] **CI on GitHub Actions**, modeled on their `branch-checks.yml` but
  trimmed to four jobs: `cargo fmt --check`, `cargo clippy --locked
  --all-targets -- -D warnings`, `cargo test --locked`, and `cargo deny
  check`. Matrix on `ubuntu-latest` + `macos-latest`. Pin action versions by
  SHA as they do. `CARGO_INCREMENTAL=0` and `concurrency.cancel-in-progress`.
- [ ] **`cargo-deny` with `deny.toml`.** Advisories, license allow-list,
  `unknown-registry = "deny"`. Cheap, and it is the only thing that will tell
  you when `keyring` or `reqwest` pulls in something unwanted.
- [ ] **Dependabot for GitHub Actions** (their config is five lines) and,
  optionally, for Cargo with a monthly cadence.
- [ ] **`[profile.release] strip = true`** and **`[profile.dev] debug = 1`**;
  the second one noticeably speeds up incremental builds.

### Soon

- [ ] **Tag-driven release workflow.** On `v*.*.*`: build for
  `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`; attach tarballs + `sha256` to a GitHub
  Release; generate the changelog from commits. Then a Homebrew tap and
  `cargo install tuipr`. Their `release-tag.yml` computes versions once and
  fans out; copy the shape, not the size.
- [ ] **OSC 52 clipboard.** Their `clipboard.rs` writes
  `ESC ] 52 ; c ; <base64> BEL` straight to `/dev/tty`, so copy works over
  SSH, tmux and mosh without `pbcopy`/`xclip`/`wl-copy`. tuipr's
  `app/desktop.rs` shells out to platform helpers. Use OSC 52 first and keep
  the helpers as fallback (some terminals disable OSC 52 writes).
- [ ] **Auto light/dark theme.** They detect the terminal background with an
  OSC 11 query (`terminal-colorsaurus`) *before* entering raw mode, with
  `auto | dark | light` as the config value. tuipr's `theme = "…"` picks a
  named palette; add `auto` that maps to a light or dark default. Detection
  must run before `ratatui::init()`.
- [ ] **Integration tests in `tests/`.** Their crates keep unit tests inline
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
- [ ] **`AGENTS.md` / `CLAUDE.md`.** They keep one file as the primary
  instruction surface for coding agents and point it at ARCHITECTURE and
  CONTRIBUTING. tuipr's ARCHITECTURE.md already reads like one; a short
  `CLAUDE.md` that names the principles, the two-views rule, the I/O pattern
  and the test commands would make every agent session start on the same
  footing.
- [ ] **A `tui-development` skill, or the same content in ARCHITECTURE.md.**
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
