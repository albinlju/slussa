# Contributing to slussa

Contributions are welcome, with a few rules that keep the project small and
reviewable. The short version: **open an issue first**, follow the rules in
[CLAUDE.md](CLAUDE.md), and be able to explain every line you submit.

## Before you start

- **Open an issue first** for anything that is not a small fix. Say what you
  want to change and why; wait for a reply before you spend time on it. A pull
  request without an agreed issue may be closed without review.
- **Small fixes need no issue:** a typo, an obvious bug with a test that shows
  it.
- **Read what slussa is for.** [docs/ROADMAP.md](docs/ROADMAP.md) has the
  positioning and a list of what it rules out. The main one: there are two
  views, the PR list and the PR. A new capability appears as a better default,
  a column, a marker or one key inside those views, never as a new screen,
  dashboard or sidebar.
- **Bitbucket Data Center is in maintenance.** That is the self-hosted
  Bitbucket that Atlassian is retiring, not Bitbucket Cloud (which is not
  supported yet). Bug fixes are welcome; new features for it are not planned
  (see the README).

## How to contribute

You cannot push branches to this repository, so work from a fork:

```sh
gh repo fork albinlju/slussa --clone   # or fork on GitHub and clone your fork
cd slussa
git switch -c fix/short-description
```

Make the change, then open a pull request against `main` from your fork.

## The rules

1. **One thing per pull request.** The title is an imperative sentence that
   reads as a line in release notes: "Show the review count in the list".
2. **Follow [CLAUDE.md](CLAUDE.md).** It applies to people as much as to
   agents: two views only; provider and process calls block and run off the UI
   thread through `App::spawn_fetch`, with no async HTTP and no ad hoc
   threads; no new `unwrap`, `expect` or `unreachable!` in non-test code;
   modules stay under about 500 lines; show only what the provider supports.
3. **Run the four checks before you push.** CI runs the same ones on Linux and
   macOS.

   ```sh
   cargo fmt --all
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked
   cargo deny check          # cargo install cargo-deny --locked
   ```
4. **Add a test for behaviour you can observe**, especially for navigation and
   asynchronous state. Tests never call a real service: use `FakeGh` and
   `MockHttp` from `src/test_support.rs`.
5. **Update the README** (keys, usage) **and the matching item in
   [docs/ROADMAP.md](docs/ROADMAP.md)** when behaviour changes, and
   say in the pull request when a test double stands in for behaviour you could
   not check against the real service. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before
   you change app-level code; it has checklists for adding a provider write or
   a read resource.
6. **You must understand your code**, including code an AI tool wrote. Say in
   the pull request if an agent was involved. You are responsible for the
   change and for answering review comments yourself.
7. **Do not commit secrets, tokens or real host names.** Use made-up names in
   tests and docs.

## Reporting bugs and asking for features

Use the issue templates. For a bug, include `slussa --version`, the provider
(GitHub or Bitbucket Data Center), your operating system and terminal, and the
steps that show it. Security problems go through
[SECURITY.md](.github/SECURITY.md), not a public issue.

## Who decides

This is a one-person project. Reviews happen when there is time, and not every
proposal will be accepted; a no is about the project's scope, not about you.
