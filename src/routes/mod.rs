use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::state::AppState;

pub mod feed;
pub mod health;

#[derive(OpenApi)]
#[openapi(
    paths(
        health::healthz,
        feed::get_feed,
    ),
    info(
        title = "Distribution Service",
        version = env!("CARGO_PKG_VERSION"),
        description = "Generates podcast RSS feeds from playlists stored in podcast-service."
    ),
    tags(
        (name = "feed", description = "RSS feed endpoints"),
        (name = "health", description = "Liveness probes"),
    )
)]
struct ApiDoc;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::healthz))
        .route("/feed/:playlist_id", get(feed::get_feed))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
