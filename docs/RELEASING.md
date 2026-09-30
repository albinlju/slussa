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

[VERIFICATION.md](VERIFICATION.md) lists what has not been checked against a
real service and how to check it: the release rehearsal, a clean install, the
Linux ARM binary, a blocked GitHub PR, a real review request, reopening, a
large repository, and Bitbucket Data Center. It also holds the two decisions
the release needs, repository visibility and a screenshot for the README.
Release when its items are ticked or knowingly accepted.

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
