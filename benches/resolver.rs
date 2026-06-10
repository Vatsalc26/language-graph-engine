use criterion::{criterion_group, criterion_main, Criterion};
use language_graph_engine::content::cid::compute_cid;
use language_graph_engine::content::encoding::to_dag_cbor;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::model::{
    AlphabetSnapshot, GraphemeRevision, ProfileCollectionRef, SnapshotMember, TextProfileSnapshot,
};
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::ascii_supplemental::seed_phase2_1;
use language_graph_engine::written_forms::{
    find_written_form_exact, get_written_form_details, list_written_forms, preview_written_form,
    publish_store_snapshot, save_written_form, STORE_ENTITY_ID,
};
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
    let medium_input =
        "Hello, #Vatsal! Room 101: Is this working? (Phase 2.1) - 2026! +~` ".repeat(15);
    c.bench_function("resolver_medium_1k_chars", |b| {
        b.iter(|| resolver.resolve(&medium_input).unwrap())
    });

    // 4. Resolve a larger valid input (~100,000 graphemes)
    let large_input =
        "Hello, #Vatsal! Room 101: Is this working? (Phase 2.1) - 2026! +~` ".repeat(1500);
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

    // --- Phase 3 Benchmarks ---

    // 11. Preview composition of bank
    c.bench_function("preview_composition_bank", |b| {
        b.iter(|| {
            let res = preview_written_form(&resolver, &_conn, "bank").unwrap();
            assert!(res.is_eligible);
        })
    });

    // 12. Idempotent save of an existing written form
    let mut conn_save = Connection::open_in_memory().unwrap();
    run_migrations(&conn_save).unwrap();
    seed_phase2_1(&mut conn_save).unwrap();
    let resolver_save = TextResolver::load(&conn_save).unwrap();
    save_written_form(&resolver_save, &mut conn_save, "bank").unwrap();
    c.bench_function("save_idempotent_existing_written_form", |b| {
        b.iter(|| {
            let res = save_written_form(&resolver_save, &mut conn_save, "bank").unwrap();
            assert_eq!(res.status, "Already Stored");
        })
    });

    // 13. Explicit save of a new written form (using a counter to keep it unique)
    let mut counter = 0;
    c.bench_function("save_new_written_form", |b| {
        b.iter(|| {
            counter += 1;
            let mut word = String::new();
            let mut temp = counter;
            while temp > 0 {
                let rem = (temp % 26) as u8;
                word.push((b'a' + rem) as char);
                temp /= 26;
            }
            if word.is_empty() {
                word.push('a');
            }
            let res = save_written_form(&resolver_save, &mut conn_save, &word).unwrap();
            assert_eq!(res.status, "Created");
        })
    });

    // 14. Exact indexed retrieval of bank
    c.bench_function("exact_retrieval_bank", |b| {
        b.iter(|| {
            let res = find_written_form_exact(&conn_save, "bank")
                .unwrap()
                .unwrap();
            assert_eq!(res.surface_form, "bank");
        })
    });

    // 15. Detail retrieval including component trace
    c.bench_function("detail_retrieval_bank", |b| {
        b.iter(|| {
            let res = get_written_form_details(&conn_save, "bank")
                .unwrap()
                .unwrap();
            assert_eq!(res.surface_form, "bank");
        })
    });

    // 16. Paginated listing of a populated test store (100 seeded words)
    let mut conn_list = Connection::open_in_memory().unwrap();
    run_migrations(&conn_list).unwrap();
    seed_phase2_1(&mut conn_list).unwrap();
    let resolver_list = TextResolver::load(&conn_list).unwrap();
    for i in 0..100 {
        let mut word = String::new();
        let mut temp = i + 1;
        while temp > 0 {
            let rem = (temp % 26) as u8;
            word.push((b'a' + rem) as char);
            temp /= 26;
        }
        save_written_form(&resolver_list, &mut conn_list, &word).unwrap();
    }
    c.bench_function("paginated_listing_store", |b| {
        b.iter(|| {
            let res = list_written_forms(&conn_list, STORE_ENTITY_ID, 20, 40).unwrap();
            assert_eq!(res.len(), 20);
        })
    });

    // 17. Publishing a deterministic written-form store snapshot
    c.bench_function("publish_store_snapshot", |b| {
        b.iter(|| {
            let res = publish_store_snapshot(&mut conn_list).unwrap();
            assert!(!res.snapshot_cid.is_empty());
        })
    });

    // --- Phase 3.1 Lexicon Import Benchmarks ---

    // 18. Dry-run classification of a small fixture
    let small_fixture = b"bank\ncan't\nmother-in-law\ncaf\xc3\xa9\nice cream\nCOVID-19\n";
    c.bench_function("import_dry_run_classification_small_fixture", |b| {
        b.iter(|| {
            let res = language_graph_engine::lexicon_import::importer::analyze_esdb_file(
                &conn_save,
                small_fixture,
                None,
            )
            .unwrap();
            assert_eq!(res.entries_read, 6);
        })
    });

    // 19. Bulk import of 1,000 generated eligible words
    let fixture_1k = generate_synthetic_wordlist(1000);
    c.bench_function("import_bulk_1k_words", |b| {
        b.iter_with_setup(
            || {
                let mut conn_temp = Connection::open_in_memory().unwrap();
                run_migrations(&conn_temp).unwrap();
                seed_phase2_1(&mut conn_temp).unwrap();
                let resolver_temp = TextResolver::load(&conn_temp).unwrap();
                (conn_temp, resolver_temp)
            },
            |(mut conn_temp, resolver_temp)| {
                let res = language_graph_engine::lexicon_import::importer::import_eligible_words(
                    &mut conn_temp,
                    &resolver_temp,
                    &fixture_1k,
                    None,
                )
                .unwrap();
                assert_eq!(res.eligible_new_words, 1000);
            },
        )
    });

    // 20. Bulk import of a larger synthetic inventory approximating 100,000 candidates
    let mut group = c.benchmark_group("lexicon_import");
    group.sample_size(10);
    let fixture_100k = generate_synthetic_wordlist(100000);
    group.bench_function("import_bulk_100k_words", |b| {
        b.iter_with_setup(
            || {
                let mut conn_temp = Connection::open_in_memory().unwrap();
                run_migrations(&conn_temp).unwrap();
                seed_phase2_1(&mut conn_temp).unwrap();
                let resolver_temp = TextResolver::load(&conn_temp).unwrap();
                (conn_temp, resolver_temp)
            },
            |(mut conn_temp, resolver_temp)| {
                let res = language_graph_engine::lexicon_import::importer::import_eligible_words(
                    &mut conn_temp,
                    &resolver_temp,
                    &fixture_100k,
                    None,
                )
                .unwrap();
                assert_eq!(res.eligible_new_words, 100000);
            },
        )
    });

    // 21. Re-import/idempotency path where all words already exist
    let mut conn_idem = Connection::open_in_memory().unwrap();
    run_migrations(&conn_idem).unwrap();
    seed_phase2_1(&mut conn_idem).unwrap();
    let resolver_idem = TextResolver::load(&conn_idem).unwrap();
    let fixture_idem = generate_synthetic_wordlist(1000);
    language_graph_engine::lexicon_import::importer::import_eligible_words(
        &mut conn_idem,
        &resolver_idem,
        &fixture_idem,
        None,
    )
    .unwrap();
    group.bench_function("import_idem_1k_words_already_exist", |b| {
        b.iter(|| {
            let res = language_graph_engine::lexicon_import::importer::import_eligible_words(
                &mut conn_idem,
                &resolver_idem,
                &fixture_idem,
                None,
            )
            .unwrap();
            assert_eq!(res.eligible_new_words, 0);
        })
    });

    // 22. Indexed exact lookup after a large import (10k words)
    let mut conn_large = Connection::open_in_memory().unwrap();
    run_migrations(&conn_large).unwrap();
    seed_phase2_1(&mut conn_large).unwrap();
    let resolver_large = TextResolver::load(&conn_large).unwrap();
    let fixture_large = generate_synthetic_wordlist(10000);
    language_graph_engine::lexicon_import::importer::import_eligible_words(
        &mut conn_large,
        &resolver_large,
        &fixture_large,
        None,
    )
    .unwrap();
    group.bench_function("exact_lookup_after_10k_import", |b| {
        b.iter(|| {
            let res = find_written_form_exact(&conn_large, "b").unwrap().unwrap();
            assert_eq!(res.surface_form, "b");
        })
    });

    // 23. Paginated store listing after a large import (10k words)
    group.bench_function("paginated_listing_after_10k_import", |b| {
        b.iter(|| {
            let res = list_written_forms(&conn_large, STORE_ENTITY_ID, 50, 5000).unwrap();
            assert_eq!(res.len(), 50);
        })
    });

    // 24. Immutable store-snapshot publication after large membership (10k words)
    group.bench_function("publish_snapshot_after_10k_import", |b| {
        b.iter(|| {
            let res = publish_store_snapshot(&mut conn_large).unwrap();
            assert_eq!(res.status, "No Changes");
        })
    });

    // 25. Import-manifest CID creation
    let manifest = language_graph_engine::lexicon_import::importer::LexiconImportManifest {
        schema: "language-graph/lexicon-import-manifest/v1".to_string(),
        source_entity_id:
            "urn:language-graph:lexicon-source:esdb:en-us:rel-2026.02.25:size-60:default-variants"
                .to_string(),
        source_release_tag: "rel-2026.02.25".to_string(),
        source_file_name: "en_US.txt".to_string(),
        source_file_sha256: "abc123digest".to_string(),
        source_entry_count: 109902,
        admission_policy: "ascii-letters-with-internal-apostrophe-or-hyphen-v1".to_string(),
        accepted_new_count: 100000,
        accepted_reused_count: 9902,
        deferred_count: 0,
        rejected_or_malformed_count: 0,
        resulting_store_snapshot_cid: "bafyreihjhfma5buevawmzrchj3bauifuvc5ey5b2x4fgfsjmiqqease33u"
            .to_string(),
    };
    group.bench_function("manifest_cid_creation", |b| {
        b.iter(|| {
            let bytes = to_dag_cbor(&manifest).unwrap();
            let _cid = compute_cid(&bytes).unwrap();
        })
    });

    group.finish();
}

fn generate_synthetic_wordlist(count: usize) -> Vec<u8> {
    let mut out = String::new();
    for i in 0..count {
        let mut word = String::new();
        let mut temp = i + 1;
        while temp > 0 {
            let rem = (temp % 26) as u8;
            word.push((b'a' + rem) as char);
            temp /= 26;
        }
        out.push_str(&word);
        out.push('\n');
    }
    out.into_bytes()
}

criterion_group!(benches, bench_resolver_ops);
criterion_main!(benches);
