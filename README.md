# Language Graph Engine (Phase 2.1)

Language Graph Engine is a local, versioned language-object and retrieval engine. It is built around a performant, SQLite-backed, and content-addressed architecture.

---

## 1. Product Vision & Phase 2 Scope

The long-term vision of this engine is to provide a durable local storage layer for natural language, supporting graphemes, wordforms, lexemes, relationships, sentences, and semantic retrieval layers.

* **Phase 1** focused strictly on the lowercase Latin letters `a` through `z` (26 symbols).
* **Phase 2** expanded the system to support a Basic English Written Text Profile containing exactly 74 written symbols.
* **Phase 2.1** expands the system to support the **Printable ASCII Text Profile** containing exactly **95 written symbols**:
  * 26 lowercase letters: `abcdefghijklmnopqrstuvwxyz`
  * 26 uppercase letters: `ABCDEFGHIJKLMNOPQRSTUVWXYZ`
  * 10 decimal digits: `0123456789`
  * 1 whitespace separator: `U+0020 SPACE` (tabs, newlines, and carriage returns are rejected)
  * 11 essential ASCII punctuation symbols: `. , ? ! ' " - : ; ( )`
  * 21 supplemental ASCII symbols: `# $ % & * + / < = > @ [ \ ] ^ _ ` { | } ~`

### Why digits and punctuation are separate collections
Instead of dumping digits and punctuation into a single "alphabet" collection, they are represented as distinct immutable collections (`urn:language-graph:collection:decimal-digits-0-9` and `urn:language-graph:collection:basic-english-punctuation`). This models the linguistic boundary between alphabetic characters, numerical symbols, and typographic separators, allowing future profiles (such as numeric-only or specific language profiles) to reuse them independently.

### Explicitly unsupported in Phase 2
Smart quotes (`’`, `“`, `”`), typographic dashes (`–`, `—`), ellipses (`…`), accented letters (e.g. `é`), combining characters (e.g. `á`), emoji, backslashes, math operators, and other scripts are rejected with descriptive validation errors. This maintains strict bounds on text formatting and ensures input sanitization is deterministic.

---

## 2. Architecture & Design Principles

### Identity Distinctions
Our architecture separates stable identity from immutable content:

1. **Stable Entity ID (URN)**: Identifies a logical object over time. Canonical seeded symbols use deterministic IDs based on their NFC Unicode hexadecimal scalar.
   * *Example*: `urn:language-graph:grapheme:nfc:0041` identifies the grapheme `A`.
2. **Immutable Revision CID**: Identifies the exact cryptographic block representing a specific revision of a symbol (CIDv1 generated from canonical DAG-CBOR bytes using SHA2-256).
   * *Example*: `bafyreievw56s5ltwde2xmmxzt3etkfz73qsutx43x7xuxe3iuvnbpobm2e`
3. **Published Collection Snapshot CID**: An immutable snapshot object containing the ordered membership of a collection and the exact revision CIDs chosen for each symbol.
4. **Active Text-Profile Snapshot CID**: An immutable profile snapshot object (`urn:language-graph:profile:printable-ascii-text`) referencing the ordered list of 6 collection snapshots forming the resolver environment.
   * *Example*: `bafyreidfdj3hw7gv5rt7bpsfkrkhuptprcjlwzpaq3yektnztec4caqdn4`

### Seeding Guarantees
Seeding is idempotent and transactional. Re-running the database initialization:
* Never alters Phase 1 blocks or CIDs.
* Never generates duplicate blocks or entities.
* Fails with a controlled integrity error if there are conflicting entities or heads.
* Rolls back atomically if any step fails, leaving the database unmodified.

### High Performance & Read-Only Resolution
* **In-Memory Caching**: On startup, the active profile snapshot is loaded into a flat 95-symbol cache.
* **Microsecond Resolving**: The text resolver works entirely in memory using the pre-loaded cache. 
* **Read-Only**: Normal text resolution creates no new CIDs, performs no database writes, does not execute migrations, and makes no network API calls.
* **Duplicate Detection**: Repeated graphemes in a single request are resolved from the cache once and marked as `Reused` in the trace.

---

## 3. Installation & Development Commands

### Prerequisites (Install Rust)
Verify Rust and Cargo are installed:
```powershell
rustc --version
cargo --version
```

### Build & Run Commands

1. **Run the Automated Tests**:
   Runs all 34 regression tests (golden CIDs, database rollback, resolver checks, property-based tests, mock HTTP endpoints):
   ```powershell
   cargo test
   ```

2. **Run Benchmarks**:
   Compiles and runs performance benches (warm-cache resolution, database cache load, seeding initialization, CID computation for revisions, collections, and profiles):
   ```powershell
   cargo bench
   ```

3. **Start the Application Server**:
   ```powershell
   cargo run
   ```
   This will initialize the database at `data/language_graph.sqlite`, run migrations/seeding, and start the local browser application on port `8080`.

---

## 4. Known Limitations & Roadmap

### Phase 2.1 Limitations
* Registry is read-only at runtime; mutations only happen through startup seeding.
* Supports exactly 95 symbols; other typographic variations or layout whitespaces (tabs/newlines) are blocked.

### Roadmap
* **Phase 3**: Persistent wordforms, lexeme records, dictionaries, and lexical metadata tracking.
* **Phase 4**: Meanings, definitions, semantic annotations, sentences, and vector embeddings.
