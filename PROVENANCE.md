# Provenance

This repository is the clean side of a two-wall rewrite. The original Comical
0.8 source is GPL. The goal is an MIT application written from behavior only.

## Roles

| Role | Name | May read original source | May write product code |
|---|---|---:|---:|
| Spec author | Engineer (Bloks) | Yes | No |
| Implementer | pending | No | Yes |
| Oracle runner | pending | Yes | No |
| Reviewer | pending | No | No |
| Legal reviewer | pending | Yes | No |

## Spec author attestation

I, Engineer (Bloks), attest that:

1. I read the Comical 0.8 source during analysis.
2. I wrote `spec/behavior.md` and the seed vectors from behavior, not by
   copying source text, comments or identifiers.
3. I did not write product code in this repository.
4. The clean repository contains no original source or source-derived code.

Signed: Engineer (Bloks)
Date: 2026-10-20
Spec version: spec-v1

## Implementer attestation

Add this before the first code commit:

```
I, <name>, attest that:

1. I have not read the Comical 0.8 source code or any source-derived notes.
2. I wrote the code in this contribution from the behavior spec and test vectors.
3. I did not copy code, comments, identifiers or structure from the original.
4. I have disclosed every external source I used.

Signed: <name>
Date: <date>
Spec version: spec-v1
```

## Access log

- 2026-10-20: clean repository created by the spec author. No implementer named
  yet. No product code.

## AI caveat

Clean-room doctrine was built for people. Model weights are not auditable. A
separate session with no access to the source is the best available
approximation, not a legal guarantee. Legal review is required before release.

## Forbidden material

The original source, the analysis document and the C++ harness are outside this
repository. They are not listed here on purpose. Do not search for them.
