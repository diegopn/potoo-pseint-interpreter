# Security Policy

## Supported versions

The `develop` branch is the active development line. Reports affecting `develop` and the latest published release will be considered; older releases may no longer receive security updates.

## Report a vulnerability

Do not publish vulnerability details in issues, pull requests, discussions, or public comments. If GitHub provides the private **Report a vulnerability** form on the repository’s Security page, use it. Otherwise, use the [Potoo contact page](https://diegopn.github.io/#contato) only to request a private reporting channel. Do not include technical details or a proof of concept in that first message.

After arranging a private channel, include the affected version or commit, Rust version, operating system, reproduction steps, impact, and a minimal `PSeInt` example. Remove credentials, personal information, and unrelated data.

For non-security issues, see [CONTRIBUTING.md](CONTRIBUTING.md). For conduct concerns, see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security boundaries

The interpreter executes PSeInt source in the calling process. It does not create an operating-system sandbox or enforce general CPU or memory quotas. The CLI reads the selected source file and uses the process's standard input and output; an embedding `Host` supplies the runtime services it implements. Nesting and call-depth limits are implementation guards, not isolation. Run untrusted source in a restricted process with operating-system access and resource limits.
