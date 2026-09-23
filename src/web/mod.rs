//! Web Dashboard Module.
//!
//! Provides an HTTP server using Axum to power the interactive browser-based
//! financial terminal, serving the embedded single-page application and REST APIs.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::{self, StockEntry};
use crate::domain::{
    CreateNoteInput, MarketCalendar, ScanResult, StockMover, StockNote, Timeframe, UpdateNoteInput,
};
use crate::provider::CandleSyncService;
use crate::provider::yahoo::YahooProvider;
use crate::scanner::{ScanState, Scanner};
use crate::storage::{CacheSyncMeta, MarketDb};

/// Embedded single-page dashboard HTML/CSS/JS.
const DASHBOARD_HTML: &str = include_str!("index.html");

/// Embedded SVG favicon.
const FAVICON_SVG: &str = include_str!("favicon.svg");

/// Shared application state for Web API endpoints.
#[derive(Clone)]
pub struct WebState {
    pub config_path: PathBuf,
    pub provider: Arc<YahooProvider>,
    pub db: MarketDb,
    pub sync_service: CandleSyncService,
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

#[derive(Debug, Serialize)]
pub struct MarketStatus {
    pub is_open: bool,
    pub is_trading_day: bool,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
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
) -> Result<Json<StockEntry>, (StatusCode, Json<ErrorResponse>)> {
    let original_config = config::load_stock_config(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    let mut config = original_config.clone();

    let sym_upper = new_stock.symbol.trim().to_uppercase();
    if sym_upper.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Stock symbol cannot be empty".to_string(),
            }),
        ));
    }
    if new_stock.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Stock name cannot be empty".to_string(),
            }),
        ));
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

    let added = StockEntry {
        symbol: sym_upper.clone(),
        name: new_stock.name.trim().to_string(),
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

    Ok(Json(added))
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

    let sym_upper = symbol.trim().to_uppercase();
    let initial_len = config.stocks.len();
    config
        .stocks
        .retain(|s| s.symbol.to_uppercase() != sym_upper);

    if config.stocks.len() == initial_len {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Stock '{symbol}' not found"),
            }),
        ));
    }

    if config.stocks.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Cannot delete the last remaining stock in the universe".to_string(),
            }),
        ));
    }

    config::save_stock_config(&state.config_path, &config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if let Err(e) = state.db.delete_ticker(&sym_upper) {
        let rollback = config::save_stock_config(&state.config_path, &original_config);
        let message = match rollback {
            Ok(()) => format!("Failed to delete ticker database record: {e}"),
            Err(rollback_error) => {
                format!(
                    "Failed to delete ticker database record: {e}; config rollback also failed: {rollback_error}"
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
                "[{}] {} ({}) -> served {} candles",
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

/// Scan movers across the universe.
async fn scan_movers(
    State(state): State<WebState>,
    Query(params): Query<ScanQuery>,
) -> Result<Json<ScanResult>, (StatusCode, Json<ErrorResponse>)> {
    let threshold = params.threshold.unwrap_or(3.0);
    let force = params.force.unwrap_or(false);

    let state_path = ScanState::default_path();
    let mut scan_state = ScanState::load(&state_path);

    let now = chrono::Utc::now().timestamp();
    if !force
        && let Some(ref last) = scan_state.last_scan_result
        // If scan is less than 3 minutes old, filter locally
        && (now - last.timestamp).abs() < 180
    {
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

        return Ok(Json(updated));
    }

    let config = config::load_stock_config(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let scanner = Scanner::new(state.provider.clone());
    let result = scanner.scan(&config.stocks, threshold).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    scan_state.last_scan_result = Some(result.clone());
    let _ = scan_state.save(&state_path);

    Ok(Json(result))
}

/// Retrieve the most recent scan result from cache.
async fn get_scan_cached(
    Query(params): Query<ScanQuery>,
) -> Result<Json<Option<ScanResult>>, (StatusCode, Json<ErrorResponse>)> {
    let threshold = params.threshold.unwrap_or(3.0);
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
    let meta = state
        .db
        .get_sync_meta(&params.symbol, &params.timeframe)
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
    };

    let app = create_router(state);
    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    let url = format!("http://localhost:{port}");
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║       MarketWatch Web Dashboard is Running!                  ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  URL:      {:<49} ║", url);
    println!("║  Theme:    White (Default) / Black (Toggle with 'T')        ║");
    println!("║  Features: Top 150 Universe | Movers Screener (1%,2%,3%)    ║");
    println!("║            Multi-Chart Grid | 09:30 & 15:30 Cron Catch-up   ║");
    println!("║  Controls: Use ↑/↓ to flip stocks | 1-6 timeframes | R reload║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    if open_browser {
        println!("Opening {url} in your default browser...");
        let _ = webbrowser::open(&url);
    }

    println!("Press Ctrl+C in terminal to stop the server.\n");
    axum::serve(listener, app).await?;

    Ok(())
}
