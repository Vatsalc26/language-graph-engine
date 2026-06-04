use crate::app::AppState;
use crate::db::repository::Repository;
use crate::error::Error;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
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
pub struct SymbolSummary {
    pub position: i32,
    pub surface_form: String,
    pub canonical_entity_id: String,
    pub active_revision_cid: String,
    pub normalization: String,
    pub status: String,
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

async fn list_symbols(State(state): State<AppState>) -> Result<Json<Vec<SymbolSummary>>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    // Retrieve active members from snapshot
    let members = repo.get_snapshot_members(&inner.resolver.active_snapshot_cid)?;

    let mut summaries = Vec::new();
    for member in members {
        let rev = repo.get_grapheme_revision(&member.revision_cid)?;
        summaries.push(SymbolSummary {
            position: member.position,
            surface_form: rev.surface_form,
            canonical_entity_id: member.entity_id,
            active_revision_cid: member.revision_cid,
            normalization: rev.normalization,
            status: "Healthy".to_string(),
        });
    }

    Ok(Json(summaries))
}

async fn get_symbol_details(
    State(state): State<AppState>,
    Path(entity_id): Path<String>,
) -> Result<Json<SymbolDetailsResponse>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    // Get current revision CID from head
    let revision_cid = repo.get_entity_head(&entity_id)?.ok_or_else(|| {
        Error::NotFoundError(format!("No head revision found for entity {}", entity_id))
    })?;

    // Load grapheme revision details
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
) -> Result<Json<crate::model::AlphabetSnapshot>, Error> {
    let inner = state.0.lock().unwrap();
    let repo = Repository::new(&inner.conn);

    let snapshot = repo.get_alphabet_snapshot(&inner.resolver.active_snapshot_cid)?;
    Ok(Json(snapshot))
}

async fn resolve_text(
    State(state): State<AppState>,
    Json(payload): Json<ResolveRequest>,
) -> Result<Json<crate::resolver::text::ResolutionResult>, Error> {
    let inner = state.0.lock().unwrap();
    let result = inner.resolver.resolve(&payload.text)?;
    Ok(Json(result))
}
