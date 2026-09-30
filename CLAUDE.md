# tuipr

A terminal UI for pull requests (Rust, ratatui). GitHub through the `gh` CLI,
Bitbucket Data Center through REST. The product is the human approval surface
for AI-generated PRs: triage, check intent, approve or merge.

## Non-negotiables

- **Two views only: the PR list and the PR.** New capability appears as a
  better default, a column, a marker or one key inside those views. Never a new
  screen, dashboard or sidebar. A feature that cannot be explained in one
  sentence and reached in one keypress is not ready.
- **Provider and process calls block and run off the UI thread**, through
  `App::spawn_fetch`. No async HTTP, no ad hoc threads, nothing started from
  rendering or key handling.
- **No new panics in non-test code.** No `unwrap`, `expect` or `unreachable!`.
  Return an error, or restructure so the case cannot occur.
- **Modules stay under about 500 lines.** `mod.rs` composes and does not
  implement. Tests live inline, or in exactly one sibling `tests.rs`; suites that drive several modules through the doubles in `src/test_support.rs` are named `*_tests.rs`. A few
  files already exceed the limit (`widgets/comment.rs`, `diff_viewer/pane.rs`,
  `pr_list/mod.rs`, `timeline.rs` and the two big test files); split one when
  you next change it substantially, and do not make them bigger.
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
`rust-toolchain.toml`.

## Where to look

- `docs/ARCHITECTURE.md`: code map, state ownership, read and write lifecycles, and
  checklists for adding a provider write or a read resource. Read it before
  changing app-level code.
- `docs/FEATURES.md`: what exists and the priority order for what comes next.
- `docs/IMPROVEMENTS.md`: engineering backlog; each refactor has a trigger, so do
  not do them ahead of the feature that needs them.
- `docs/RELEASING.md`: how a release is cut.
- `docs/VERIFICATION.md`: what has only been tested against doubles, and how to
  check it against the real service. Keep it current when a double stands in
  for something new.

## Working here

- Repo docs are in English. Update `docs/FEATURES.md` when behaviour changes.
- Add a regression test for observable behaviour, especially when navigation or
  asynchronous state is involved. Tests never call a real provider: use
  `FakeGh` and `MockHttp` from `src/test_support.rs`. A test that installs
  `FakeGh` holds a process-wide lock, so never install two in one test without
  dropping the first.
- Commit and push only when asked.
