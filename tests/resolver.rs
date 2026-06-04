use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::lowercase_latin::seed_lowercase_latin;
use proptest::prelude::*;
use rusqlite::Connection;
use std::sync::OnceLock;

static RESOLVER: OnceLock<TextResolver> = OnceLock::new();

fn get_global_resolver() -> &'static TextResolver {
    RESOLVER.get_or_init(|| {
        let mut conn = Connection::open_in_memory().expect("Failed to open database");
        run_migrations(&conn).expect("Migrations");
        seed_lowercase_latin(&mut conn).expect("Seeding");
        TextResolver::load(&conn).expect("Load resolver")
    })
}

// Separate helper for non-mutation database checks
fn get_test_db_and_resolver() -> (Connection, TextResolver) {
    let mut conn = Connection::open_in_memory().expect("Failed to open database");
    run_migrations(&conn).expect("Migrations");
    seed_lowercase_latin(&mut conn).expect("Seeding");
    let resolver = TextResolver::load(&conn).expect("Load resolver");
    (conn, resolver)
}

#[test]
fn test_resolve_valid_cases() {
    let resolver = get_global_resolver();

    let cases = vec![
        "a",
        "vatsal",
        "banana",
        "orchestration",
        "abcdefghijklmnopqrstuvwxyz",
        "zzzzzzzz",
    ];

    for case in cases {
        let result = resolver
            .resolve(case)
            .unwrap_or_else(|e| panic!("Failed to resolve '{}': {:?}", case, e));
        assert_eq!(result.input, case);
        assert_eq!(result.output, case);
        assert_eq!(
            result.collection_snapshot_cid,
            "bafyreib4ivpoazb5skkr7yvfelvoowz6sxxncdsjewvxawyedm5tikeshm"
        );

        // Verify structure of trace rows
        for (i, step) in result.trace.iter().enumerate() {
            assert_eq!(step.position, i + 1);
            assert!(!step.entity_id.is_empty());
            assert!(!step.revision_cid.is_empty());
            assert_eq!(step.surface_form, step.input_grapheme);
        }
    }
}

#[test]
fn test_resolve_banana_reuse_logic() {
    let resolver = get_global_resolver();
    let result = resolver.resolve("banana").expect("Resolve banana");

    // b a n a n a
    // 1: b (Resolved)
    // 2: a (Resolved)
    // 3: n (Resolved)
    // 4: a (Reused)
    // 5: n (Reused)
    // 6: a (Reused)
    assert_eq!(result.trace[0].input_grapheme, "b");
    assert_eq!(result.trace[0].status, "Resolved");

    assert_eq!(result.trace[1].input_grapheme, "a");
    assert_eq!(result.trace[1].status, "Resolved");

    assert_eq!(result.trace[2].input_grapheme, "n");
    assert_eq!(result.trace[2].status, "Resolved");

    assert_eq!(result.trace[3].input_grapheme, "a");
    assert_eq!(result.trace[3].status, "Reused");

    assert_eq!(result.trace[4].input_grapheme, "n");
    assert_eq!(result.trace[4].status, "Reused");

    assert_eq!(result.trace[5].input_grapheme, "a");
    assert_eq!(result.trace[5].status, "Reused");
}

#[test]
fn test_resolver_non_mutation() {
    let (conn, resolver) = get_test_db_and_resolver();

    let get_row_counts = |c: &Connection| -> (i64, i64, i64, i64, i64, i64, i64) {
        let blocks: i64 = c
            .query_row("SELECT COUNT(*) FROM immutable_blocks", [], |r| r.get(0))
            .unwrap();
        let entities: i64 = c
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        let heads: i64 = c
            .query_row("SELECT COUNT(*) FROM entity_heads", [], |r| r.get(0))
            .unwrap();
        let collections: i64 = c
            .query_row("SELECT COUNT(*) FROM collections", [], |r| r.get(0))
            .unwrap();
        let snapshots: i64 = c
            .query_row("SELECT COUNT(*) FROM collection_snapshots", [], |r| {
                r.get(0)
            })
            .unwrap();
        let members: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM collection_snapshot_members",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let active_snaps: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM active_collection_snapshots",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (
            blocks,
            entities,
            heads,
            collections,
            snapshots,
            members,
            active_snaps,
        )
    };

    let counts_before = get_row_counts(&conn);

    // Run resolution
    let _ = resolver.resolve("banana").expect("Resolve");
    let _ = resolver.resolve("vatsal").expect("Resolve");
    let _ = resolver.resolve("orchestration").expect("Resolve");

    let counts_after = get_row_counts(&conn);

    assert_eq!(
        counts_before, counts_after,
        "Ordinary resolve operations must not modify database row counts!"
    );
}

#[test]
fn test_unicode_safe_validation_errors() {
    let resolver = get_global_resolver();

    let invalid_inputs = vec![
        "A",
        "Hello",
        "hello world",
        "123",
        "hello!",
        "é",
        "a\u{0308}", // a + combining diaeresis
        "a\u{0301}", // a + combining acute accent
        "🙂",
        "ß",
        "α",
        "中文",
        "\n",
        "\t",
    ];

    for input in invalid_inputs {
        let res = resolver.resolve(input);
        assert!(
            res.is_err(),
            "Input '{}' should have failed validation",
            input
        );
        let err_msg = format!("{:?}", res.err().unwrap());
        assert!(
            err_msg.contains("Unsupported character or grapheme"),
            "Unexpected error: {}",
            err_msg
        );
    }
}

// Property-based testing via proptest
proptest! {
    #[test]
    fn test_arbitrary_lowercase_strings(ref s in "[a-z]+") {
        let resolver = get_global_resolver();
        let result = resolver.resolve(s).unwrap();

        prop_assert_eq!(&result.output, s);
        prop_assert_eq!(result.trace.len(), s.chars().count());

        // 1. Number of unique resolved characters is <= min(unique graphemes, 26)
        let unique_graphemes: std::collections::HashSet<_> = s.chars().collect();
        let unique_resolved: std::collections::HashSet<_> = result.trace.iter()
            .filter(|step| step.status == "Resolved")
            .map(|step| &step.input_grapheme)
            .collect();
        prop_assert_eq!(unique_resolved.len(), unique_graphemes.len());

        // 2. All trace rows are valid
        for step in &result.trace {
            prop_assert!(step.position > 0);
            prop_assert!(!step.entity_id.is_empty());
            prop_assert!(!step.revision_cid.is_empty());
            prop_assert_eq!(&step.surface_form, &step.input_grapheme);
        }
    }
}
