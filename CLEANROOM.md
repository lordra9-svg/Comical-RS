# Clean-room rules for the implementer

You are the implementer. Your job is to write new code from behavior only.

## Read only this repository

Allowed:

- `spec/behavior.md`
- `tests/vectors/`
- `CLEANROOM.md`, `README.md`, `PROVENANCE.md`
- Your own code and tests

Forbidden:

- The original Comical source tree, in any form.
- The analysis document and its file and line references.
- Any C++ harness or source excerpt.
- Bloks memory files and artifacts.
- Web searches for Comical source or code.
- Copied comments, identifiers or structure from the original.

## Work rules

- Design your own modules, names and algorithms.
- Use the spec and vectors as the contract.
- Add tests for every behavior you implement.
- Commit with `Clean-Room: spec-v1`.
- Add your attestation to `PROVENANCE.md` before the first code commit.
- If you see forbidden material, stop and report it. Do not continue.

## What to build first

Scaffold the Cargo workspace and implement `comical-cli probe <book>` for ZIP,
RAR and 7z. Do not start the UI yet.
