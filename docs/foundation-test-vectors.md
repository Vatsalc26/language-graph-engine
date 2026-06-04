# Foundation Test Vectors (Phase 1)

This document locks in the golden vector values for canonical entities and their content-addressed CIDv1 identifiers in Phase 1 of the **Language Graph Engine**.

Any change to these CIDs indicates a modification in the serialization structure or CID hashing mechanism, which constitutes a breaking migration event.

---

## 1. Golden Revision CIDs

These are the deterministic CIDv1 identifiers generated for the initial seeded lowercase characters using the schema `language-graph/grapheme-revision/v1`, encoded in canonical DAG-CBOR and hashed via SHA2-256.

| Character | Stable Entity URN | Golden Revision CIDv1 |
| --------- | ----------------- | --------------------- |
| **a** | `urn:language-graph:grapheme:nfc:0061` | `bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a` |
| **z** | `urn:language-graph:grapheme:nfc:007a` | `bafyreiamiyi7szcus6c67balumi6ejdvby5jiad5yr375sveyufkkb63dm` |

---

## 2. Golden Snapshot CID

The published alphabet snapshot collects all 26 lowercase letter members (ordered `1` to `26`), each pointing to its golden revision CID.

* **Collection URN**: `urn:language-graph:collection:latin-lowercase-a-z`
* **Golden Snapshot CIDv1**: `bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm`
