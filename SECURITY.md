# Security policy

ScriptCraft opens files that other people write, listens on local sockets and can be driven by
agents. Security reports are welcome and taken seriously.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Use GitHub's private reporting:
<https://github.com/NoJuanNobody/scriptcraft/security/advisories/new>

If you cannot use GitHub, email <alondono11@gmail.com> with "ScriptCraft security" in the subject.

Include what you found, how to reproduce it (a sample file or command is ideal) and the version or
commit. You will get an acknowledgement within 7 days. This is a single-maintainer project, so fix
times vary, but reports are triaged first and you will be told the outcome.

## Supported versions

Only the latest release and the `main` branch receive security fixes.

## What is in scope

| Surface | What to expect |
|---|---|
| **File import** (`.fdx`, `.fountain`, `.docx`, Celtx, PDF) | Treated as hostile input. A crash, hang, unbounded memory use or unsafe write caused by a crafted file is a bug. See "Never crash" in [AGENTS.md](AGENTS.md). |
| **Control channel** | JSON lines on `127.0.0.1` only. Every request must carry a random 256-bit key, stored in a file with mode 0600 and removed on exit. See [docs/control-protocol.md](docs/control-protocol.md). Bypassing the key or binding beyond loopback is in scope. |
| **MCP server** | Headless over stdio, or bridged to the control channel, so it inherits the same key. See [docs/mcp.md](docs/mcp.md). |
| **Collaboration** | TCP on `127.0.0.1` by default; `lan: true` binds every interface. A joiner must present a random 128-bit session key, compared in constant time. Lines over 32 MB or malformed lines are dropped. See [docs/collaboration.md](docs/collaboration.md). |
| **Settings and recovery files** | Parsed through `craft-settings`. A crafted config or recovery file that causes a crash or data loss is in scope. |

## Out of scope

- Anyone who already has your user account or can read your key file.
- Sharing a collaboration key over a channel you do not trust (the docs ask you to pick one).
- Denial of service that needs an attacker to already control the local machine.

## Our practices

- No `unwrap`, `expect`, `panic!` or `unsafe` in non-test code; input-derived numbers are clamped.
- A test runs every command with junk parameters (`hostile_params_never_panic`).
- CI runs formatting, `clippy -D warnings`, tests, an asset check and a layering check on every push
  and weekly, to catch breakage from new dependencies.

## Disclosure

I will credit reporters in the release notes unless you prefer to stay anonymous.
