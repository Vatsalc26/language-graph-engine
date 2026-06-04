use rusqlite::Connection;
use language_graph_engine::db::migrations::run_migrations;
use language_graph_engine::seed::lowercase_latin::seed_lowercase_latin;
use language_graph_engine::resolver::text::TextResolver;

fn setup_resolver() -> TextResolver {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory database");
    run_migrations(&conn).expect("Failed to run migrations");
    seed_lowercase_latin(&mut conn).expect("Failed to seed database");
    TextResolver::load(&conn).expect("Failed to load resolver")
}

#[test]
fn test_resolve_vatsal() {
    let resolver = setup_resolver();
    let result = resolver.resolve("vatsal").expect("Failed to resolve vatsal");
    assert_eq!(result.input, "vatsal");
    assert_eq!(result.output, "vatsal");
    assert_eq!(result.trace.len(), 6);
    
    // Check tracing: 'vatsal' has repeated 'a' (index 1 and index 4)
    assert_eq!(result.trace[0].input_grapheme, "v");
    assert_eq!(result.trace[0].status, "Resolved");
    assert_eq!(result.trace[1].input_grapheme, "a");
    assert_eq!(result.trace[1].status, "Resolved");
    assert_eq!(result.trace[2].input_grapheme, "t");
    assert_eq!(result.trace[2].status, "Resolved");
    assert_eq!(result.trace[3].input_grapheme, "s");
    assert_eq!(result.trace[3].status, "Resolved");
    assert_eq!(result.trace[4].input_grapheme, "a");
    assert_eq!(result.trace[4].status, "Reused");
    assert_eq!(result.trace[5].input_grapheme, "l");
    assert_eq!(result.trace[5].status, "Resolved");
}

#[test]
fn test_resolve_banana() {
    let resolver = setup_resolver();
    let result = resolver.resolve("banana").expect("Failed to resolve banana");
    assert_eq!(result.input, "banana");
    assert_eq!(result.output, "banana");
    assert_eq!(result.trace.len(), 6);

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
fn test_resolve_orchestration() {
    let resolver = setup_resolver();
    let result = resolver.resolve("orchestration").expect("Failed to resolve orchestration");
    assert_eq!(result.input, "orchestration");
    assert_eq!(result.output, "orchestration");
}

#[test]
fn test_resolver_validation_errors() {
    let resolver = setup_resolver();

    // 1. Empty input
    let err_empty = resolver.resolve("").unwrap_err();
    assert!(err_empty.to_string().contains("Input text cannot be empty"));

    // 2. Uppercase input
    let err_upper = resolver.resolve("Vatsal").unwrap_err();
    assert!(err_upper.to_string().contains("Unsupported character or grapheme: 'V'"));

    // 3. Digits
    let err_digit = resolver.resolve("banana1").unwrap_err();
    assert!(err_digit.to_string().contains("Unsupported character or grapheme: '1'"));

    // 4. Spaces
    let err_space = resolver.resolve("hello world").unwrap_err();
    assert!(err_space.to_string().contains("Unsupported character or grapheme: ' '"));

    // 5. Punctuation
    let err_punct = resolver.resolve("hello!").unwrap_err();
    assert!(err_punct.to_string().contains("Unsupported character or grapheme: '!'"));

    // 6. Unsupported multi-byte Unicode or combining character
    let err_emoji = resolver.resolve("hello👋").unwrap_err();
    assert!(err_emoji.to_string().contains("Unsupported character or grapheme: '👋'"));

    // 7. Combining characters (like a + combining diaeresis: ä represented as two unicode codepoints in 1 grapheme cluster)
    // "a\u{0308}" yields a single grapheme cluster "ä" (with two unicode codepoints)
    let err_combining = resolver.resolve("a\u{0308}").unwrap_err();
    assert!(err_combining.to_string().contains("Unsupported character or grapheme: 'a\u{0308}'"));
}
