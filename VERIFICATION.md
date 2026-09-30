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

- [x] **V1. Rehearse the release.** The publish job has never run. Do it from
  a throwaway branch, so `main` keeps `0.1.0`, nothing needs reverting, and the
  repository can stay private: a prerelease in a private repository is visible
  only to people with access.
  1. `git switch -c release-rehearsal`, set `version` in `Cargo.toml` to
     `0.1.0-pre.1`, run `cargo check` so `Cargo.lock` follows (the release
     builds with `--locked`), commit.
  2. `git push origin release-rehearsal`, then tag that commit and push the
     tag: `git tag v0.1.0-pre.1 && git push origin v0.1.0-pre.1`.
  3. In Actions, watch **Release** until it finishes. The first job fails at
     once if the tag and `Cargo.toml` disagree.
  4. Expect: a *prerelease* named `v0.1.0-pre.1` with four `.tar.gz` files, each
     with a `.sha256`, and one `SHA256SUMS`.
  5. Check the download:
     `gh release download v0.1.0-pre.1 -D rel && (cd rel && shasum -a 256 -c SHA256SUMS)`.
  6. Clean up:
     `gh release delete v0.1.0-pre.1 --cleanup-tag --yes`,
     `git push origin --delete release-rehearsal`,
     `git branch -D release-rehearsal`.
  Report: pass, or the failing step and its log.
  **Done 2026-09-30, pass.** Run for tag `v0.1.0-pre.1` from branch
  `release-rehearsal`: all six jobs succeeded, including Publish. The release
  is a prerelease (not a draft) with four archives and `SHA256SUMS`. All four
  checksums verified, and each binary has the right architecture: arm64 and
  x86_64 Mach-O for macOS, ARM aarch64 and x86-64 ELF for Linux. Each archive
  holds `tuipr`, `LICENSE` and `README.md`. The Mac arm64 binary ran and printed
  `tuipr 0.1.0-pre.1` (maintainer), and the x86_64 Mac binary did the same under
  Rosetta. Cleaned up the same day: release, tag and branch are deleted and
  `main` is untouched at `0.1.0`.
  Deleting it removed the archives V2 and V3 use, so they need a release to
  download from: repeat V1 to get one, or do them on the real `0.1.0`.

- [ ] **V2. Clean install.** Use an archive from that prerelease on a
  machine *without Rust*, unpack it, run `./tuipr --version` and then `./tuipr`
  inside a repository. Note whether macOS blocks it and whether
  `xattr -d com.apple.quarantine tuipr` is enough. Check the checksum with
  `shasum -a 256 -c <file>.sha256`.
  Report: what happened, and the exact message if it was blocked.
  **Partly done 2026-09-30.** The Mac arm64 archive, fetched with `gh`, unpacked
  and ran. Still open: a machine without Rust, and above all the quarantine
  question, which only arises for a file downloaded in a browser (`gh` does not
  set the flag). Download an archive from the release page in a browser, then
  run `xattr -l tuipr` and `./tuipr --version`.

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
  3. Press `f`, choose Merged, press `L` a few times. Expect batches of 30 on GitHub and
     a footer that drops `L: older` only when the history ends.
  Report: the seconds, and anything that stalled or looked wrong.
  **Measured by me 2026-09-30 on `cli/cli`** (63 open, 3 256 merged, 1 352
  closed; read-only `gh api graphql` with tuipr's real selection, 51
  rate-limit points in all, of 5 000 an hour). The interactive part above is still yours.

  | Query | Time | Cost |
  | --- | --- | --- |
  | open PRs, first 100 (63 exist) | 5.8 s | 4 |
  | closed PRs, first 50 by update | 4.6 s | 2 |
  | closed PRs, first 25 | 2.9 s | 1 |
  | any state, first 100 (how it read before) | 11.2 s **failed** | |
  | any state, first 100, repeated | 6.9 s and 8.0 s passed | 4 |
  | any state, first 50 | 2.5 to 6.2 s | 2 |
  | any state, first 30 | 2.0 s | 1 |

  What it shows: the new first load costs 6 points and about 10 s end to end
  (the two reads run one after the other), where the old way needed 47 pages and
  could not even get its first page through. GitHub answered "We couldn't
  respond to your request in time" for a 100-PR page; the timing is noisy, but
  100 is at the edge and 30 to 50 is comfortably under. So the bounded list is a
  correctness fix on a big repository, not only a speed-up, and a page of 100
  is still too large for tuipr's selection. See IMPROVEMENTS.md.

  **Re-measured after the fix, same day, with tuipr's own code** (`GH_REPO=cli/cli`,
  a temporary test, not kept): a page size of 30 and the open and closed reads
  run together. The first load took 6.5 s for 93 PRs (63 open of which 30 are
  drafts, plus a page of closed ones), against about 10 s before, and the next
  three older batches took 2.3, 3.4 and 4.5 s. Nothing failed. Still the
  maintainer's: running the TUI itself and pressing `L` (steps above).

  **Changed afterwards:** merged and declined PRs are read per view, only when
  that view is first opened. The start now reads the open group alone. This has
  been tested against the scripted `gh` but not yet timed on a real repository.

  **Changed again, same day:** the list query no longer reads the body and the
  labels (read per PR when it is opened), and the open group is read a page at
  a time with the first page shown at once. Measured live on `cli/cli`, three
  runs of one page of 30 open PRs as now queried: 2.7, 2.2 and 2.0 s. The
  maintainer's part: open a PR on a real repository and check that the
  description and labels appear, and that the list appears with its first page.

## C. Bitbucket Data Center

Every Bitbucket path is tested only against a mock written from Atlassian's
documentation. The customer's server cannot be used freely, so the real test is
a **local Bitbucket Data Center in Docker**. It is the only way to exercise the
writes (comment, review, merge, decline, reopen) safely. A run against the
customer's server, read-only and only with their consent, can add to it later.

Until the checks below pass, the README says that only listing has been seen
working on a real server and everything else is tested against a mock.

### Reported so far

- **2026-09-30, customer's Bitbucket Data Center, reported by the maintainer:**
  tuipr was run against a real repository there and lists its pull requests.
  The version of that server and which tabs and actions were tried were not
  recorded. This is real evidence for connecting, authenticating with a token
  and reading the list, and for the remote being recognised. It is not evidence
  for any write. Fill in below what else was exercised.
  - Server version: **9.4.23**, from `version` in
    `https://<host>/rest/api/1.0/application-properties`, the same
    unauthenticated call tuipr makes to recognise a Bitbucket host.
    How current that is, from Atlassian's release notes and Docker Hub on
    2026-09-30: the latest is 10.5 (29 September 2026), and 9.6 (18 March 2025)
    and 10.0 (8 September 2025) came after the 9.4 line. 9.4.24 exists, so the
    server is one patch behind on its line. Atlassian's support policy for 9.4
    was not checked.
  - Also tried: *(list the tabs and actions)*
- This server evidently has no context path in its address, since tuipr would
  otherwise have refused the remote; so P2 below is not a problem for this
  customer, and P1 does not apply to it because it is served over https.

### Prerequisites that are not met yet

Found by reading the code while planning this. Nothing has been changed.

- [x] **P1. A local instance cannot be reached. Fixed, unit-tested only.** tuipr builds every Bitbucket
  address as `https://{host}` and probes that to recognise the server. It
  accepts an `http://` remote but drops the scheme. An instance at
  `http://localhost:7990` therefore fails at startup. Either honour the scheme
  of the remote, or put a TLS proxy the tool trusts in front of the container.
  Whether an explicit port in an `https` remote survives is unchecked.
- [x] **P2. A context path is not recognised. Fixed, unit-tested only.** Only repository paths shaped
  `PROJECT/repo` or `scm/PROJECT/repo` are accepted. A Bitbucket served under a
  context path, such as `https://host/bitbucket/scm/PROJECT/repo.git`, is
  rejected as unparseable. The customer's server works, so it has none, but
  other installations may, and their users would see tuipr refuse the remote.
  Not reproduced; it follows from reading the code. Tracked in FEATURES.md.

### What is known about the image

Checked on 2026-09-30 against Docker Hub:

- The image is `atlassian/bitbucket`, current version 10.5.0, with `amd64` and
  `arm64`, updated days ago. Older tags exist for `8.19.0`, `9.4.x` and `10.x`.
  **`9.4.23-jdk17`, the customer's exact version, exists for both `amd64` and
  `arm64`.**
- Web on port 7990, git over ssh on 7999, about 2 GiB of memory recommended,
  and Docker 20.10.10 or newer. Bitbucket 10 has no embedded search server.
- The published quick start is one `docker volume create` and one `docker run`.

Not checked, and to be confirmed on the first run: whether version 10 needs an
external database or search service to start, how the setup wizard asks for the
licence, and the current trial terms.

### Setting it up

Use `9.4.23-jdk17`, the customer's exact version, so that any difference in
fields and endpoints shows up as it would for them. A second run on `10.x` is
worth doing later, for the day they upgrade.

1. Give the container runtime enough memory. On Colima: `colima start --cpu 2
   --memory 4`.
2. Start it, pinning a version:

   ```sh
   docker volume create bitbucketVolume
   docker run -v bitbucketVolume:/var/atlassian/application-data/bitbucket \
     --name bitbucket -d -p 7990:7990 -p 7999:7999 atlassian/bitbucket:9.4.23-jdk17
   ```

3. Open `http://localhost:7990`, go through the setup wizard with a trial
   licence from Atlassian, and create an administrator.
4. Create a second user. Bitbucket does not let you approve your own PR, and
   reviewer states need someone else.
5. Create a project and a repository, push any repository with some history and
   a branch that conflicts with the main branch.
6. Create a personal access token for the administrator with write access to
   the repository, and store it with `tuipr auth login` once P1 is solved.
7. Create what the checks need:
   - an ordinary open PR, a draft PR if the version has drafts, and a PR with a
     merge conflict;
   - a repository merge check that requires approvals, and a PR that does not
     yet have them (a blocked PR), and one that does;
   - at least 26 merged and some declined PRs, for paging and `L`;
   - inline comments, replies and a resolved thread on one PR;
   - build statuses on a commit, posted through the build-status REST API.

### The checks

Run B1 to B10 below against the instance on `9.4.23-jdk17`. Writes are safe
here, so every item can be done in full. Note the tag on each result line.

Each assumption below is checked by doing the action in tuipr against the
instance and seeing that it behaves. Note the version and the result on the
line under the item.

- [ ] **B1. Authentication.** *Partly confirmed 2026-09-30:* the customer's
  server accepted the token and the list loaded. Not recorded: that your own
  PRs are recognised as yours. Run `tuipr auth login`, then `tuipr` in a clone of
  the repository. Expect the list to load and your own PRs to be recognised as
  yours: a personal access token as a `Bearer` header is accepted, and the
  `X-AUSERNAME` response header names the current user.
- [ ] **B2. Listing.** *Partly confirmed 2026-09-30:* the list loads on the
  customer's server. Not recorded: the Merged and Declined views, paging past 50
  open PRs, and the page-size limits. Read the list; press `f`, choose Merged and Declined.
  Expect open PRs, then recent merged and declined ones, without a server error
  about the page size (`limit` 50 and 25), and correct paging on a repository
  with more than 50 open PRs if one exists.
- [ ] **B3. PR fields.** In the list and on a PR's Overview, check the status of
  a draft, the comment count and each reviewer's state. Expect `APPROVED`,
  `UNAPPROVED` and `NEEDS_WORK` to show as approved, waiting and changes
  requested. Older versions may not report `draft` at all.
- [ ] **B4. Merge status and vetoes.** Open a PR that cannot be merged yet, for
  example missing an approval or a green build. Expect `blocked` in the header
  and the server's reasons in the merge dialog (`m`). On a sandbox PR, try the
  merge and expect the same reasons in the error, not just "Merging is vetoed".
- [ ] **B5. Decline and reopen.** On a sandbox PR press `x` and confirm the
  decline, then `x` and confirm the reopen. Expect the status to follow after
  the refresh.
- [ ] **B6. Comments.** On a sandbox PR: a PR-level comment, an inline comment
  on an added line and on a removed line, a reply, editing and deleting your
  own comment, and resolving a thread. Expect each to appear on the server
  where the tool says it went.
- [ ] **B7. Reviews.** On a sandbox PR: approve, request changes, withdraw the
  approval, and a batched review with inline comments and a summary. Expect the
  server to show the right status and every comment.
- [ ] **B8. Diff, commits and builds.** Read the Diff, Commits and Builds tabs
  of a real PR, including a large diff if one exists. Expect files, hunks,
  commit list and build statuses to match what the server's own page shows.
- [ ] **B9. Activity.** Read the Overview timeline of a PR with discussion.
  Expect comments, threads, approvals and, if the server shows reactions,
  those under `properties.reactions`.
- [ ] **B10. Older PRs.** In the Merged view press `L` until it ends, on a
  repository with more than 25 merged PRs. Expect batches, then the footer
  dropping `L: older` when both merged and declined are exhausted.

## D. Decisions and to-dos (you)

- [ ] **D1. Repository visibility.** While it is private, nobody else can
  download a release. Decide whether `0.1.0` makes it public.
- [ ] **D2. Screenshot or gif** in the README, recorded against a real
  repository.

## E. What I can do myself when told to

- Time the list queries on a large public repository (V7, read-only).
- Write and run the read-only Bitbucket probe described in C.3.
- Turn anything a real run reveals into a fix and a regression test.
