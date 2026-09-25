//! Web Dashboard Module.
//!
//! Provides an HTTP server using Axum to power the interactive browser-based
//! financial terminal, serving the embedded single-page application and REST APIs.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::{self, StockEntry};
use crate::domain::{
    CreateNoteInput, MarketCalendar, ScanResult, StockMover, StockNote, Timeframe, UpdateNoteInput,
    mover::validate_threshold, note::is_valid_status,
};
use crate::provider::CandleSyncService;
use crate::provider::yahoo::YahooProvider;
use crate::scanner::{ScanState, Scanner};
use crate::storage::{CacheSyncMeta, MarketDb};

/// Embedded single-page dashboard HTML/CSS/JS.
const DASHBOARD_HTML: &str = include_str!("index.html");

/// Embedded SVG favicon.
const FAVICON_SVG: &str = include_str!("favicon.svg");

type InFlightScanResult = Result<ScanResult, String>;

/// Shared application state for Web API endpoints.
#[derive(Clone)]
pub struct WebState {
    pub config_path: PathBuf,
    pub provider: Arc<YahooProvider>,
    pub db: MarketDb,
    pub sync_service: CandleSyncService,
    pub scan_in_flight: Arc<tokio::sync::Mutex<Option<tokio::sync::watch::Receiver<Option<InFlightScanResult>>>>>,
}

#[derive(Debug, Deserialize)]
pub struct CandlesQuery {
    pub symbol: String,
    pub timeframe: String,
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ScanQuery {
    pub threshold: Option<f64>,
    pub force: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct NotesQuery {
    pub symbol: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn bad_request(msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: msg.into() }),
    )
}

/// Normalize a user-supplied symbol (e.g. "MOTILALOFS" -> "MOTILALOFS.NS").
/// Automatically appends ".NS" for Indian market symbols lacking an exchange suffix or index prefix.
pub fn normalize_symbol(symbol: &str) -> String {
    let sym = symbol.trim().to_uppercase();
    if !sym.is_empty() && !sym.contains('.') && !sym.starts_with('^') {
        format!("{sym}.NS")
    } else {
        sym
    }
}

/// Validate a ticker symbol for universe membership (shared by add/delete paths).
fn validate_universe_symbol(sym_upper: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if sym_upper.is_empty() {
        return Err(bad_request("Stock symbol cannot be empty"));
    }
    if sym_upper.len() > 32
        || !sym_upper
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '='))
    {
        return Err(bad_request(format!(
            "Invalid symbol '{sym_upper}': use Yahoo ticker characters (letters, digits, . - ^ =), max 32 chars"
        )));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct MarketStatus {
    pub is_open: bool,
    pub is_trading_day: bool,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        // Fallback only: every handler must return an explicit status tuple.
        // Defaulting to 500 (not 400) so a future bare `Err(ErrorResponse)`
        // can never misreport a server failure as a client error.
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

/// Serve the embedded HTML dashboard.
async fn serve_index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Html(DASHBOARD_HTML),
    )
}

/// Serve the favicon.
async fn serve_favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml")], FAVICON_SVG)
}

/// Get all stocks in the universe.
async fn get_stocks(
    State(state): State<WebState>,
) -> Result<Json<Vec<StockEntry>>, (StatusCode, Json<ErrorResponse>)> {
    let config = config::load_stock_config(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(config.stocks))
}

/// Add a new stock to the universe.
async fn add_stock(
    State(state): State<WebState>,
    Json(new_stock): Json<StockEntry>,
) -> Result<(StatusCode, Json<StockEntry>), (StatusCode, Json<ErrorResponse>)> {
    let original_config = config::load_stock_config(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    let mut config = original_config.clone();

    let sym_upper = normalize_symbol(&new_stock.symbol);
    validate_universe_symbol(&sym_upper)?;

    let stock_name = if new_stock.name.trim().is_empty() {
        sym_upper.clone()
    } else {
        new_stock.name.trim().to_string()
    };

    if stock_name.chars().count() > 200 {
        return Err(bad_request("Stock name is too long (max 200 characters)"));
    }

    if config
        .stocks
        .iter()
        .any(|s| s.symbol.to_uppercase() == sym_upper)
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("Stock '{sym_upper}' is already in the universe"),
            }),
        ));
    }

    // Verify ticker against Yahoo Finance before adding to universe
    if let Err(e) = state.provider.fetch_mover(&sym_upper, &stock_name).await {
        return Err(bad_request(format!(
            "Symbol '{sym_upper}' could not be verified on Yahoo Finance: symbol not found or has no trading data ({e})."
        )));
    }

    let added = StockEntry {
        symbol: sym_upper.clone(),
        name: stock_name,
    };

    config.stocks.push(added.clone());

    config::save_stock_config(&state.config_path, &config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if let Err(e) = state.db.upsert_ticker(&crate::storage::Ticker {
        symbol: added.symbol.clone(),
        name: added.name.clone(),
        exchange: "NSE".to_string(),
        is_active: true,
        created_at: chrono::Utc::now().timestamp(),
    }) {
        let rollback = config::save_stock_config(&state.config_path, &original_config);
        let message = match rollback {
            Ok(()) => format!("Failed to update ticker database: {e}"),
            Err(rollback_error) => {
                format!(
                    "Failed to update ticker database: {e}; config rollback also failed: {rollback_error}"
                )
            }
        };
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: message }),
        ));
    }

    Ok((StatusCode::CREATED, Json(added)))
}

/// Remove a stock from the universe.
async fn delete_stock(
    State(state): State<WebState>,
    Path(symbol): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let original_config = config::load_stock_config(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    let mut config = original_config.clone();

    let raw_upper = symbol.trim().to_uppercase();
    let norm_upper = normalize_symbol(&symbol);
    validate_universe_symbol(&raw_upper)?;
    let initial_len = config.stocks.len();
    let found = config
        .stocks
        .iter()
        .find(|s| {
            let u = s.symbol.to_uppercase();
            u == raw_upper || u == norm_upper
        })
        .map(|s| s.symbol.clone());

    let target_symbol = match found {
        Some(sym) => sym,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Stock '{symbol}' not found"),
                }),
            ));
        }
    };

    // The sole remaining stock cannot be removed (checked after existence so
    // a typo never reports "cannot delete last" for a stock that isn't there).
    if initial_len == 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Cannot delete the last remaining stock in the universe".to_string(),
            }),
        ));
    }

    config
        .stocks
        .retain(|s| !s.symbol.eq_ignore_ascii_case(&target_symbol));

    config::save_stock_config(&state.config_path, &config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if let Err(e) = state.db.deactivate_ticker(&target_symbol) {
        let rollback = config::save_stock_config(&state.config_path, &original_config);
        let message = match rollback {
            Ok(()) => format!("Failed to deactivate ticker database record: {e}"),
            Err(rollback_error) => {
                format!(
                    "Failed to deactivate ticker database record: {e}; config rollback also failed: {rollback_error}"
                )
            }
        };
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: message }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Fetch normalized candle data for a symbol and timeframe with smart SQLite caching.
async fn get_candles(
    State(state): State<WebState>,
    Query(params): Query<CandlesQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if params.symbol.trim().is_empty() {
        return Err(bad_request("Query parameter 'symbol' cannot be empty"));
    }
    let timeframe: Timeframe = params
        .timeframe
        .parse()
        .map_err(|e: String| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;

    let force = params.force.unwrap_or(false);

    match state
        .sync_service
        .get_candles(&params.symbol, timeframe, force)
        .await
    {
        Ok((candles, status)) => {
            println!(
                "{} [API] [{}] {} ({}) -> {} candles",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                status.header_value(),
                params.symbol,
                timeframe.label(),
                candles.len()
            );
            Ok((
                [(
                    header::HeaderName::from_static("x-cache"),
                    status.header_value(),
                )],
                Json(candles),
            )
                .into_response())
        }
        Err(e) => {
            let status = match e {
                crate::error::MarketError::Provider { status: 404, .. }
                | crate::error::MarketError::NoData { .. } => StatusCode::NOT_FOUND,
                crate::error::MarketError::Provider { .. }
                | crate::error::MarketError::Network(_) => StatusCode::BAD_GATEWAY,
                crate::error::MarketError::InvalidSymbol(_)
                | crate::error::MarketError::InvalidTimeframe(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Err((
                status,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            ))
        }
    }
}

/// Filter and re-sort a cached or coalesced ScanResult by threshold percent.
fn filter_scan_result(last: &ScanResult, threshold: f64) -> ScanResult {
    let mut movers: Vec<StockMover> = last
        .all_quotes
        .iter()
        .filter(|m| m.matches_threshold(threshold))
        .cloned()
        .collect();

    movers.sort_by(|a, b| {
        b.change_percent
            .abs()
            .partial_cmp(&a.change_percent.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let gainers_count = movers.iter().filter(|m| m.is_gainer()).count();
    let losers_count = movers.iter().filter(|m| !m.is_gainer()).count();
    let movers_count = movers.len();

    let mut updated = last.clone();
    updated.threshold_percent = threshold;
    updated.movers_count = movers_count;
    updated.gainers_count = gainers_count;
    updated.losers_count = losers_count;
    updated.movers = movers;
    updated
}

/// Scan movers across the universe.
async fn scan_movers(
    State(state): State<WebState>,
    Query(params): Query<ScanQuery>,
) -> Result<Json<ScanResult>, (StatusCode, Json<ErrorResponse>)> {
    let threshold = params.threshold.unwrap_or(3.0);
    let threshold = validate_threshold(threshold)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let force = params.force.unwrap_or(false);

    let state_path = ScanState::default_path();
    let scan_state = ScanState::load(&state_path);

    let now = chrono::Utc::now().timestamp();
    if !force
        && let Some(ref last) = scan_state.last_scan_result
        // If scan is less than 3 minutes old, filter locally. Only past
        // timestamps count: a future/corrupt timestamp must re-scan, not
        // masquerade as fresh.
        && {
            let age = now - last.timestamp;
            (0..180).contains(&age)
        }
    {
        return Ok(Json(filter_scan_result(last, threshold)));
    }

    // Coalesce concurrent in-flight scans: if a scan is already running, wait on its watch receiver
    let leader_tx = {
        let mut guard = state.scan_in_flight.lock().await;
        if let Some(ref rx) = *guard {
            let mut follower_rx = rx.clone();
            drop(guard);

            while follower_rx.borrow().is_none() {
                if follower_rx.changed().await.is_err() {
                    break;
                }
            }

            return match &*follower_rx.borrow() {
                Some(Ok(scan_result)) => Ok(Json(filter_scan_result(scan_result, threshold))),
                Some(Err(err_msg)) => Err((
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: err_msg.clone(),
                    }),
                )),
                None => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "In-flight scan terminated without result".to_string(),
                    }),
                )),
            };
        } else {
            let (tx, rx) = tokio::sync::watch::channel(None);
            *guard = Some(rx);
            tx
        }
    };

    let config_res = config::load_stock_config(&state.config_path);
    let scan_res = match config_res {
        Ok(config) => {
            let scanner = Scanner::new(state.provider.clone());
            scanner.scan(&config.stocks, threshold).await
        }
        Err(e) => Err(e),
    };

    // Release in-flight registration and broadcast to followers
    {
        let mut guard = state.scan_in_flight.lock().await;
        *guard = None;
    }

    match scan_res {
        Ok(result) => {
            let mut scan_state = ScanState::load(&state_path);
            scan_state.last_scan_result = Some(result.clone());
            let _ = scan_state.save(&state_path);

            let _ = leader_tx.send(Some(Ok(result.clone())));
            Ok(Json(result))
        }
        Err(e) => {
            let err_string = e.to_string();
            let _ = leader_tx.send(Some(Err(err_string.clone())));

            let status = match e {
                crate::error::MarketError::InvalidInput(_) => StatusCode::BAD_REQUEST,
                crate::error::MarketError::NoData { .. } => StatusCode::NOT_FOUND,
                crate::error::MarketError::Provider { .. } | crate::error::MarketError::Network(_) => {
                    StatusCode::BAD_GATEWAY
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Err((status, Json(ErrorResponse { error: err_string })))
        }
    }
}

/// Retrieve the most recent scan result from cache.
async fn get_scan_cached(
    Query(params): Query<ScanQuery>,
) -> Result<Json<Option<ScanResult>>, (StatusCode, Json<ErrorResponse>)> {
    let threshold = params.threshold.unwrap_or(3.0);
    let threshold = validate_threshold(threshold)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let state_path = ScanState::default_path();
    let scan_state = ScanState::load(&state_path);

    if let Some(mut last) = scan_state.last_scan_result {
        let mut movers: Vec<StockMover> = last
            .all_quotes
            .iter()
            .filter(|m| m.matches_threshold(threshold))
            .cloned()
            .collect();

        movers.sort_by(|a, b| {
            b.change_percent
                .abs()
                .partial_cmp(&a.change_percent.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        last.threshold_percent = threshold;
        last.movers_count = movers.len();
        last.gainers_count = movers.iter().filter(|m| m.is_gainer()).count();
        last.losers_count = movers.iter().filter(|m| !m.is_gainer()).count();
        last.movers = movers;

        return Ok(Json(Some(last)));
    }

    Ok(Json(None))
}

/// Get the current scan schedule and persistence state.
async fn get_scan_state() -> Json<ScanState> {
    let state_path = ScanState::default_path();
    Json(ScanState::load(&state_path))
}

/// Return the current market status using the shared NSE calendar.
async fn get_market_status() -> Json<MarketStatus> {
    let now = crate::domain::calendar::now_ist();
    Json(MarketStatus {
        is_open: MarketCalendar::is_market_open(now),
        is_trading_day: MarketCalendar::is_trading_day(now.date_naive()),
    })
}

/// Get cache synchronization metadata for a symbol and timeframe.
async fn get_cache_meta(
    State(state): State<WebState>,
    Query(params): Query<CandlesQuery>,
) -> Result<Json<Option<CacheSyncMeta>>, (StatusCode, Json<ErrorResponse>)> {
    // Normalize exactly like the sync service's storage keys and validate
    // the timeframe (an unknown timeframe is a 400, not a silent null).
    let symbol = params.symbol.trim().to_uppercase();
    if symbol.is_empty() {
        return Err(bad_request("Query parameter 'symbol' cannot be empty"));
    }
    let timeframe: Timeframe = params
        .timeframe
        .parse()
        .map_err(|e: String| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let meta = state
        .db
        .get_sync_meta(&symbol, timeframe.label())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

    Ok(Json(meta))
}

/// List stock notes with optional symbol and status filters.
async fn list_notes(
    State(state): State<WebState>,
    Query(query): Query<NotesQuery>,
) -> Result<Json<Vec<StockNote>>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref status) = query.status
        && status != "all"
        && !is_valid_status(status)
    {
        return Err(bad_request(format!(
            "Invalid status filter '{status}': must be one of open, validated, invalidated, cancelled, all"
        )));
    }
    let notes = state
        .db
        .list_notes(query.symbol.as_deref(), query.status.as_deref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;
    Ok(Json(notes))
}

/// Retrieve a single stock note by ID.
async fn get_note(
    State(state): State<WebState>,
    Path(id): Path<i64>,
) -> Result<Json<StockNote>, (StatusCode, Json<ErrorResponse>)> {
    let note = state.db.get_note(id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    match note {
        Some(n) => Ok(Json(n)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note #{id} not found"),
            }),
        )),
    }
}

/// Create a new stock analysis note.
async fn create_note(
    State(state): State<WebState>,
    Json(input): Json<CreateNoteInput>,
) -> Result<(StatusCode, Json<StockNote>), (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) = input.validate() {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })));
    }

    let note = state.db.create_note(&input).map_err(|e| match e {
        crate::error::MarketError::NotFound(msg) => {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg }))
        }
        crate::error::MarketError::InvalidInput(msg) => {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg }))
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    Ok((StatusCode::CREATED, Json(note)))
}

/// Update an existing stock analysis note.
async fn update_note(
    State(state): State<WebState>,
    Path(id): Path<i64>,
    Json(input): Json<UpdateNoteInput>,
) -> Result<Json<StockNote>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(e) = input.validate(id) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })));
    }

    let updated = state.db.update_note(id, &input).map_err(|e| match e {
        crate::error::MarketError::NotFound(msg) => {
            (StatusCode::NOT_FOUND, Json(ErrorResponse { error: msg }))
        }
        crate::error::MarketError::InvalidInput(msg) => {
            (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: msg }))
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        ),
    })?;

    match updated {
        Some(n) => Ok(Json(n)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note #{id} not found"),
            }),
        )),
    }
}

/// Delete a stock analysis note.
async fn delete_note(
    State(state): State<WebState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let deleted = state.db.delete_note(id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Note #{id} not found"),
            }),
        ))
    }
}

/// Validate CSRF / Origin headers and optional auth token on state-modifying requests (POST, PUT, DELETE, PATCH).
/// Protects local terminal endpoints from cross-origin drive-by attacks from arbitrary websites.
async fn validate_mutation_csrf(
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let method = req.method();
    if matches!(
        method,
        &axum::http::Method::POST
            | &axum::http::Method::PUT
            | &axum::http::Method::DELETE
            | &axum::http::Method::PATCH
    ) {
        // Optional environment-configured Bearer token
        if let Some(expected_token) =
            std::env::var("MARKETWATCH_API_TOKEN").ok().filter(|t| !t.is_empty())
        {
            let authorized = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .map(|token| token == expected_token)
                .or_else(|| {
                    req.headers()
                        .get("x-api-key")
                        .and_then(|h| h.to_str().ok())
                        .map(|key| key == expected_token)
                })
                .unwrap_or(false);

            if !authorized {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "Missing or invalid authorization token".to_string(),
                    }),
                ));
            }
        }

        // 1. Block explicit cross-site fetch metadata
        if req
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|sec_fetch| sec_fetch == "cross-site")
        {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Cross-site request forgery forbidden".to_string(),
                }),
            ));
        }

        // 2. If Origin header is present, ensure it comes from localhost / 127.0.0.1 or matches Host
        if let Some(origin_str) = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|o| o.to_str().ok())
        {
            let is_local = origin_str.starts_with("http://localhost")
                || origin_str.starts_with("https://localhost")
                || origin_str.starts_with("http://127.0.0.1")
                || origin_str.starts_with("https://127.0.0.1");

            let host_matches = req
                .headers()
                .get(header::HOST)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|host| {
                    origin_str
                        .trim_start_matches("http://")
                        .trim_start_matches("https://")
                        == host
                });

            if !is_local && !host_matches {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: format!(
                            "Untrusted origin '{origin_str}' forbidden for state modifications"
                        ),
                    }),
                ));
            }
        }
    }

    Ok(next.run(req).await)
}

/// Build the Axum router with all routes and middleware.
pub fn create_router(state: WebState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/favicon.svg", get(serve_favicon))
        .route("/favicon.ico", get(serve_favicon))
        .route("/api/stocks", get(get_stocks).post(add_stock))
        .route("/api/stocks/{symbol}", delete(delete_stock))
        .route("/api/candles", get(get_candles))
        .route("/api/cache/meta", get(get_cache_meta))
        .route("/api/scan", get(scan_movers))
        .route("/api/scan/cached", get(get_scan_cached))
        .route("/api/scan/state", get(get_scan_state))
        .route("/api/market/status", get(get_market_status))
        .route("/api/notes", get(list_notes).post(create_note))
        .route(
            "/api/notes/{id}",
            get(get_note).put(update_note).delete(delete_note),
        )
        .layer(middleware::from_fn(validate_mutation_csrf))
        .with_state(state)
}

/// Start the Web Dashboard server on the specified port.
pub async fn start_server(
    config_path: PathBuf,
    port: u16,
    open_browser: bool,
) -> anyhow::Result<()> {
    let provider = Arc::new(YahooProvider::new()?);
    let db = MarketDb::open_default()?;
    let cfg = config::load_stock_config(&config_path)?;
    db.sync_tickers(&cfg.stocks)?;
    let sync_service = CandleSyncService::new(db.clone(), provider.clone());
    let state = WebState {
        config_path,
        provider,
        db,
        sync_service,
        scan_in_flight: Arc::new(tokio::sync::Mutex::new(None)),
    };

    let app = create_router(state);
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://localhost:{port}");
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       MarketWatch Web Dashboard is Running!                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  URL:      {:<49} ║", url);
    println!("║  Controls: ↑/↓ stocks | 1-6 timeframes | Space sync | ? help  ║");
    println!("║  Features: Top 150 Universe | Movers Screener (1%,2%,3%)    ║");
    println!("║            Multi-Chart Grid | 09:30 & 15:30 Cron Catch-up   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    if open_browser {
        println!("Opening {url} in your default browser...");
        let _ = webbrowser::open(&url);
    }

    println!("Press Ctrl+C in terminal to stop the server.\n");
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_symbol() {
        assert_eq!(normalize_symbol("MOTILALOFS"), "MOTILALOFS.NS");
        assert_eq!(normalize_symbol("motilalofs"), "MOTILALOFS.NS");
        assert_eq!(normalize_symbol("  motilalofs  "), "MOTILALOFS.NS");
        assert_eq!(normalize_symbol("RELIANCE.NS"), "RELIANCE.NS");
        assert_eq!(normalize_symbol("reliance.ns"), "RELIANCE.NS");
        assert_eq!(normalize_symbol("^NSEI"), "^NSEI");
        assert_eq!(normalize_symbol("^BSESN"), "^BSESN");
        assert_eq!(normalize_symbol("AAPL.US"), "AAPL.US");
        assert_eq!(normalize_symbol(""), "");
    }

    #[test]
    fn test_validate_universe_symbol() {
        assert!(validate_universe_symbol("MOTILALOFS.NS").is_ok());
        assert!(validate_universe_symbol("^NSEI").is_ok());
        assert!(validate_universe_symbol("TCS-EQ.NS").is_ok());
        assert!(validate_universe_symbol("").is_err());
        assert!(validate_universe_symbol("INVALID SYMBOL").is_err());
        assert!(validate_universe_symbol("TOOLONG".repeat(10).as_str()).is_err());
    }

    #[tokio::test]
    async fn test_csrf_protection_mutation_endpoints() {
        let db = MarketDb::open_in_memory().unwrap();
        let provider = Arc::new(YahooProvider::new().unwrap());
        let sync_service = CandleSyncService::new(db.clone(), provider.clone());
        let temp_config = std::env::temp_dir().join(format!("marketwatch_test_{}.toml", std::process::id()));
        config::save_stock_config(
            &temp_config,
            &config::StockConfig {
                stocks: vec![StockEntry {
                    symbol: "RELIANCE.NS".to_string(),
                    name: "Reliance Industries".to_string(),
                }],
            },
        )
        .unwrap();
        let state = WebState {
            config_path: temp_config.clone(),
            provider,
            db,
            sync_service,
            scan_in_flight: Arc::new(tokio::sync::Mutex::new(None)),
        };

        let app = create_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();

        // 1. Cross-site Sec-Fetch-Site on POST must be rejected with 403
        let res = client
            .post(format!("http://127.0.0.1:{port}/api/stocks"))
            .header("sec-fetch-site", "cross-site")
            .header("content-type", "application/json")
            .body(r#"{"symbol":"TCS.NS","name":"TCS"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), reqwest::StatusCode::FORBIDDEN);

        // 2. Untrusted Origin on POST must be rejected with 403
        let res = client
            .post(format!("http://127.0.0.1:{port}/api/stocks"))
            .header("origin", "https://malicious-site.com")
            .header("content-type", "application/json")
            .body(r#"{"symbol":"TCS.NS","name":"TCS"}"#)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), reqwest::StatusCode::FORBIDDEN);

        // 3. GET requests with untrusted origin are not blocked by CSRF middleware
        let res = client
            .get(format!("http://127.0.0.1:{port}/api/stocks"))
            .header("origin", "https://malicious-site.com")
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), reqwest::StatusCode::OK);

        // 4. Trusted localhost origin on POST is accepted through middleware
        let res = client
            .post(format!("http://127.0.0.1:{port}/api/stocks"))
            .header("origin", format!("http://127.0.0.1:{port}"))
            .header("sec-fetch-site", "same-origin")
            .header("content-type", "application/json")
            .body(r#"{"symbol":"TCS.NS","name":"TCS"}"#)
            .send()
            .await
            .unwrap();
        // Since temp config exists, handler succeeds with 201 Created
        assert_eq!(res.status(), reqwest::StatusCode::CREATED);

        let _ = std::fs::remove_file(temp_config);
    }

    #[tokio::test]
    async fn test_scan_movers_follower_coalesces_with_leader() {
        let db = MarketDb::open_in_memory().unwrap();
        let provider = Arc::new(YahooProvider::new().unwrap());
        let sync_service = CandleSyncService::new(db.clone(), provider.clone());
        let temp_config =
            std::env::temp_dir().join(format!("marketwatch_scan_test_{}.toml", std::process::id()));
        config::save_stock_config(
            &temp_config,
            &config::StockConfig {
                stocks: vec![StockEntry {
                    symbol: "INFY.NS".to_string(),
                    name: "Infosys".to_string(),
                }],
            },
        )
        .unwrap();

        // Pre-create an in-flight watch channel simulating an active leader scan
        let (tx, rx) = tokio::sync::watch::channel(None);
        let scan_in_flight = Arc::new(tokio::sync::Mutex::new(Some(rx)));

        let state = WebState {
            config_path: temp_config.clone(),
            provider,
            db,
            sync_service,
            scan_in_flight,
        };

        let app = create_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Spawn a background task to resolve the leader scan after 50ms
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let mock_mover = StockMover {
                symbol: "INFY.NS".to_string(),
                name: "Infosys".to_string(),
                price: 1600.0,
                prev_close: 1550.0,
                change: 50.0,
                change_percent: 3.22,
                volume: 100000,
                day_high: 1610.0,
                day_low: 1540.0,
                timestamp: chrono::Utc::now().timestamp(),
            };
            let mock_result = ScanResult {
                timestamp: chrono::Utc::now().timestamp(),
                scan_time: "15:30:00".to_string(),
                threshold_percent: 2.0,
                total_scanned: 1,
                movers_count: 1,
                gainers_count: 1,
                losers_count: 0,
                movers: vec![mock_mover.clone()],
                all_quotes: vec![mock_mover],
                failed_count: 0,
            };
            let _ = tx.send(Some(Ok(mock_result)));
        });

        let client = reqwest::Client::new();
        let res = client
            .get(format!(
                "http://127.0.0.1:{port}/api/scan?threshold=2.0&force=true"
            ))
            .send()
            .await
            .unwrap();

        assert_eq!(res.status(), reqwest::StatusCode::OK);
        let body: ScanResult = res.json().await.unwrap();
        assert_eq!(body.movers.len(), 1);
        assert_eq!(body.movers[0].symbol, "INFY.NS");

        let _ = std::fs::remove_file(temp_config);
    }
}
