# slussa

> *slussa* (Swedish): to pass a boat through a lock. The *slussvakt* is the lock
> keeper who decides what gets through.

A terminal UI for pull requests. Two views: the list of PRs, and the PR you
opened. Read, comment, review, merge and decline without leaving the terminal,
next to your editor, git client and coding agent.

The list opens sorted by what needs you, with the reason beside each PR, and a
merge that cannot go through says why. It is not a replacement for the web UI,
and it stays small on purpose.

![slussa: the list sorted by what needs you, then an agent-written PR: its stated intent, the review conversation and the diff](https://raw.githubusercontent.com/albinlju/slussa/main/docs/media/demo.gif)

**Where it is going.** Pull requests written by agents, and reviews written by
AI, are becoming most of what a reviewer sees. The plan is to make them
first-class: marked as AI, summarized in the header, and handed back to a
coding agent. None of that is built yet; [ROADMAP.md](docs/ROADMAP.md) has the
plan.

**Status:** early. It is used daily by its author. Prebuilt binaries come with
tagged releases.

## Providers

| Provider | How it connects | Notes |
| --- | --- | --- |
| GitHub | the [`gh`](https://cli.github.com) CLI | uses your existing `gh auth login` |
| Bitbucket Data Center | REST with a personal access token | token in the OS keyring; tested against a mock, and only listing has been seen on a real server |

Bitbucket Data Center is Atlassian's self-hosted Bitbucket, not Bitbucket Cloud.
Atlassian ends Data Center licence sales on 2028-03-30 and support on
2029-03-28, so this provider is in maintenance: bugs are fixed, nothing new is
added for it. Bitbucket Cloud is a different product that this does not affect;
it and GitLab are on the roadmap, not supported.

The provider is detected from the `origin` remote of the repository you run
slussa in.

## Install

**Platforms.** macOS and Linux, on arm64 and x86_64, are built and tested in
CI. Windows is not built or tested yet.

**Prebuilt binary.** Tagged releases attach tarballs for macOS and Linux
(arm64 and x86_64) to GitHub Releases, with checksums. Unpack it and put
`slussa` on your `PATH`. The macOS binary is not signed or notarized, so a file
downloaded in a browser may be blocked; remove the flag with
`xattr -d com.apple.quarantine slussa`. Check a download with
`grep <target> SHA256SUMS | shasum -a 256 -c -`, for example
`aarch64-apple-darwin`.

**From crates.io.** With a recent Rust toolchain (1.95 or newer):

```sh
cargo install slussa --locked
```

Run the same command again to update to a newer version.

**From source.** To build the current `main` instead:

```sh
cargo install --git https://github.com/albinlju/slussa
```

At runtime slussa needs `git`, and for GitHub the `gh` CLI, authenticated for
the host.

## Use

```sh
cd path/to/a/repo
slussa                  # open the PR browser for this repo
slussa -C path/to/repo  # same, as if started in that directory
slussa auth login       # Bitbucket Data Center: store a personal access token
slussa --version
slussa --help
```

For Bitbucket Data Center, run `slussa auth login` once per host. For GitHub,
run `gh auth login` instead.

Press `?` in any view for the keys that are available right now; the full list
is in [KEYS.md](docs/KEYS.md). To start with: `j`/`k` move, `enter` opens, `/`
searches, `f` filters by status, `s` switches the sort and `q` quits. Actions the
connected provider does not support are hidden; actions blocked by the PR's
state (for example merging with conflicts) stay visible and say why.

A "Needs you" column, shown at 90 columns or wider, says why a PR is on top:
changes requested, CI failed, review requested, or approved. Team review requests
on GitHub are not counted yet. Press `s` for plain newest-first order, or set
`sort = "recent"`.

Drafts (comment editor text and queued review comments) are saved locally and
survive a restart. Copying a link falls back to the terminal's clipboard (OSC 52)
over SSH; inside tmux that needs `set -g set-clipboard on`.

## Configure

`~/.config/slussa/config.toml` (or `$XDG_CONFIG_HOME/slussa/config.toml`):

```toml
theme = "graphite"   # graphite (default), slate, gruvbox, catppuccin, terminal
sort = "attention"   # attention (default) or recent

[ai]
# A comment whose first line starts with one of these is an AI agent's.
markers = ["> **gator-agent**"]
```

`SLUSSA_THEME` overrides the file.

With `markers` set, `f` in the Overview shows all comments, only people's, or only
the agents'. Without it nothing is marked and the key is not offered. Matching is
by text, so it works on both providers and for agents that post with your own token.

## Documentation

- [KEYS.md](docs/KEYS.md): every key, for the list and for a PR
- [ROADMAP.md](docs/ROADMAP.md): the product idea, what is planned and the engineering backlog
- [ARCHITECTURE.md](docs/ARCHITECTURE.md): how the code is organised and why

## Contributing

Open an issue first, then send a pull request from a fork. How to build and test,
and the rules, are in [CONTRIBUTING.md](CONTRIBUTING.md); security problems go
through [SECURITY.md](.github/SECURITY.md).

## License

[MIT](LICENSE)
