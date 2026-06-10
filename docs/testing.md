# Testing Architecture & Guidelines

This document outlines the testing architecture, quality check commands, and verification guidelines for the **Language Graph Engine**.

---

## 1. Scope & Constraints

### Supported in Phase 1
* **Input Characters**: Lowercase Latin graphemes `a` through `z` only.
* **Format Policy**: CIDv1 over canonical DAG-CBOR encoded objects with SHA2-256 multihashes.
* **Storage**: Relational database storage (SQLite) mapping stable entities to heads, and storing raw blocks.

### Intentionally Not Supported
The following features are out of scope for Phase 1 testing and must not be implemented:
* Uppercase letters, punctuation, spaces, or digits.
* Wordforms, meaning nodes, sentences, generic graph relations.
* Vector search, embeddings, or AI reasoning layers.
* Git-based runtime operations.

---

## 2. Key Architecture Distinctions

Our testing ensures these boundaries remain clean:
1. **Stable Entity ID (URN)**: Conceptual symbol representation (e.g., `urn:language-graph:grapheme:nfc:0061`). Durable over time.
2. **Immutable Revision CID**: Identifies the exact cryptographic block representing a specific revision of a symbol. Does not contain volatile timestamps or logs.
3. **Alphabet Snapshot CID**: Identifies the collection snapshot state.
4. **Resolution Performance**: Ordinary resolution is an in-memory lookup operation. It does not compute new CIDs, query the database, or execute Git commands.

---

## 3. Running Quality Checks

Developer validation command scripts and gates are described below.

### Developer Verification Script
Run the automated script to check code formatting, compile-time warnings, all unit/integration tests, doc tests, and benchmark compilations:
```powershell
.\scripts\verify.ps1
```

### Manual Command Reference
To execute quality checks manually:

1. **Formatting Check**:
   ```powershell
   cargo fmt --all -- --check
   ```
2. **Clippy (Treat Warnings as Errors)**:
   ```powershell
   cargo clippy --all-targets --all-features -- -D warnings
   ```
3. **Run Unit & Integration Tests**:
   ```powershell
   cargo test --all-targets
   ```
4. **Run Documentation Tests**:
   ```powershell
   cargo test --doc
   ```
5. **Compile Benchmarks**:
   ```powershell
   cargo bench --no-run
   ```

### Optional Deeper Verification Tools
If the corresponding Cargo extensions are installed locally, you can run:

* **Nextest** (Parallel test executor):
  ```powershell
  cargo nextest run
  ```
* **Coverage report** (HTML coverage generator):
  ```powershell
  cargo llvm-cov --all-targets --html
  ```
* **Dependency Audit**:
  ```powershell
  cargo audit
  ```
* **Dependency Deny Check**:
  ```powershell
  cargo deny check
  ```
* **Mutation Testing**:
  ```powershell
  cargo mutants
  ```

---

## 4. Test Isolation Policy

* All automated tests must run in isolated environments using temporary file-backed SQLite connections (via `tempfile`) or in-memory databases.
* Tests **must not** modify or write to the permanent database at `D:\Prototypes\Project_3\data\language_graph.sqlite`.
* HTTP integration tests must run in-process using the `tower::Service` API on the Axum router, without binding local TCP ports.
