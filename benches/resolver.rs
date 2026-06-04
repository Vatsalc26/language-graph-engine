use criterion::{criterion_group, criterion_main, Criterion};
use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::to_dag_cbor;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::model::{
    AlphabetSnapshot, GraphemeRevision, ProfileCollectionRef, SnapshotMember, TextProfileSnapshot,
};
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::ascii_supplemental::seed_phase2_1;
use rusqlite::Connection;

fn get_temp_db_and_resolver() -> (Connection, TextResolver) {
    let mut conn = Connection::open_in_memory().expect("Failed to open database");
    run_migrations(&conn).expect("Migrations");
    seed_phase2_1(&mut conn).expect("Seeding");
    let resolver = TextResolver::load(&conn).expect("Load resolver");
    (conn, resolver)
}

fn bench_resolver_ops(c: &mut Criterion) {
    let (_conn, resolver) = get_temp_db_and_resolver();

    // 1. Resolve a short warm-cache valid input ("banana")
    c.bench_function("resolver_short_banana", |b| {
        b.iter(|| resolver.resolve("banana").unwrap())
    });

    // 2. Resolve a warm-cache valid input (Phase 2.1 short check)
    c.bench_function("resolver_hello_vatsal", |b| {
        b.iter(|| resolver.resolve("Hello, #Vatsal! +~`").unwrap())
    });

    // 3. Resolve a medium valid input (~1,000 graphemes)
    let medium_input = "Hello, #Vatsal! Room 101: Is this working? (Phase 2.1) - 2026! +~` ".repeat(15);
    c.bench_function("resolver_medium_1k_chars", |b| {
        b.iter(|| resolver.resolve(&medium_input).unwrap())
    });

    // 4. Resolve a larger valid input (~100,000 graphemes)
    let large_input = "Hello, #Vatsal! Room 101: Is this working? (Phase 2.1) - 2026! +~` ".repeat(1500);
    c.bench_function("resolver_large_100k_chars", |b| {
        b.iter(|| resolver.resolve(&large_input).unwrap())
    });

    // 5. Load the active 95-symbol snapshot mapping from SQLite into memory
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    seed_phase2_1(&mut conn).unwrap();

    c.bench_function("load_resolver_cache_from_db_95_symbols", |b| {
        b.iter(|| {
            let _ = TextResolver::load(&conn).unwrap();
        })
    });

    // 6. Fresh Phase 2.1 seed/migration initialization
    c.bench_function("seed_fresh_db_phase2_1", |b| {
        b.iter(|| {
            let mut conn_temp = Connection::open_in_memory().unwrap();
            run_migrations(&conn_temp).unwrap();
            let _ = seed_phase2_1(&mut conn_temp).unwrap();
        })
    });

    // 7. Compute one new uppercase revision CID ('A')
    let rev_a = GraphemeRevision {
        schema: "language-graph/grapheme-revision/v1".to_string(),
        entity_id: "urn:language-graph:grapheme:nfc:0041".to_string(),
        kind: "grapheme".to_string(),
        surface_form: "A".to_string(),
        normalized_form: "A".to_string(),
        normalization: "NFC".to_string(),
        unicode_scalars: vec!["U+0041".to_string()],
        script: "Latn".to_string(),
        case: "uppercase".to_string(),
        previous_revision_cid: None,
    };
    c.bench_function("cid_uppercase_revision_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&rev_a).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });

    // 8. Compute one punctuation revision CID ('!')
    let rev_excl = GraphemeRevision {
        schema: "language-graph/grapheme-revision/v1".to_string(),
        entity_id: "urn:language-graph:grapheme:nfc:0021".to_string(),
        kind: "grapheme".to_string(),
        surface_form: "!".to_string(),
        normalized_form: "!".to_string(),
        normalization: "NFC".to_string(),
        unicode_scalars: vec!["U+0021".to_string()],
        script: "Common".to_string(),
        case: "none".to_string(),
        previous_revision_cid: None,
    };
    c.bench_function("cid_punctuation_revision_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&rev_excl).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });

    // 9. Compute one collection snapshot CID (Uppercase alphabet snap - 26 members)
    let mut members = Vec::new();
    for i in 0..26 {
        members.push(SnapshotMember {
            position: (i + 1) as i32,
            entity_id: format!("urn:language-graph:grapheme:nfc:{:04x}", (b'A' + i) as u32),
            revision_cid: "bafyreievw56s5ltwde2xmmxzt3etkfz73qsutx43x7xuxe3iuvnbpobm2e".to_string(),
        });
    }
    let snap_upper = AlphabetSnapshot {
        schema: "language-graph/collection-snapshot/v1".to_string(),
        collection_entity_id: "urn:language-graph:collection:latin-uppercase-a-z".to_string(),
        kind: "ordered-grapheme-collection".to_string(),
        label: "Latin uppercase alphabet A-Z".to_string(),
        members,
    };
    c.bench_function("cid_collection_snapshot_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&snap_upper).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });

    // 10. Compute the full Basic English Written Text Profile snapshot CID (5 members)
    let profile = TextProfileSnapshot {
        schema: "language-graph/text-profile-snapshot/v1".to_string(),
        profile_entity_id: "urn:language-graph:profile:basic-english-written-text".to_string(),
        kind: "written-text-profile".to_string(),
        label: "Basic English Written Text Profile".to_string(),
        collections: vec![
            ProfileCollectionRef {
                position: 1,
                collection_entity_id: "urn:language-graph:collection:latin-lowercase-a-z"
                    .to_string(),
                snapshot_cid: "bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm"
                    .to_string(),
            },
            ProfileCollectionRef {
                position: 2,
                collection_entity_id: "urn:language-graph:collection:latin-uppercase-a-z"
                    .to_string(),
                snapshot_cid: "bafyreie5eeznusjiimg7l666feoxwvvk62pbhs6hb7cryh2ana53gm2uqm"
                    .to_string(),
            },
            ProfileCollectionRef {
                position: 3,
                collection_entity_id: "urn:language-graph:collection:decimal-digits-0-9"
                    .to_string(),
                snapshot_cid: "bafyreig3oqpkm3gpsmcs7f25u4vmyvkzczwhjqfvs4iumwpi5uelb353s4"
                    .to_string(),
            },
            ProfileCollectionRef {
                position: 4,
                collection_entity_id: "urn:language-graph:collection:basic-english-whitespace"
                    .to_string(),
                snapshot_cid: "bafyreidh3pi73vtvgkpt7gbbpbbx2lbi42yracmlnagtfljkfmi4mv36ky"
                    .to_string(),
            },
            ProfileCollectionRef {
                position: 5,
                collection_entity_id: "urn:language-graph:collection:basic-english-punctuation"
                    .to_string(),
                snapshot_cid: "bafyreidh6nez45kqwkc5ue6c5fvhcblmtehlbae5ubiafmt77rpazrzyuq"
                    .to_string(),
            },
        ],
    };
    c.bench_function("cid_profile_snapshot_computation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&profile).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });
}

criterion_group!(benches, bench_resolver_ops);
criterion_main!(benches);
