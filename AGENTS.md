# slussa

A terminal UI for pull requests (Rust, ratatui). GitHub through the `gh` CLI,
Bitbucket Data Center through REST. The aim is to be the human approval surface
for AI-generated PRs: triage, check intent, approve or merge.

## Non-negotiables

- **Two views only: the PR list and the PR.** New capability appears as a
  better default, a column, a marker or one key inside those views. Never a new
  screen, dashboard or sidebar. A feature that cannot be explained in one
  sentence and reached in one keypress is not ready. Agent-facing subcommands
  are not views; they print and exit.
- **Provider and process calls block and run off the UI thread**, through
  `App::spawn_fetch`. No async HTTP, no ad hoc threads, nothing started from
  rendering or key handling. A headless subcommand has no UI thread and may call
  the provider directly, one call after another and outside the Tokio runtime.
- **No new panics in non-test code.** No `unwrap`, `expect`, `panic!`,
  `unreachable!` or indexing that can go out of bounds; clippy refuses them.
  Return an error, or restructure so the case cannot occur. An exception is an
  `#[expect(lint, reason = "...")]`, never an `#[allow]`: an `expect` that is
  no longer needed is reported, an `allow` lingers.
- **Types before runtime checks.** A state that should not exist is made
  impossible to write, not guarded where it is used. Reach for, in this order:
  an enum with the data in the variant it belongs to, instead of a struct with
  a flag and optional fields; a newtype with one constructor, for a value with
  a rule (`Username`, `NonBlank`, `WebUrl`); a ticket or a constructor that
  requires its data, instead of a call order to remember (`FetchTicket`,
  `WriteTicket`, `Session`). A `match` on one of our own enums names every
  variant; `_ =>` is for foreign enums such as `KeyCode` and for strings from a
  server. Clippy refuses the catch-all in `app`, `domain` and `providers`.
  ARCHITECTURE.md (*Types that carry the rules*) lists what exists.
- **Modules stay under about 500 lines.** `mod.rs` composes and does not
  implement. Tests live inline, or in exactly one sibling `tests.rs`; suites that drive several modules through the doubles in `src/test_support.rs` are named `*_tests.rs`. A test
  suite that outgrows the limit becomes a directory with a file per concern and
  a `support.rs` for what they share (`src/app/tests/`). Split a file by
  concern when it passes the limit, not by trimming it to fit;
  `tests/repo_rules.rs` is the stop behind the rule and fails a file over 600
  lines.
- **Show only what the provider supports.** Hide unsupported actions; keep
  actions blocked by PR state visible with a reason.

## Commands

```sh
cargo fmt --all
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo deny check
```

CI runs all four on Linux and macOS. The toolchain is pinned in
`rust-toolchain.toml`; `rust-version` in `Cargo.toml` is the oldest compiler
supported, and CI builds with that one too. Do not use a newer standard
library API than `rust-version` allows.

## Where to look

- `docs/ARCHITECTURE.md`: code map, state ownership, read and write lifecycles, and
  checklists for adding a provider write or a read resource. Read it before
  changing app-level code.
- `docs/ROADMAP.md`: the positioning, what to build next in priority order, and
  the engineering backlog; each refactor has a trigger, so do not do them ahead
  of the feature that needs them.

## Working here

- Repo docs are in English. Update the README, `docs/KEYS.md` (keys) and the matching item in
  `docs/ROADMAP.md` when behaviour changes. The help tables, the themes and
  the sorts each have a test that fails until `docs/KEYS.md` or the README's
  config example names the same ones (`src/doc_contract.rs`).
- Add a regression test for observable behaviour, especially when navigation or
  asynchronous state is involved. Tests never call a real provider: use
  `FakeGh` and `MockHttp` from `src/test_support.rs`; a test build has no
  `gh` at all unless a fake is installed. A test that installs
  `FakeGh` holds a process-wide lock, so never install two in one test without
  dropping the first. Say in the PR when a double stands in for real behaviour
  you could not check.
- The draft file must stay readable. The types in `src/app/reviews.rs`, with
  `CommentAnchor` and `DiffRevision`, are written to it, and a file that cannot
  be parsed stops slussa from starting. Change how they are spelled on disk
  only with a new file version; the version 1 fixture in `src/app/drafts.rs`
  fails otherwise.
- The log holds states, resource keys, counts and error text, the server's
  included. It never holds the content of a PR (a title, a description, a
  comment, a diff) and never a token: an answer that could not be read is
  logged by its size and where reading stopped, not by a sample of it. The log
  file is for its owner only, as the drafts are (`src/private_file.rs`).
- Commit and push only when asked.
