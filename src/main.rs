use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use serde::Deserialize;
use std::fs;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Deserialize, Clone)]
struct Config {
    message: String,
    port: u16,
}

struct AppState {
    config: Config,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "config_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_config().unwrap_or_else(|err| {
        eprintln!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    let port = config.port;
    let state = Arc::new(AppState { config });

    // Build our application with routes
    let app = Router::new()
        .route("/", get(home_handler))
        .route("/health", get(health_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Failed to bind to {}: {}", addr, err);
            std::process::exit(1);
        });

    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app).await.unwrap_or_else(|err| {
        eprintln!("Server error: {}", err);
        std::process::exit(1);
    });
}

async fn home_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    state.config.message.clone()
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    // Try multiple config paths in order of preference
    let config_paths = vec![
        "config/config.json",    // Production path (relative to installation)
        "../config/config.json", // When running from bin/ directory
        "config.json",           // Development/fallback
    ];

    let mut last_error = None;

    for config_path in config_paths {
        match fs::read_to_string(config_path) {
            Ok(config_str) => {
                let config: Config = serde_json::from_str(&config_str)
                    .map_err(|e| format!("Failed to parse {}: {}", config_path, e))?;
                tracing::info!("Loaded config from: {}", config_path);
                return Ok(config);
            }
            Err(e) => {
                last_error = Some(format!("{}: {}", config_path, e));
                continue;
            }
        }
    }

    Err(format!(
        "Failed to load config from any path. Last error: {}",
        last_error.unwrap_or_default()
    )
    .into())
}
