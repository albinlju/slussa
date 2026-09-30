# What has not been verified, and how to check it off

Everything in tuipr is tested, but mostly against doubles: a scripted `gh`, a
loopback server that imitates Bitbucket, and fixtures. Those prove that the
code does what it was written to do. They cannot prove that the real services
behave the way the code assumes. This file lists each place where that gap
matters, what would close it, and who can close it.

**How to use it.** Do an item, tick its box and write the date and outcome on
the line under it. When an item fails, it becomes a bug or a known limit, and
the README says so. Items marked *you* need a person with the right access.
Items marked *me* I can do with read-only calls when told to.

Decisions and the release procedure are in [RELEASING.md](RELEASING.md).

## A. Release mechanics (you)

- [ ] **V1. Rehearse the release.** The publish job has never run.
  1. Set `version` in `Cargo.toml` to `0.1.0-pre.1`, commit, push.
  2. `git tag v0.1.0-pre.1 && git push origin v0.1.0-pre.1`.
  3. In Actions, watch **Release** until it finishes.
  4. Expect: a *prerelease* named `v0.1.0-pre.1` with four `.tar.gz` files, each
     with a `.sha256`, and one `SHA256SUMS`.
  5. Delete the release and the tag, and set the version back to `0.1.0`.
  Report: pass, or the failing step and its log.

- [ ] **V2. Clean install.** Download one archive from that prerelease on a
  machine *without Rust*, unpack it, run `./tuipr --version` and then `./tuipr`
  inside a repository. Note whether macOS blocks it and whether
  `xattr -d com.apple.quarantine tuipr` is enough. Check the checksum with
  `shasum -a 256 -c <file>.sha256`.
  Report: what happened, and the exact message if it was blocked.

- [ ] **V3. The Linux ARM binary runs.** It is cross-compiled and has never been
  executed. On a Mac with Docker:

  ```sh
  mkdir x && tar -xzf tuipr-0.1.0-pre.1-aarch64-unknown-linux-gnu.tar.gz -C x
  docker run --rm --platform linux/arm64 -v "$PWD/x":/t ubuntu:22.04 \
    /t/tuipr-0.1.0-pre.1-aarch64-unknown-linux-gnu/tuipr --version
  ```

  Expect `tuipr 0.1.0-pre.1`. Report: the output, or the error.

## B. GitHub against the real service

- [ ] **V4. A genuinely blocked PR.** The wording for `BLOCKED` is derived from
  `reviewDecision` and may be too general.
  1. In a test repository, add a ruleset on the default branch that requires one
     approving review.
  2. Open a PR and leave it unapproved.
  3. In tuipr open it, press `m`, and read the dialog; note the header badge.
  4. Repeat with a failing required check, and with the branch behind its base
     if "require branches to be up to date" is on.
  Report: the text of the merge dialog and the badge for each case.

- [ ] **V5. "Review requested" on a real PR.** This needs a PR where someone else
  requested *your* review, which one account cannot create alone.
  1. Ask a teammate, or use a second account, to open a PR and request your
     review (a person, not a team).
  2. Open tuipr as yourself.
  3. Expect: that PR first in the list, with `review requested` in the
     "Needs you" column at 90 columns or wider.
  4. Approve it; expect the reason to disappear after the next refresh (`F`).
  Report: whether it appeared, and where it sorted.

- [ ] **V6. Reopen on a real PR.** This changes a real PR and notifies people,
  so use a throwaway. PR #3 in `albinlju/tuipr` is closed and not merged.
  1. In the list press `f`, choose Declined, open the PR, go to the Overview.
  2. Press `x`, confirm. Expect `PR #3 · reopened` and the status to change
     after the refresh.
  3. Press `x` again and confirm the decline, to put it back.
  4. A refused reopen: close a PR, delete its head branch on GitHub, try to
     reopen it in tuipr. Expect GitHub's own message in the error dialog.
  Report: anything that looked wrong, and the message in step 4.

- [ ] **V7. A large repository.** The list used to read every PR ever opened.
  It now reads the open ones and a batch of closed ones. This has been checked
  against a tiny repository only.
  1. `git clone --depth 1 https://github.com/cli/cli && cd cli && tuipr`.
  2. Note how many seconds until the list appears, and whether `F` stays
     quick.
  3. Press `f`, choose Merged, press `L` a few times. Expect batches of 50 and
     a footer that drops `L: older` only when the history ends.
  Report: the seconds, and anything that stalled or looked wrong.
  (*Me*, instead: I can time the same queries with `gh api graphql`, read-only,
  against a public repository, at a cost of some of your rate limit. Say go.)

## C. Bitbucket Data Center

Every Bitbucket path is tested only against a mock written from Atlassian's
documentation. The customer's server is not something you can experiment on,
so there are three ways to close this, from least to most access. Pick the one
that fits, and until one is done the README should call Bitbucket support
*untested against a real server*.

1. **No access at all.** Keep the mock tests, say so in the README and in the
   release notes, and let the first real user report. Nothing to do here.

2. **A local Bitbucket Data Center.** The only way to test writes safely.
   Atlassian publishes a Docker image (`atlassian/bitbucket`) that runs on port
   7990 and needs a trial licence. Create a project and repository, push any
   repository to it, create a personal access token, open a few PRs, and point
   tuipr at it with `tuipr auth login`. This is an evening of setup, and I have
   not verified the current steps or licence terms.

3. **Read-only against the customer's server, without sharing their data.** The
   reads are safe: listing, opening, diffs, comments, merge status. I can write
   a small script that performs only `GET` requests and saves the responses to a
   local folder, and a test that runs tuipr's parsing over that folder and
   reports only pass or fail and which fields did not match, never the content.
   The folder stays on your machine. Writes (comment, approve, merge, decline,
   reopen) stay unverified. Ask the customer before pointing anything at their
   server, even read-only.

What I need to know in any case:

- [ ] **The Bitbucket Data Center version** the customer runs, and whether
  personal access tokens are enabled for the account tuipr would use.

What a real run has to confirm. These are the assumptions in the code:

- [ ] **B1. Authentication.** A personal access token as a `Bearer` header is
  accepted, and the `X-AUSERNAME` response header names the current user.
- [ ] **B2. Listing.** `state=OPEN`, `MERGED` and `DECLINED` with `limit` and
  `start`; the page fields `isLastPage` and `nextPageStart`; a limit of 50 and
  25 accepted and not capped lower.
- [ ] **B3. PR fields.** `draft`, `properties.commentCount`, and reviewer status
  values `APPROVED`, `UNAPPROVED`, `NEEDS_WORK`. Older versions may lack
  `draft`.
- [ ] **B4. Merge status.** `GET …/merge` returns `canMerge`, `conflicted` and
  `vetoes[].summaryMessage`, and a refused `POST …/merge?version=` reports its
  vetoes in `errors[].vetoes`.
- [ ] **B5. Decline and reopen.** Both take the PR `version`, and reopen is
  accepted for a declined PR.
- [ ] **B6. Comments.** Inline anchors (`fromHash`, `toHash`, `diffType`,
  `lineType`, `fileType`), replies, editing and deleting (which need the
  comment version), and resolving a thread.
- [ ] **B7. Reviews.** Approve, needs work and unapprove through
  `…/participants/{user}`.
- [ ] **B8. Diff, commits and builds.** The JSON diff shape including large
  diffs, the commit list, and build statuses.
- [ ] **B9. Activity.** The activity feed, including reactions under
  `properties.reactions`.
- [ ] **B10. Older PRs.** `L` continues merged and declined from their own
  offsets and ends correctly.

## D. Decisions and to-dos (you)

- [ ] **D1. Repository visibility.** While it is private, nobody else can
  download a release. Decide whether `0.1.0` makes it public.
- [ ] **D2. Screenshot or gif** in the README, recorded against a real
  repository.

## E. What I can do myself when told to

- Time the list queries on a large public repository (V7, read-only).
- Write and run the read-only Bitbucket probe described in C.3.
- Turn anything a real run reveals into a fix and a regression test.
