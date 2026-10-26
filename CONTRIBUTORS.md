# Contributing

## Goals

- Minimal, concise code. No code without a purpose.
- Fast at runtime and light when idle.
- Readable and maintainable. One clear style, no mixed approaches.

## Dependencies

- Prefer the Rust stdlib, then the libraries we already use, then OS APIs.

## Safety

- Avoid `unsafe`. Keep the `unsafe` that remains in small, safe wrappers.

## Platforms

- Call the OS only where neither the stdlib nor a library does the job.
- OS calls live in `src/platform/<os>/<module>.rs`, named after the module they serve. Generic modules hold the logic
  and the shared types.
- Every platform feature is optional: a platform without it returns nothing, and the rest keeps working.

## Constants

- Take ids and keys from libraries or read them at runtime. Hard-code them only when neither works, with a comment
  saying why.
- Never hard-code anything that only fits one machine, such as device addresses, serials or screen positions.

## Structure and style

- File layout follows responsibility, and every module follows the same pattern.
- Match the surrounding code in naming, comments and idiom.

## Documentation

- Keep docs and comments concise and to the point.
- Leave out the obvious, in code comments as in user docs.
- Edit docs only where a change makes them wrong or incomplete.

## Changes

- Refactors keep every feature and behavior.
- Before committing, run `cargo fmt` and `cargo clippy` with zero warnings.
- Commit messages are short and concise and describe what the commit changes.
