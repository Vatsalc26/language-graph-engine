# Language Graph Engine (Phase 1)

Language Graph Engine is a local, versioned language-object and retrieval engine. It replaces a previous Git-based prototype with a performant, SQLite-backed, and content-addressed architecture.

---

## 1. Product Vision & Phase 1 Scope

The long-term vision of this engine is to provide a durable local storage layer for natural language, supporting graphemes, wordforms, lexemes, relationships, sentences, and semantic retrieval layers.

**Phase 1** focuses strictly on the foundational collection: **Lowercase Latin letters `a` through `z`**.

---

## 2. Architecture & Design Principles

### Replacing the Git Prototype
The old Git-based prototype resolved symbols by walking Git repositories and utilizing Git commit hashes. This introduced significant file system overhead, subprocess costs, and runtime dependencies. The new engine implements a lightweight, embedded content-addressed storage (CAS) model utilizing SQLite, completely removing Git from the application runtime.

### Identity Distinctions
Our architecture separates stable identity from immutable content:

1. **Stable Entity ID (URN)**: Identifies a logical object over time. Canonical seeded symbols use deterministic IDs instead of random UUIDs.
   * *Example*: `urn:language-graph:grapheme:nfc:0061` identifies the grapheme `a`.
   * *Collection*: `urn:language-graph:collection:latin-lowercase-a-z` identifies the lowercase alphabet collection.
2. **Immutable Revision CID**: Identifies the exact cryptographic block representing a specific revision of a symbol. This is a CIDv1 generated from canonical DAG-CBOR bytes using SHA2-256.
   * *Example*: `bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a`
3. **Published Collection Snapshot CID**: An immutable snapshot object containing the ordered membership of the collection and the exact revision CID chosen for each symbol. The snapshot itself receives its own CIDv1.

### Deterministic Content-Addressing (CIDv1 + DAG-CBOR + SHA2-256)
Instead of using arbitrary JSON serialization (which is subject to key order and whitespace variations), immutable objects are serialized using **canonical DAG-CBOR** key ordering. The resulting byte stream is hashed using SHA2-256 and wrapped in a standard **CIDv1** container (codec `0x71` for `dag-cbor`).

### High Performance & Local Storage
* **In-Memory Caching**: Normal resolution requests run against an in-memory cache loaded at startup from the active alphabet snapshot. It does not re-hash characters or scan the database on every lookup, enabling microsecond resolutions.
* **SQLite Embedded Database**: Uses `rusqlite` with bundled SQLite, running completely inside the application process. No separate database server or configurations are required.
* **Unicode-Safe Design**: Text is processed using grapheme-aware segmentations (`unicode-segmentation` crate) and normalized to NFC format, ensuring the engine is architecturally ready for future Unicode and combining-character support.

---

## 3. Installation & Getting Started

### Prerequisites (Install Rust)
If Rust and Cargo are not installed:
1. Download the official Rust installer through [rustup](https://rustup.rs/) for Windows.
2. Follow the prompt to install the desktop C++ build tools if requested.
3. Open a fresh PowerShell window and verify:
   ```powershell
   rustc --version
   cargo --version
   ```

### Development & Build Commands

1. **Navigate to the Project Root**:
   ```powershell
   cd "D:\Prototypes\Project_3"
   ```

2. **Run the Automated Tests**:
   Runs content addressing proofs, seeding idempotency tests, resolver logic, and HTTP server endpoint tests.
   ```powershell
   cargo test
   ```

3. **Start the Application Server**:
   ```powershell
   cargo run
   ```
   The engine will initialize the database at `D:\Prototypes\Project_3\data\language_graph.sqlite`, run seeding verification, and start a web server at [http://localhost:8080](http://localhost:8080).

4. **Build a Windows Release Executable**:
   Generates an optimized standalone executable.
   ```powershell
   cargo build --release
   ```
   The output binary will be located at `target\release\language-graph-engine.exe`.

---

## 4. Known Limitations & Roadmap

### Phase 1 Limitations
* Supports lowercase Latin letters `a` through `z` only.
* Uppercase letters, digits, spaces, punctuation, emoji, and combining character sequences are rejected with friendly validation errors.
* The registry is read-only at runtime; no mutation APIs are active outside of startup seeding.

### Roadmap
* **Phase 2**: Introduce uppercase letters, spaces, digits, and common punctuation.
* **Phase 3**: Grapheme clusters, diacritics, and Unicode symbol expansions.
* **Phase 4**: Wordforms, lexemes, dictionaries, and lexical metadata tracking.
* **Phase 5**: Sentences, typing relationships, contextual annotations, and vector embeddings.
