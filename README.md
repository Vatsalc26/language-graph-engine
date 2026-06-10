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

### Supported ASCII Characters (Phase 2.1)
* All 95 printable ASCII characters from U+0020 SPACE through U+007E TILDE inclusive are supported.
* ASCII printable operator-like characters are supported, including: `+ - * / = < > % ^ & |`
* The ASCII backslash `\` is supported.

### Explicitly Unsupported Inputs
Unsupported inputs include non-ASCII typography and layout controls, such as:
* Layout controls: tabs (`\t`), newlines (`\n`), carriage returns (`\r`).
* Smart quotes: `’`, `“`, `”`.
* Typographic dashes: `–`, `—`.
* Ellipsis: `…`.
* Non-ASCII mathematical symbols: `×`, `÷`, `−`, `≤`, `≥`, `≠`.
* Accented letters and combining graphemes (e.g. `é`, `á`).
* Emoji and other scripts.


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
* Phase 2.1 supplemental symbol/profile additions are transactional.
* If Phase 2.1 additions fail, no partial Phase 2.1 active profile is left behind.
* When bootstrapping from an earlier or empty database, prerequisite Phase 1/Phase 2 initialization may already have been committed before a Phase 2.1-specific failure.
* Existing deterministic identity blocks are never silently overwritten.

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

### Phase 3 additions: English Natural-Language Written Forms Store
Phase 3 adds a separate, logical live store: `urn:language-graph:store:english-natural-language-written-forms` (English Words).

* **A Written Form is a composite spelling/composition object, not a meaning**:
  * For example, `bank = b + a + n + k`.
  * Each saved written form is a separate immutable composite object referencing the exact immutable symbol revision CIDs used to compose it.
  * The engine does not yet know what `bank` means (senses/lexemes are deferred).
* **Identity and Storage**:
  * Each written form has a deterministic stable entity ID: `urn:language-graph:written-form:nfc:utf8:<lowercase-hex-of-normalized-utf8-bytes>`.
  * It has an immutable `WrittenFormRevision` CID representing its state.
  * Membership in the live English Words store is tracked dynamically in operational tables, separate from the immutable written-form CID.
  * Manually published store snapshots are created as distinct immutable blocks containing sorted members and revision CIDs.
  * Word persistence occurs ONLY via explicit Save operations; ordinary text resolution and preview remain strictly read-only.
* **Admission Policy**:
  * Accepting only letters with optional internal apostrophes or hyphens: `[A-Za-z]+(?:['-][A-Za-z]+)*`.
  * Technical tokens, identifiers, numeric forms, emails, and URLs are rejected from this store (though still fully resolvable in the Printable ASCII profile).

### Roadmap
* **Phase 3: Persistent Written Forms**
  Creates stored written-form objects composed from existing symbol revision CIDs. (No meanings, dictionaries, lexemes, sentences, embeddings or AI are added here).
* **Phase 4: Lexemes and Senses / Meanings**
* **Phase 5: Source Text, Sentences and Contextual Occurrences**
* **Later: Interpretation, retrieval ranking and optional semantic/AI layers**
