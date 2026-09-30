## What and why

<!--
Link the issue this closes ("Closes #123"). The title should be an imperative
sentence that reads as a line in release notes, e.g. "Show the review count in
the list".
-->

## Checks

- [ ] There is an issue, or this is a small fix (typo, obvious bug with a test)
- [ ] `cargo fmt --all`, `cargo clippy --locked --all-targets -- -D warnings`,
      `cargo test --locked` and `cargo deny check` pass
- [ ] Observable behaviour has a test, using `FakeGh` / `MockHttp`
- [ ] The README, `docs/KEYS.md` and `docs/ROADMAP.md` are updated if behaviour changed
- [ ] It follows the rules in `AGENTS.md` (two views, no async HTTP, no new
      `unwrap`/`expect`, modules under about 500 lines)
- [ ] I read my own diff and can explain every change
- [ ] An AI tool was involved in this change: yes / no
