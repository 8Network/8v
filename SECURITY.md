# Security Policy

## Reporting a vulnerability

If you discover a security vulnerability in 8v, please report it responsibly.

**Email:** security@8network.io

Please include:
- Description of the vulnerability
- Steps to reproduce
- Impact assessment
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and provide a timeline for resolution.

## Scope

8v executes external tools (compilers, linters, formatters) as subprocesses. Security-relevant areas include:

- **Filesystem containment** (`o8v-fs`): symlink traversal, path escape, FIFO/device rejection
- **Process execution** (`o8v-process`): command injection, timeout enforcement, signal handling
- **Input parsing**: malformed tool output, oversized files, encoding attacks

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Disclosure policy

We follow coordinated disclosure. We ask that you give us 90 days to address the issue before public disclosure.
