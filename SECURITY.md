# Security Policy

## Supported versions

The `develop` branch is the active development line. Reports affecting `develop` and the latest published release will be considered; older releases may no longer receive security updates.

## Report a vulnerability

Do not publish vulnerability details in issues, pull requests, discussions, or public comments. If GitHub provides the private **Report a vulnerability** form on the repository’s Security page, use it. Otherwise, use the [Potoo contact page](https://diegopn.github.io/#contato) only to request a private reporting channel. Do not include technical details or a proof of concept in that first message.

After arranging a private channel, include the affected version or commit, Rust version, operating system, reproduction steps, impact, and a minimal `PSeInt` example. Remove credentials, personal information, and unrelated data.

For non-security issues, see [CONTRIBUTING.md](CONTRIBUTING.md). For conduct concerns, see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security boundaries

The repository currently contains only a crate scaffold and does not execute PSeInt source. This policy will be updated with the runtime boundaries and resource limits before source execution is implemented. When execution is introduced, hosts running untrusted source should apply process isolation; resource limits alone are not an operating-system sandbox.
