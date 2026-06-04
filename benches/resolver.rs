use criterion::{criterion_group, criterion_main, Criterion};
use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::to_dag_cbor;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::model::{AlphabetSnapshot, GraphemeRevision, SnapshotMember};
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::lowercase_latin::seed_lowercase_latin;
use rusqlite::Connection;

fn get_temp_db_and_resolver() -> (Connection, TextResolver) {
    let mut conn = Connection::open_in_memory().expect("Failed to open database");
    run_migrations(&conn).expect("Migrations");
    seed_lowercase_latin(&mut conn).expect("Seeding");
    let resolver = TextResolver::load(&conn).expect("Load resolver");
    (conn, resolver)
}

fn bench_resolver_ops(c: &mut Criterion) {
    let (_conn, resolver) = get_temp_db_and_resolver();

    // 1. Resolve a short warm-cache valid input ("banana")
    c.bench_function("resolver_short_banana", |b| {
        b.iter(|| resolver.resolve("banana").unwrap())
    });

    // 2. Resolve a medium valid input (~1,000 graphemes)
    let medium_input = "vatsalbananaorchestration".repeat(40); // 25 chars * 40 = 1,000 chars
    c.bench_function("resolver_medium_1k_chars", |b| {
        b.iter(|| resolver.resolve(&medium_input).unwrap())
    });

    // 3. Resolve a larger valid input (~100,000 graphemes)
    let large_input = "vatsalbananaorchestration".repeat(4000); // 25 chars * 4000 = 100,000 chars
    c.bench_function("resolver_large_100k_chars", |b| {
        b.iter(|| resolver.resolve(&large_input).unwrap())
    });

    // 4. Load the active 26-symbol snapshot mapping from SQLite into memory
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    seed_lowercase_latin(&mut conn).unwrap();

    c.bench_function("load_resolver_cache_from_db", |b| {
        b.iter(|| {
            let _ = TextResolver::load(&conn).unwrap();
        })
    });

    // 5. Seed a fresh temporary database
    c.bench_function("seed_fresh_db", |b| {
        b.iter(|| {
            let mut conn_temp = Connection::open_in_memory().unwrap();
            run_migrations(&conn_temp).unwrap();
            let _ = seed_lowercase_latin(&mut conn_temp).unwrap();
        })
    });

    // 6. Encode and compute the CID for one immutable grapheme revision ('a')
    let rev_a = GraphemeRevision {
        schema: "language-graph/grapheme-revision/v1".to_string(),
        entity_id: "urn:language-graph:grapheme:nfc:0061".to_string(),
        kind: "grapheme".to_string(),
        surface_form: "a".to_string(),
        normalized_form: "a".to_string(),
        normalization: "NFC".to_string(),
        unicode_scalars: vec!["U+0061".to_string()],
        script: "Latn".to_string(),
        case: "lowercase".to_string(),
        previous_revision_cid: None,
    };

    c.bench_function("cid_grapheme_revision_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&rev_a).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });

    // 7. Encode and compute the CID for the full initial 26-member alphabet snapshot
    let mut members = Vec::new();
    for i in 0..26 {
        members.push(SnapshotMember {
            position: (i + 1) as i32,
            entity_id: format!("urn:language-graph:grapheme:nfc:{:04x}", (b'a' + i) as u32),
            revision_cid: "bafyreigzc6usxy4ufmz43vpotqo54cqrdqwtzgebsxwhyybmkgphbfwq5a".to_string(),
        });
    }
    let snap = AlphabetSnapshot {
        schema: "language-graph/collection-snapshot/v1".to_string(),
        collection_entity_id: "urn:language-graph:collection:latin-lowercase-a-z".to_string(),
        kind: "ordered-grapheme-collection".to_string(),
        label: "Latin lowercase alphabet a-z".to_string(),
        members,
    };

    c.bench_function("cid_snapshot_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&snap).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });
}

criterion_group!(benches, bench_resolver_ops);
criterion_main!(benches);
