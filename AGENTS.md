# AGENTS.md

Follow the goals and rules in [CONTRIBUTORS.md](CONTRIBUTORS.md). [docs/how-it-works.md](docs/how-it-works.md)
describes the code layout.

## GUI tests

The user shares the PC, so simulated input and their own input interfere.

- Before simulating input, notify the user and ask "Are you ready for a short GUI test?", always with "I'll test it
  myself" as an option. Afterwards, say "You can use your PC again".
- Simulated clicks don't open the tray popup. After one failed try, ask the user to click.
- Blame failed tests on interference first. No workarounds for test-only failures.
