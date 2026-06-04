use unicode_normalization::UnicodeNormalization;

/// Checks if a string is eligible for admission into the English Natural-Language Written Forms store.
///
/// The grammar policy requires the NFC-normalized surface form to match:
/// - One or more ASCII letters (A-Za-z)
/// - Followed by zero or more groups of:
///   - An internal apostrophe U+0027 (') or hyphen U+002D (-)
///   - Followed by one or more ASCII letters (A-Za-z)
///
/// Conceptual shape: [A-Za-z]+(?:['-][A-Za-z]+)*
pub fn is_eligible(surface: &str) -> bool {
    let normalized: String = surface.nfc().collect();
    if normalized.is_empty() {
        return false;
    }

    enum State {
        Start,
        Letter,
        Punct,
    }

    let mut state = State::Start;
    for c in normalized.chars() {
        match state {
            State::Start => {
                if c.is_ascii_alphabetic() {
                    state = State::Letter;
                } else {
                    return false;
                }
            }
            State::Letter => {
                if c.is_ascii_alphabetic() {
                    // stay in Letter state
                } else if c == '\'' || c == '-' {
                    state = State::Punct;
                } else {
                    return false;
                }
            }
            State::Punct => {
                if c.is_ascii_alphabetic() {
                    state = State::Letter;
                } else {
                    return false;
                }
            }
        }
    }

    matches!(state, State::Letter)
}
