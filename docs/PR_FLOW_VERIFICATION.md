# GitHub flow verification

A live run on 2026-09-12 against the public repository
[albinlju/prtest](https://github.com/albinlju/prtest), with the authenticated
`albinlju` account. The actions were performed through the compiled TUI in a
PTY (it was called tuipr then), and GitHub API reads independently confirmed the
resulting writes. Bitbucket Data Center was not live-verified and, being in
maintenance, will not be; every Bitbucket write is tested against a mock of the
documented API.

## Results

| Flow | Result |
| --- | --- |
| Read | Passed: PR list, description, overview, diff, commits and Builds |
| CI | Passed: actual checks shown with success state and duration |
| Pagination | Passed: PR #4 loaded and opened commit 101/101; large comment/thread collections remain locally tested only |
| PR comment | Passed: create and edit; GitHub returned the edited text |
| Inline comment | Passed: posted to `arithmetic.py:2` on the reviewed HEAD |
| Older commit | Passed: the server stored the older SHA; its thread displays only with that revision |
| Reply | Passed: stored under the intended inline comment |
| Delete | Passed: cancel retained the disposable comment; confirm removed it |
| Review | Passed: a queued line comment and summary submitted as a COMMENTED review |
| Own-PR gate | Passed: approve and request changes are disabled in the picker |
| Approve / request changes | Needs a PR authored by a different account |
| Resolve | Passed: thread resolved, collapsed, then reopened |
| Merge | Passed: PR #2 squash-merged; the server confirms MERGED |
| Close | Passed: cancel then confirm on PR #3; the server confirms CLOSED |
| Protected or conflicting merge | Checked later, on 2026-09-30, against a real ruleset: a missing approval, a failing required check and a branch behind its base all show `blocked` with the server's reason in the merge dialog |
| Reopen | Checked on 2026-09-30 on a real closed PR, including GitHub's refusal when another PR is already open for the branch |
| Partial review, timeout and navigation during writes | Local regression tests; no live fault injection |

## Evidence

The comment and review texts on GitHub still begin `tuipr-live:`, the name the
tool had when they were written.

- [PR #1: comments and reviews](https://github.com/albinlju/prtest/pull/1)
  remains open.
- [Edited PR comment](https://github.com/albinlju/prtest/pull/1#issuecomment-5645638707):
  `tuipr-live: PR comment original (edited)`.
- [Inline thread](https://github.com/albinlju/prtest/pull/1#discussion_r3996098597):
  reply id `3996109995` has `in_reply_to_id: 3996098597`.
- [Comment review](https://github.com/albinlju/prtest/pull/1#pullrequestreview-5186327133):
  state `COMMENTED`, body `tuipr-live: comment review summary`, with queued line 8.
- [Older-commit comment](https://github.com/albinlju/prtest/pull/1#discussion_r3996112676):
  the server `commit_id` is `674510f6836e13519a95842e18c8830e0554e89a`, whereas
  PR HEAD was `2a3f69c79e4e43c547d0779b07a7265a93222ecf`. GitHub marks its
  current line null (outdated); the original commit keeps its code location.
- [PR #2: merge](https://github.com/albinlju/prtest/pull/2): merged at
  `2026-09-12T11:50:00Z`, merge commit `53b3eb36fba6dafeb1d5a092e665cdfa7bb1d012`.
- [PR #3: close](https://github.com/albinlju/prtest/pull/3): closed at
  `2026-09-12T11:50:33Z`.
- [PR #4: pagination](https://github.com/albinlju/prtest/pull/4) remains open;
  the tool displayed `101/101`, opened `d8852b6` (`Pagination fixture 101/101`)
  and rendered the change from test commit 100 to 101.

## Defects found and fixed during the run

1. Rapid keyboard input could open the previous PR selection: input is now
   applied before the next key is translated, instead of queueing local input
   alongside asynchronous provider results.
2. `r` in the diff pane advertised replying but did not open an editor: reply
   routing now uses the focused diff thread.
3. An older-commit thread could show a code excerpt from the current PR diff:
   thread revisions are now retained and checked before rendering code context
   or placing a thread inside a diff.

## Limits

Quitting does not undo an in-flight server write. A lost acknowledgement cannot
establish whether the last attempted post succeeded, so check the PR before
retrying. GitHub batches must use one diff revision; comments from different
revisions need separate batches. Local tests do not establish account
permissions or server-version compatibility.
