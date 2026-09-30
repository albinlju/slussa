# Releasing slussa

Releases are built by `.github/workflows/release.yml` when a version tag is
pushed. The workflow builds four targets, packages each as a tarball with a
`.sha256` file, and publishes a GitHub Release with generated notes and a
combined `SHA256SUMS`.

| Target | Built on |
| --- | --- |
| `aarch64-apple-darwin` | macOS runner |
| `x86_64-apple-darwin` | macOS runner, cross-compiled |
| `x86_64-unknown-linux-gnu` | Ubuntu 22.04 (glibc 2.35 or newer) |
| `aarch64-unknown-linux-gnu` | Ubuntu 22.04, cross-compiled with gcc |

## Before the first release (0.1.0)

Checked against the real services before 0.1.0: two release rehearsals (the
second with the renamed package), the macOS and Linux ARM binaries, a blocked
GitHub PR, reopening a PR and a large repository. Still open:

- **Repository visibility.** While the repository is private nobody else can
  download a release. Decide whether 0.1.0 makes it public.
- **A real review request.** "Review requested" in the list has only been seen
  against scripted `gh` output. It needs a PR where someone else asked for your
  review, which one account cannot create. It can follow the release.
- **Bitbucket Data Center** is in maintenance and has only been seen listing
  PRs on one real server; every write is tested against a mock of the documented
  API. The README says so.

Release when these are done or knowingly accepted. The last two are accepted
for 0.1.0.

## Cut a release

1. Make sure `main` has a green CI run. The release workflow does not re-run
   the test suite.
2. Set `version` in `Cargo.toml` and commit the change, including `Cargo.lock`.
3. Tag and push. The tag must be `v` plus the exact Cargo version:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

   A version with a suffix such as `0.2.0-pre.1` (tag `v0.2.0-pre.1`) is
   published as a prerelease.

The first job fails the run if the tag and `Cargo.toml` disagree, before
anything is built.

## Dry run

Run the **Release** workflow manually from the Actions tab. It builds and
uploads all four archives as workflow artifacts and skips publishing. Do this
before the first real release and after any change to the workflow.

## Verify a download

```sh
shasum -a 256 -c slussa-0.2.0-aarch64-apple-darwin.tar.gz.sha256
```

## Not automated yet

- A Homebrew tap (needs a separate `homebrew-slussa` repository).
- Publishing to crates.io: remove `publish = false` from `Cargo.toml` first.
- Signing and notarizing the macOS binaries. Downloaded binaries may need
  `xattr -d com.apple.quarantine slussa` until that exists.
