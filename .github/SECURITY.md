# Security

tuipr reads and writes pull requests using your existing `gh` login (GitHub) or
a personal access token that it stores in the operating system's keyring
(Bitbucket Data Center). Problems that could expose a token, or act on a
repository without the user asking, are the ones that matter most.

## Reporting a vulnerability

Please report it privately, not in a public issue: open the repository's
**Security** tab and choose **Report a vulnerability**. Include what you found,
how to reproduce it, and which version (`tuipr --version`).

Do not include real tokens or private repository contents in the report.

## Supported versions

Fixes go into the latest release and `main`. Bitbucket Data Center (the self-hosted
product that Atlassian is retiring) is in maintenance mode: security fixes are
made, new features are not.
