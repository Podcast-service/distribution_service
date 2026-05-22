use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::db::queries;
use crate::error::{AppError, AppResult};
use crate::rss;
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/feed/{playlist_id}",
    tag = "feed",
    params(
        ("playlist_id" = String, Path, description = "UUID of the public playlist (with or without .xml suffix)")
    ),
    responses(
        (status = 200, description = "RSS feed", content_type = "application/rss+xml"),
        (status = 400, description = "Invalid playlist_id"),
        (status = 403, description = "Playlist exists but is not public"),
        (status = 404, description = "Playlist not found"),
    )
)]
pub async fn get_feed(
    State(state): State<AppState>,
    Path(playlist_id_raw): Path<String>,
) -> AppResult<Response> {
    let id_str = playlist_id_raw
        .strip_suffix(".xml")
        .unwrap_or(&playlist_id_raw);

    let playlist_id = Uuid::parse_str(id_str)
        .map_err(|_| AppError::BadRequest(format!("invalid uuid: {id_str}")))?;

    let playlist = queries::fetch_playlist(&state.pool, playlist_id).await?;
    let episodes = queries::fetch_episodes(&state.pool, playlist_id).await?;

    let xml = rss::build_feed(&playlist, &episodes, &state.public_base_url)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/rss+xml; charset=utf-8"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );

    Ok((StatusCode::OK, headers, xml).into_response())
}
