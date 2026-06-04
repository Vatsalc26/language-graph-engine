use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::resolver::text::TextResolver;
use language_graph_engine::seed::lowercase_latin::seed_lowercase_latin;
use rusqlite::Connection;
use std::sync::Arc;

fn get_resolver() -> TextResolver {
    let mut conn = Connection::open_in_memory().expect("Failed to open database");
    run_migrations(&conn).expect("Migrations");
    seed_lowercase_latin(&mut conn).expect("Seeding");
    TextResolver::load(&conn).expect("Load resolver")
}

#[tokio::test]
async fn test_concurrent_resolutions() {
    let resolver = Arc::new(get_resolver());
    let mut tasks = Vec::new();

    let test_inputs = [
        "banana",
        "vatsal",
        "orchestration",
        "zzzzzz",
        "abcdefghijklmnopqrstuvwxyz",
    ];

    // Spawn 100 concurrent tasks (20 rounds of the 5 test inputs)
    for i in 0..100 {
        let resolver_clone = Arc::clone(&resolver);
        let input = test_inputs[i % test_inputs.len()];

        let handle = tokio::spawn(async move {
            let result = resolver_clone.resolve(input);
            assert!(
                result.is_ok(),
                "Concurrent resolution failed for input: {}",
                input
            );
            let res = result.unwrap();
            assert_eq!(res.output, input);
            assert_eq!(
                res.collection_snapshot_cid,
                resolver_clone.active_snapshot_cid
            );
        });
        tasks.push(handle);
    }

    // Wait for all tasks to complete successfully without panics
    for task in tasks {
        task.await
            .expect("Task panicked during concurrent resolution");
    }
}
