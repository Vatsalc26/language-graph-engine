use crate::app::AppState;
use crate::db::repository::Repository;
use crate::error::Error;
use crate::model::TextProfileSnapshot;
use crate::seed::lowercase_latin::COLLECTION_ENTITY_ID as LOW_COL_ID;
use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use rusqlite::OptionalExtension;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

// Custom response wrapper to convert application Errors to HTTP responses
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Error::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            Error::NotFoundError(msg) => (StatusCode::NOT_FOUND, msg),
            Error::IntegrityError(msg) => (StatusCode::CONFLICT, msg),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

// Request and response types
#[derive(Deserialize)]
pub struct ResolveRequest {
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub active_snapshot_cid: String,
    pub symbol_count: usize,
    pub identifier_format: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub collection_entity_id: String,
    pub label: String,
    pub symbol_count: usize,
    pub snapshot_cid: String,
    pub status: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolSummary {
    pub position: i32,
    pub surface_form: String,
    pub canonical_entity_id: String,
    pub active_revision_cid: String,
    pub normalization: String,
    pub status: String,
    pub category: String,
    pub source_collection_entity_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDetailsResponse {
    pub entity_id: String,
    pub revision_cid: String,
    pub surface_form: String,
    pub normalized_form: String,
    pub normalization: String,
    pub unicode_scalars: Vec<String>,
    pub script: String,
    pub case: String,
    pub codec: String,
    pub multihash_format: String,
}

pub struct Server;

impl Server {
    pub fn build_router(state: AppState) -> Router {
        Router::new()
            .route("/api/status", get(get_status))
            .route("/api/symbols", get(list_symbols))
            .route("/api/symbols/:entity_id", get(get_symbol_details))
            .route("/api/snapshots/active", get(get_active_snapshot))
            .route("/api/resolve", post(resolve_text))
            .route("/api/collections", get(list_collections))
            .route("/api/profiles/active", get(get_active_profile))
            .route(
                "/api/word-stores/english-natural-language-written-forms",
                get(get_word_store_metadata),
            )
            .route("/api/wordforms/preview", post(preview_wordform))
            .route(
                "/api/wordforms",
                post(save_wordform).get(list_wordforms_route),
            )
            .route("/api/wordforms/exact", get(exact_wordform_lookup))
            .route("/api/wordforms/details", get(get_wordform_details_route))
            .route(
                "/api/word-stores/english-natural-language-written-forms/publish",
                post(publish_store_snapshot_route),
            )
            .route(
                "/api/word-stores/english-natural-language-written-forms/snapshots/active",
                get(get_active_store_snapshot_route),
            )
            .route(
                "/api/lexicon-import/esdb/analyze",
                post(analyze_esdb_import).layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)),
            )
            .route(
                "/api/lexicon-import/esdb/import",
                post(execute_esdb_import).layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)),
            )
            .route("/api/lexicon-import/sources", get(list_sources_route))
            .route("/api/lexicon-import/batches", get(list_batches_route))
            .route(
                "/api/lexicon-import/batches/:import_id",
                get(get_batch_details_route),
            )
            .route(
                "/api/lexicon-import/batches/:import_id/deferred",
                get(get_batch_deferred_route),
            )
            .fallback_service(ServeDir::new("public"))
            .layer(CorsLayer::permissive())
            .with_state(state)
    }

    pub async fn run(state: AppState) -> Result<(), Error> {
        let port = {
            let inner = state.0.lock().unwrap();
            inner.config.listen_port
        };

        let app = Self::build_router(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            Error::CidError(format!(
                "Failed to bind server to address {}: {:?}",
                addr, e
            ))
        })?;

        println!("Language Graph Engine running at http://localhost:{}", port);

        axum::serve(listener, app)
            .await
            .map_err(|e| Error::CidError(format!("Server execution failed: {:?}", e)))?;

        Ok(())
    }
}

// --- Route Handlers ---

async fn get_status(State(state): State<AppState>) -> Result<Json<StatusResponse>, Error> {
    let inner = state.0.lock().unwrap();
    Ok(Json(StatusResponse {
        active_snapshot_cid: inner.resolver.active_snapshot_cid.clone(),
        symbol_count: inner.resolver.cache.len(),
        identifier_format: "CIDv1 / DAG-CBOR / SHA2-256".to_string(),
    }))
}

async fn list_collections(
    State(state): State<AppState>,
) -> Result<Json<Vec<CollectionSummary>>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    let mut summaries = Vec::new();

    if inner.resolver.is_profile {
        let profile_collections =
            repo.get_profile_collections(&inner.resolver.active_snapshot_cid)?;
        for col_ref in profile_collections {
            let label: String = inner
                .conn
                .query_row(
                    "SELECT label FROM collections WHERE collection_entity_id = ?1",
                    [&col_ref.collection_entity_id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "Unknown Collection".to_string());

            let members = repo.get_snapshot_members(&col_ref.snapshot_cid)?;

            summaries.push(CollectionSummary {
                collection_entity_id: col_ref.collection_entity_id,
                label,
                symbol_count: members.len(),
                snapshot_cid: col_ref.snapshot_cid,
                status: "Healthy".to_string(),
            });
        }
    } else {
        let members = repo.get_snapshot_members(&inner.resolver.active_snapshot_cid)?;
        summaries.push(CollectionSummary {
            collection_entity_id: LOW_COL_ID.to_string(),
            label: "Latin lowercase alphabet a-z".to_string(),
            symbol_count: members.len(),
            snapshot_cid: inner.resolver.active_snapshot_cid.clone(),
            status: "Healthy".to_string(),
        });
    }

    Ok(Json(summaries))
}

async fn list_symbols(State(state): State<AppState>) -> Result<Json<Vec<SymbolSummary>>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    let mut summaries = Vec::new();
    let mut flat_pos = 1;

    if inner.resolver.is_profile {
        let collections = repo.get_profile_collections(&inner.resolver.active_snapshot_cid)?;
        for col_ref in collections {
            let members = repo.get_snapshot_members(&col_ref.snapshot_cid)?;
            for member in members {
                let rev = repo.get_grapheme_revision(&member.revision_cid)?;
                summaries.push(SymbolSummary {
                    position: flat_pos,
                    category: crate::resolver::text::get_category(&rev.surface_form),
                    surface_form: rev.surface_form,
                    canonical_entity_id: member.entity_id,
                    active_revision_cid: member.revision_cid,
                    normalization: rev.normalization,
                    status: "Healthy".to_string(),
                    source_collection_entity_id: col_ref.collection_entity_id.clone(),
                });
                flat_pos += 1;
            }
        }
    } else {
        let members = repo.get_snapshot_members(&inner.resolver.active_snapshot_cid)?;
        for member in members {
            let rev = repo.get_grapheme_revision(&member.revision_cid)?;
            summaries.push(SymbolSummary {
                position: member.position,
                surface_form: rev.surface_form,
                canonical_entity_id: member.entity_id,
                active_revision_cid: member.revision_cid,
                normalization: rev.normalization,
                status: "Healthy".to_string(),
                category: "letter".to_string(),
                source_collection_entity_id: LOW_COL_ID.to_string(),
            });
        }
    }

    Ok(Json(summaries))
}

async fn get_symbol_details(
    State(state): State<AppState>,
    Path(entity_id): Path<String>,
) -> Result<Json<SymbolDetailsResponse>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    let revision_cid = repo.get_entity_head(&entity_id)?.ok_or_else(|| {
        Error::NotFoundError(format!("No head revision found for entity {}", entity_id))
    })?;

    let rev = repo.get_grapheme_revision(&revision_cid)?;

    Ok(Json(SymbolDetailsResponse {
        entity_id: rev.entity_id,
        revision_cid,
        surface_form: rev.surface_form,
        normalized_form: rev.normalized_form,
        normalization: rev.normalization,
        unicode_scalars: rev.unicode_scalars,
        script: rev.script,
        case: rev.case,
        codec: "dag-cbor".to_string(),
        multihash_format: "sha2-256".to_string(),
    }))
}

async fn get_active_snapshot(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    if inner.resolver.is_profile {
        let bytes = repo
            .get_block_bytes(&inner.resolver.active_snapshot_cid)?
            .ok_or_else(|| Error::NotFoundError("Active snapshot block not found".to_string()))?;
        let val: serde_json::Value =
            serde_ipld_dagcbor::from_slice(&bytes).map_err(|e| Error::CborError(e.to_string()))?;
        Ok(Json(val))
    } else {
        let snapshot = repo.get_alphabet_snapshot(&inner.resolver.active_snapshot_cid)?;
        Ok(Json(serde_json::to_value(snapshot)?))
    }
}

async fn get_active_profile(
    State(state): State<AppState>,
) -> Result<Json<TextProfileSnapshot>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    if inner.resolver.is_profile {
        let snap = repo.get_profile_snapshot(&inner.resolver.active_snapshot_cid)?;
        Ok(Json(snap))
    } else {
        Err(Error::NotFoundError(
            "No active text profile snapshot found".to_string(),
        ))
    }
}

async fn resolve_text(
    State(state): State<AppState>,
    Json(payload): Json<ResolveRequest>,
) -> Result<Json<crate::resolver::text::ResolutionResult>, Error> {
    let inner = state.0.lock().unwrap();
    let result = inner.resolver.resolve(&payload.text)?;
    Ok(Json(result))
}

// --- Phase 3 Written Forms Types & Handlers ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreMetadataResponse {
    pub store_entity_id: String,
    pub canonical_key: String,
    pub label: String,
    pub store_kind: String,
    pub admission_policy: String,
    pub saved_word_count: usize,
    pub active_snapshot_cid: Option<String>,
}

#[derive(Deserialize)]
pub struct WordPreviewRequest {
    pub text: String,
}

#[derive(Deserialize)]
pub struct WordSaveRequest {
    pub text: String,
}

#[derive(Deserialize)]
pub struct ExactLookupParams {
    pub surface: String,
}

#[derive(Deserialize)]
pub struct ListWordformsParams {
    pub store: String,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct DetailsParams {
    pub surface: String,
}

async fn get_word_store_metadata(
    State(state): State<AppState>,
) -> Result<Json<StoreMetadataResponse>, Error> {
    let inner = state.0.lock().unwrap();
    let conn = &inner.conn;

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM written_form_store_members WHERE store_entity_id = ?1 AND status = 'active'",
        [crate::written_forms::STORE_ENTITY_ID],
        |row| row.get(0),
    )?;

    let active_snapshot_cid: Option<String> = conn
        .query_row(
            "SELECT snapshot_cid FROM active_written_form_store_snapshots WHERE store_entity_id = ?1",
            [crate::written_forms::STORE_ENTITY_ID],
            |row| row.get(0),
        )
        .optional()?;

    let (label, canonical_key, store_kind, admission_policy): (String, String, String, String) = conn.query_row(
        "SELECT label, canonical_key, store_kind, admission_policy FROM written_form_stores WHERE store_entity_id = ?1",
        [crate::written_forms::STORE_ENTITY_ID],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;

    Ok(Json(StoreMetadataResponse {
        store_entity_id: crate::written_forms::STORE_ENTITY_ID.to_string(),
        canonical_key,
        label,
        store_kind,
        admission_policy,
        saved_word_count: count as usize,
        active_snapshot_cid,
    }))
}

async fn preview_wordform(
    State(state): State<AppState>,
    Json(payload): Json<WordPreviewRequest>,
) -> Result<Json<crate::written_forms::PreviewResult>, Error> {
    let inner = state.0.lock().unwrap();
    let res =
        crate::written_forms::preview_written_form(&inner.resolver, &inner.conn, &payload.text)?;
    Ok(Json(res))
}

async fn save_wordform(
    State(state): State<AppState>,
    Json(payload): Json<WordSaveRequest>,
) -> Result<Json<crate::written_forms::SaveResult>, Error> {
    let mut inner = state.0.lock().unwrap();
    let resolver = inner.resolver.clone();
    let res = crate::written_forms::save_written_form(&resolver, &mut inner.conn, &payload.text)?;
    Ok(Json(res))
}

async fn exact_wordform_lookup(
    State(state): State<AppState>,
    Query(params): Query<ExactLookupParams>,
) -> Result<Json<crate::written_forms::StoredWrittenFormSummary>, Error> {
    let inner = state.0.lock().unwrap();
    let res = crate::written_forms::find_written_form_exact(&inner.conn, &params.surface)?
        .ok_or_else(|| {
            Error::NotFoundError(format!("Word '{}' not found in store", params.surface))
        })?;
    Ok(Json(res))
}

async fn list_wordforms_route(
    State(state): State<AppState>,
    Query(params): Query<ListWordformsParams>,
) -> Result<Json<Vec<crate::written_forms::StoredWrittenFormSummary>>, Error> {
    let inner = state.0.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let res = crate::written_forms::list_written_forms(&inner.conn, &params.store, limit, offset)?;
    Ok(Json(res))
}

async fn get_wordform_details_route(
    State(state): State<AppState>,
    Query(params): Query<DetailsParams>,
) -> Result<Json<crate::written_forms::WrittenFormDetails>, Error> {
    let inner = state.0.lock().unwrap();
    let res = crate::written_forms::get_written_form_details(&inner.conn, &params.surface)?
        .ok_or_else(|| {
            Error::NotFoundError(format!("Word details for '{}' not found", params.surface))
        })?;
    Ok(Json(res))
}

async fn publish_store_snapshot_route(
    State(state): State<AppState>,
) -> Result<Json<crate::written_forms::PublishResult>, Error> {
    let mut inner = state.0.lock().unwrap();
    let res = crate::written_forms::publish_store_snapshot(&mut inner.conn)?;
    Ok(Json(res))
}

async fn get_active_store_snapshot_route(
    State(state): State<AppState>,
) -> Result<Json<crate::model::WrittenFormStoreSnapshot>, Error> {
    let inner = state.0.lock().unwrap();
    let res = crate::written_forms::get_active_store_snapshot(&inner.conn)?.ok_or_else(|| {
        Error::NotFoundError("No active published store snapshot found".to_string())
    })?;
    Ok(Json(res))
}

#[derive(Deserialize)]
pub struct ImportPaginationParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn analyze_esdb_import(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<crate::lexicon_import::report::LexiconImportBatchResult>, Error> {
    let mut file_bytes = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::ValidationError(e.to_string()))?
    {
        if field.name() == Some("file") {
            file_bytes = field
                .bytes()
                .await
                .map_err(|e| Error::ValidationError(e.to_string()))?
                .to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err(Error::ValidationError("Missing file parameter".to_string()));
    }

    let inner = state.0.lock().unwrap();
    let expected_count = Some(109902);
    let expected_sha = Some("4ff7e0b6d86763e1e042ffd746e94cdf4432618702deac303a1669e2a838db04");
    let res = crate::lexicon_import::importer::analyze_esdb_file(
        &inner.conn,
        &file_bytes,
        expected_count,
        expected_sha,
    )?;
    Ok(Json(res))
}

async fn execute_esdb_import(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<crate::lexicon_import::report::LexiconImportBatchResult>, Error> {
    let mut file_bytes = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| Error::ValidationError(e.to_string()))?
    {
        if field.name() == Some("file") {
            file_bytes = field
                .bytes()
                .await
                .map_err(|e| Error::ValidationError(e.to_string()))?
                .to_vec();
            break;
        }
    }

    if file_bytes.is_empty() {
        return Err(Error::ValidationError("Missing file parameter".to_string()));
    }

    let mut inner = state.0.lock().unwrap();
    let expected_count = Some(109902);
    let expected_sha = Some("4ff7e0b6d86763e1e042ffd746e94cdf4432618702deac303a1669e2a838db04");
    let resolver = inner.resolver.clone();
    let res = crate::lexicon_import::importer::import_eligible_words(
        &mut inner.conn,
        &resolver,
        &file_bytes,
        expected_count,
        expected_sha,
    )?;
    Ok(Json(res))
}

async fn list_sources_route(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::lexicon_import::provenance::LexiconSource>>, Error> {
    let inner = state.0.lock().unwrap();
    let res = crate::lexicon_import::importer::list_lexicon_sources(&inner.conn)?;
    Ok(Json(res))
}

async fn list_batches_route(
    State(state): State<AppState>,
    Query(params): Query<ImportPaginationParams>,
) -> Result<Json<Vec<crate::lexicon_import::provenance::LexiconImportBatch>>, Error> {
    let inner = state.0.lock().unwrap();
    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);
    let res = crate::lexicon_import::importer::list_import_batches(&inner.conn, limit, offset)?;
    Ok(Json(res))
}

async fn get_batch_details_route(
    State(state): State<AppState>,
    Path(import_id): Path<String>,
) -> Result<Json<crate::lexicon_import::provenance::LexiconImportBatch>, Error> {
    let inner = state.0.lock().unwrap();
    let res = crate::lexicon_import::importer::get_import_batch(&inner.conn, &import_id)?
        .ok_or_else(|| Error::NotFoundError(format!("Import batch {} not found", import_id)))?;
    Ok(Json(res))
}

async fn get_batch_deferred_route(
    State(state): State<AppState>,
    Path(import_id): Path<String>,
    Query(params): Query<ImportPaginationParams>,
) -> Result<Json<Vec<crate::lexicon_import::provenance::DeferredLexiconEntry>>, Error> {
    let inner = state.0.lock().unwrap();
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let res = crate::lexicon_import::importer::list_deferred_entries(
        &inner.conn,
        &import_id,
        limit,
        offset,
    )?;
    Ok(Json(res))
}
