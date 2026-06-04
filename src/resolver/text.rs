use crate::db::repository::Repository;
use crate::error::Error;
use crate::seed::lowercase_latin::COLLECTION_ENTITY_ID as LOW_COL_ID;

pub const PROFILE_ENTITY_ID: &str = "urn:language-graph:profile:basic-english-written-text";
pub const PROFILE_2_1_ENTITY_ID: &str = "urn:language-graph:profile:printable-ascii-text";

use std::collections::{HashMap, HashSet};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphemeCachedInfo {
    pub entity_id: String,
    pub revision_cid: String,
    pub surface_form: String,
    pub display_name: String,
    pub category: String,
    pub source_collection_entity_id: String,
    pub source_collection_snapshot_cid: String,
}

#[derive(Clone, Debug)]
pub struct TextResolver {
    pub active_snapshot_cid: String,
    pub is_profile: bool,
    pub cache: HashMap<String, GraphemeCachedInfo>,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionStep {
    pub position: usize,
    pub input_grapheme: String,
    pub entity_id: String,
    pub revision_cid: String,
    pub surface_form: String,
    pub status: String, // "Resolved" or "Reused"
    pub display_name: String,
    pub category: String,
    pub source_collection_entity_id: String,
    pub source_collection_snapshot_cid: String,
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionResult {
    pub input: String,
    pub output: String,
    pub collection_snapshot_cid: String,
    pub trace: Vec<ResolutionStep>,
}

pub fn get_display_name(grapheme: &str) -> String {
    match grapheme {
        " " => "SPACE".to_string(),
        "." => "FULL STOP".to_string(),
        "," => "COMMA".to_string(),
        "?" => "QUESTION MARK".to_string(),
        "!" => "EXCLAMATION MARK".to_string(),
        "'" => "APOSTROPHE".to_string(),
        "\"" => "QUOTATION MARK".to_string(),
        "-" => "HYPHEN-MINUS".to_string(),
        ":" => "COLON".to_string(),
        ";" => "SEMICOLON".to_string(),
        "(" => "LEFT PARENTHESIS".to_string(),
        ")" => "RIGHT PARENTHESIS".to_string(),
        "#" => "NUMBER SIGN".to_string(),
        "$" => "DOLLAR SIGN".to_string(),
        "%" => "PERCENT SIGN".to_string(),
        "&" => "AMPERSAND".to_string(),
        "*" => "ASTERISK".to_string(),
        "+" => "PLUS SIGN".to_string(),
        "/" => "SOLIDUS".to_string(),
        "<" => "LESS-THAN SIGN".to_string(),
        "=" => "EQUALS SIGN".to_string(),
        ">" => "GREATER-THAN SIGN".to_string(),
        "@" => "COMMERCIAL AT".to_string(),
        "[" => "LEFT SQUARE BRACKET".to_string(),
        "\\" => "REVERSE SOLIDUS".to_string(),
        "]" => "RIGHT SQUARE BRACKET".to_string(),
        "^" => "CIRCUMFLEX ACCENT".to_string(),
        "_" => "LOW LINE".to_string(),
        "`" => "GRAVE ACCENT".to_string(),
        "{" => "LEFT CURLY BRACKET".to_string(),
        "|" => "VERTICAL LINE".to_string(),
        "}" => "RIGHT CURLY BRACKET".to_string(),
        "~" => "TILDE".to_string(),
        c if c.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false) => {
            let digit_names = ["ZERO", "ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX", "SEVEN", "EIGHT", "NINE"];
            let idx = c.chars().next().unwrap() as usize - '0' as usize;
            format!("DIGIT {}", digit_names[idx])
        }
        c if c.chars().next().map(|ch| ch.is_ascii_uppercase()).unwrap_or(false) => {
            let ch = c.chars().next().unwrap();
            format!("LATIN CAPITAL LETTER {}", ch)
        }
        c if c.chars().next().map(|ch| ch.is_ascii_lowercase()).unwrap_or(false) => {
            let ch = c.chars().next().unwrap();
            format!("LATIN SMALL LETTER {}", ch)
        }
        _ => "UNKNOWN".to_string(),
    }
}

pub fn get_category(grapheme: &str) -> String {
    match grapheme {
        " " => "whitespace".to_string(),
        "." | "," | "?" | "!" | "'" | "\"" | "-" | ":" | ";" | "(" | ")" => "punctuation".to_string(),
        "#" | "$" | "%" | "&" | "*" | "+" | "/" | "<" | "=" | ">" | "@" | "[" | "\\" | "]" | "^" | "_" | "`" | "{" | "|" | "}" | "~" => {
            "ascii-supplemental-symbol".to_string()
        }
        c if c.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(false) => "digit".to_string(),
        c if c.chars().next().map(|ch| ch.is_alphabetic()).unwrap_or(false) => "letter".to_string(),
        _ => "unknown".to_string(),
    }
}

fn get_unsupported_char_name(ch: char) -> &'static str {
    match ch {
        '\t' => "TAB",
        '\n' => "NEWLINE",
        '\r' => "CARRIAGE RETURN",
        '\u{00A0}' => "NON-BREAKING SPACE",
        '’' => "RIGHT SINGLE QUOTATION MARK",
        '‘' => "LEFT SINGLE QUOTATION MARK",
        '“' => "LEFT DOUBLE QUOTATION MARK",
        '”' => "RIGHT DOUBLE QUOTATION MARK",
        '–' => "EN DASH",
        '—' => "EM DASH",
        '…' => "HORIZONTAL ELLIPSIS",
        'é' => "LATIN SMALL LETTER E WITH ACUTE",
        _ => "UNSUPPORTED SYMBOL",
    }
}

impl TextResolver {
    pub fn load(conn: &rusqlite::Connection) -> Result<Self, Error> {
        let repo = Repository::new(conn);

        // 1. Try to load the active Printable ASCII profile snapshot first
        if let Some(active_profile_cid) = repo.get_active_profile_snapshot_cid(PROFILE_2_1_ENTITY_ID)? {
            let profile = repo.get_profile_snapshot(&active_profile_cid)?;
            let mut cache = HashMap::new();

            for col_ref in &profile.collections {
                let members = repo.get_snapshot_members(&col_ref.snapshot_cid)?;
                for member in members {
                    let rev = repo.get_grapheme_revision(&member.revision_cid)?;
                    let surface = rev.surface_form.clone();
                    cache.insert(
                        surface.clone(),
                        GraphemeCachedInfo {
                            entity_id: member.entity_id,
                            revision_cid: member.revision_cid,
                            surface_form: surface.clone(),
                            display_name: get_display_name(&surface),
                            category: get_category(&surface),
                            source_collection_entity_id: col_ref.collection_entity_id.clone(),
                            source_collection_snapshot_cid: col_ref.snapshot_cid.clone(),
                        },
                    );
                }
            }

            return Ok(Self {
                active_snapshot_cid: active_profile_cid,
                is_profile: true,
                cache,
            });
        }

        // 2. Try to load the Phase 2 Basic English written text profile
        if let Some(active_profile_cid) = repo.get_active_profile_snapshot_cid(PROFILE_ENTITY_ID)? {
            let profile = repo.get_profile_snapshot(&active_profile_cid)?;
            let mut cache = HashMap::new();

            for col_ref in &profile.collections {
                let members = repo.get_snapshot_members(&col_ref.snapshot_cid)?;
                for member in members {
                    let rev = repo.get_grapheme_revision(&member.revision_cid)?;
                    let surface = rev.surface_form.clone();
                    cache.insert(
                        surface.clone(),
                        GraphemeCachedInfo {
                            entity_id: member.entity_id,
                            revision_cid: member.revision_cid,
                            surface_form: surface.clone(),
                            display_name: get_display_name(&surface),
                            category: get_category(&surface),
                            source_collection_entity_id: col_ref.collection_entity_id.clone(),
                            source_collection_snapshot_cid: col_ref.snapshot_cid.clone(),
                        },
                    );
                }
            }

            return Ok(Self {
                active_snapshot_cid: active_profile_cid,
                is_profile: true,
                cache,
            });
        }

        // 3. Fallback to lowercase alphabet if no profile is active (Phase 1 legacy compatibility)
        let active_cid = repo
            .get_active_snapshot_cid(LOW_COL_ID)?
            .ok_or_else(|| {
                Error::NotFoundError(
                    "No active snapshot found for lowercase latin alphabet".to_string(),
                )
            })?;

        let members = repo.get_snapshot_members(&active_cid)?;
        let mut cache = HashMap::new();
        for member in members {
            let rev = repo.get_grapheme_revision(&member.revision_cid)?;
            let surface = rev.surface_form.clone();
            cache.insert(
                surface.clone(),
                GraphemeCachedInfo {
                    entity_id: member.entity_id,
                    revision_cid: member.revision_cid,
                    surface_form: surface.clone(),
                    display_name: get_display_name(&surface),
                    category: get_category(&surface),
                    source_collection_entity_id: LOW_COL_ID.to_string(),
                    source_collection_snapshot_cid: active_cid.clone(),
                },
            );
        }

        Ok(Self {
            active_snapshot_cid: active_cid,
            is_profile: false,
            cache,
        })
    }

    pub fn resolve(&self, input: &str) -> Result<ResolutionResult, Error> {
        if input.is_empty() {
            return Err(Error::ValidationError(
                "Input text cannot be empty".to_string(),
            ));
        }

        // Segment into graphemes
        let graphemes: Vec<&str> = input.graphemes(true).collect();
        let mut trace = Vec::new();
        let mut seen_graphemes = HashSet::new();
        let mut output = String::new();

        // Perform strict validation first
        let mut unsupported = Vec::new();
        for (idx, &g) in graphemes.iter().enumerate() {
            if !self.cache.contains_key(g) {
                let chars: Vec<char> = g.chars().collect();
                let name_info = if let Some(&ch) = chars.first() {
                    let hex = format!("U+{:04X}", ch as u32);
                    let name = get_unsupported_char_name(ch);
                    format!("{} {} {}", g, hex, name)
                } else {
                    format!("{}", g)
                };
                unsupported.push(format!("{} at position {}", name_info, idx + 1));
            }
        }
        if !unsupported.is_empty() {
            return Err(Error::ValidationError(format!(
                "Unsupported character or grapheme: {}",
                unsupported.join(", ")
            )));
        }

        for (idx, &g) in graphemes.iter().enumerate() {
            let cached_info = self.cache.get(g).unwrap();

            let is_new = seen_graphemes.insert(g.to_string());
            let status = if is_new {
                "Resolved".to_string()
            } else {
                "Reused".to_string()
            };

            trace.push(ResolutionStep {
                position: idx + 1,
                input_grapheme: g.to_string(),
                entity_id: cached_info.entity_id.clone(),
                revision_cid: cached_info.revision_cid.clone(),
                surface_form: cached_info.surface_form.clone(),
                status,
                display_name: cached_info.display_name.clone(),
                category: cached_info.category.clone(),
                source_collection_entity_id: cached_info.source_collection_entity_id.clone(),
                source_collection_snapshot_cid: cached_info.source_collection_snapshot_cid.clone(),
            });

            output.push_str(&cached_info.surface_form);
        }

        Ok(ResolutionResult {
            input: input.to_string(),
            output,
            collection_snapshot_cid: self.active_snapshot_cid.clone(),
            trace,
        })
    }
}
