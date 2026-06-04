use unicode_normalization::UnicodeNormalization;

#[derive(Debug, PartialEq, Eq)]
pub enum Classification {
    Eligible,
    Deferred {
        reason_code: &'static str,
        reason_detail: String,
    },
}

pub fn classify_word(surface: &str) -> Classification {
    let normalized: String = surface.nfc().collect();
    if normalized.is_empty() {
        return Classification::Deferred {
            reason_code: "malformed_or_empty",
            reason_detail: "Entry is empty or malformed.".to_string(),
        };
    }

    // 1. Non-ASCII check
    if !normalized.is_ascii() {
        return Classification::Deferred {
            reason_code: "unsupported_non_ascii",
            reason_detail: "Contains non-ASCII characters.".to_string(),
        };
    }

    // 2. Spaces / Multi-word
    if normalized.chars().any(|c| c.is_whitespace()) {
        return Classification::Deferred {
            reason_code: "contains_space_or_multiword",
            reason_detail: "Contains space or is a multi-word lexical unit.".to_string(),
        };
    }

    // 3. Digits
    if normalized.chars().any(|c| c.is_ascii_digit()) {
        return Classification::Deferred {
            reason_code: "contains_digits_or_alphanumeric_structure",
            reason_detail: "Contains numeric digits or alphanumeric structure.".to_string(),
        };
    }

    // 4. Check for leading/trailing or consecutive hyphens/apostrophes
    if normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.starts_with('\'')
        || normalized.ends_with('\'')
    {
        return Classification::Deferred {
            reason_code: "abbreviation_or_special_form",
            reason_detail: "Contains leading/trailing hyphen or apostrophe.".to_string(),
        };
    }

    // Consecutive punct or punctuation that is not apostrophe or hyphen
    let mut state = 0; // 0 = start/letter, 1 = punct
    for c in normalized.chars() {
        if c.is_ascii_alphabetic() {
            state = 0;
        } else if c == '\'' || c == '-' {
            if state == 1 {
                return Classification::Deferred {
                    reason_code: "abbreviation_or_special_form",
                    reason_detail: "Contains consecutive hyphens or apostrophes.".to_string(),
                };
            }
            state = 1;
        } else {
            // Other symbols
            if c == '.' {
                return Classification::Deferred {
                    reason_code: "abbreviation_or_special_form",
                    reason_detail: "Contains abbreviation punctuation (full stop).".to_string(),
                };
            } else {
                return Classification::Deferred {
                    reason_code: "contains_disallowed_punctuation",
                    reason_detail: format!("Contains disallowed punctuation: '{}'", c),
                };
            }
        }
    }

    // Double check eligibility with policy
    if crate::written_forms::policy::is_eligible(&normalized) {
        Classification::Eligible
    } else {
        Classification::Deferred {
            reason_code: "abbreviation_or_special_form",
            reason_detail: "Rejected by wordform admission policy.".to_string(),
        }
    }
}
