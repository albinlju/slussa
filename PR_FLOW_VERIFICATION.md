# PR flow verification

GitHub live run: 2026-09-12, using public repository
[albinlju/prtest](https://github.com/albinlju/prtest) and the authenticated
`albinlju` account. Actions were performed through the compiled tuipr TUI in a
PTY. GitHub API reads independently confirmed the resulting writes.
Bitbucket has not been live-verified.

## Results

| Flow | GitHub result | Bitbucket |
| --- | --- | --- |
| Read | Passed: PR list, description, overview, diff, commits and Builds | Pending |
| CI | Passed: actual checks shown with success state and duration | Pending |
| Pagination | Passed: PR #4 loaded and opened commit 101/101; large comment/thread collections remain locally tested only | Pending |
| PR comment | Passed: create and edit; GitHub returned the edited text | Pending |
| Inline comment | Passed: posted to arithmetic.py:2 on the reviewed HEAD | Pending |
| Older commit | Passed: server stored the older SHA; its thread displays only with that revision | Pending |
| Reply | Passed: reply stored under the intended inline comment | Pending |
| Delete | Passed: cancel retained the disposable comment; confirm removed it | Pending |
| Review | Passed: queued line comment and summary submitted as a COMMENTED review | Pending |
| Own-PR gate | Passed: approve/request changes disabled in the picker | Pending |
| Approve/request changes | Successful submission needs a PR authored by a different account | Pending |
| Resolve | Passed: thread resolved, collapsed, then reopened | Pending |
| Merge | Passed: PR #2 squash-merged through tuipr; server confirms MERGED | Pending |
| Close | Passed: cancel then confirm on PR #3; server confirms CLOSED | Pending |
| Protected/conflicting merge | Not exercised live | Pending |
| Partial review, timeout and navigation during writes | Local regression tests; no live fault injection | Pending |

## Reproducible fixtures and evidence

- [PR #1: comments and reviews](https://github.com/albinlju/prtest/pull/1) remains open.
- [Edited PR comment](https://github.com/albinlju/prtest/pull/1#issuecomment-5645638707):
  `tuipr-live: PR comment original (edited)`.
- [Inline thread](https://github.com/albinlju/prtest/pull/1#discussion_r3996098597):
  reply id `3996109995` has `in_reply_to_id: 3996098597`.
- [Comment review](https://github.com/albinlju/prtest/pull/1#pullrequestreview-5186327133):
  state `COMMENTED`, body `tuipr-live: comment review summary`, with queued line 8.
- [Older-commit comment](https://github.com/albinlju/prtest/pull/1#discussion_r3996112676):
  server `commit_id` is `674510f6836e13519a95842e18c8830e0554e89a`, whereas PR
  HEAD was `2a3f69c79e4e43c547d0779b07a7265a93222ecf`. GitHub marks its current
  line null (outdated); the original commit retains its code location.
- [PR #2: merge](https://github.com/albinlju/prtest/pull/2): merged at
  `2026-09-12T11:50:00Z`, merge commit `53b3eb36fba6dafeb1d5a092e665cdfa7bb1d012`.
- [PR #3: close](https://github.com/albinlju/prtest/pull/3): closed at
  `2026-09-12T11:50:33Z`.
- [PR #4: pagination](https://github.com/albinlju/prtest/pull/4) remains open.
  tuipr displayed `101/101`, opened `d8852b6` (`Pagination fixture 101/101`),
  and rendered the change from test commit 100 to 101.

Local fixture checkout: `/private/tmp/tuipr-prtest-20260912`.
Run from the tuipr source checkout:

```sh
cargo run --locked -- -C /private/tmp/tuipr-prtest-20260912
```

## Defects found and fixed during the live run

1. Rapid keyboard input could open the previous PR selection: input is now
   applied before the next key is translated, instead of queueing local input
   alongside asynchronous provider results.
2. `r` in the diff pane advertised replying but did not open an editor:
   reply routing now uses the focused diff thread.
3. An older-commit thread could display a code excerpt from the current PR
   diff: thread revisions are now retained and checked before rendering code
   context or placing a thread inside a diff.

Local verification after the fixes: **67 tests passed**, and
`cargo clippy --all-targets --locked -- -D warnings` passed. Existing screen
snapshots still pass.

## Remaining limits

Drafts are in memory; quitting loses them. Quitting does not undo an in-flight
server write. A lost acknowledgement cannot establish whether the last
attempted post succeeded, so check before retrying. GitHub batches must use one
diff revision; comments from different revisions need separate batches.

A Bitbucket test repository and authorization to create disposable PRs and
perform comment/review/resolve/merge/decline actions are still required for the
second provider's live run. Local tests do not establish server-version
compatibility or account permissions.

## Subsequent capability checks

After the live run, provider support was centralized in `Capabilities`. Local
regressions verify hidden controls/help/build details, blocked shortcuts and
commands, skipped optional loads, tab navigation, own-PR restrictions, and
different GitHub/Bitbucket review and merge capabilities. Full-feature screen
snapshots still match. These checks do not constitute a new live run or
Bitbucket server-version/permission verification.
