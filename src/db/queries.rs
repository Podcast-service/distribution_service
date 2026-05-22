use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{Episode, Playlist};

const PLAYLIST_SQL: &str = r#"
SELECT
    p.id,
    p.title,
    p.description,
    p.cover_image_url,
    p.is_public,
    p.updated_at,
    up.user_id        AS owner_user_id,
    up.username       AS owner_username,
    up.language       AS owner_language
FROM playlists p
JOIN user_profiles up ON up.id = p.owner_profile_id
WHERE p.id = $1
"#;

const EPISODES_SQL: &str = r#"
SELECT
    pod.id,
    pod.title,
    pod.description,
    pod.cover_image_url,
    pod.audio_url,
    pod.audio_size_bytes,
    pod.duration_seconds,
    pod.published_at,
    pod.created_at,
    pp.position,
    ap.author_name,
    c.name                AS category_name
FROM playlist_podcasts pp
JOIN podcasts pod        ON pod.id = pp.podcast_id
JOIN author_profiles ap  ON ap.id  = pod.author_id
LEFT JOIN categories c   ON c.id   = pod.category_id
WHERE pp.playlist_id = $1
  AND pod.status = 'PUBLISHED'
ORDER BY pp.position ASC
"#;

pub async fn fetch_playlist(pool: &PgPool, playlist_id: Uuid) -> AppResult<Playlist> {
    use sqlx::Row;

    let row = sqlx::query(PLAYLIST_SQL)
        .bind(playlist_id)
        .fetch_optional(pool)
        .await?;

    let row = row.ok_or(AppError::NotFound)?;

    let is_public: bool = row.try_get("is_public")?;
    if !is_public {
        return Err(AppError::Forbidden);
    }

    Ok(Playlist {
        id: row.try_get("id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        cover_image_url: row.try_get("cover_image_url")?,
        updated_at: row.try_get("updated_at")?,
        owner_user_id: row.try_get("owner_user_id")?,
        owner_username: row.try_get("owner_username")?,
        owner_language: row.try_get("owner_language")?,
    })
}

pub async fn fetch_episodes(pool: &PgPool, playlist_id: Uuid) -> AppResult<Vec<Episode>> {
    use sqlx::Row;

    let rows = sqlx::query(EPISODES_SQL)
        .bind(playlist_id)
        .fetch_all(pool)
        .await?;

    let mut episodes = Vec::with_capacity(rows.len());
    for row in rows {
        episodes.push(Episode {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            description: row.try_get("description")?,
            cover_image_url: row.try_get("cover_image_url")?,
            audio_url: row.try_get("audio_url")?,
            audio_size_bytes: row.try_get("audio_size_bytes")?,
            duration_seconds: row.try_get("duration_seconds")?,
            published_at: row.try_get("published_at")?,
            created_at: row.try_get("created_at")?,
            position: row.try_get("position")?,
            author_name: row.try_get("author_name")?,
            category_name: row.try_get("category_name")?,
        });
    }

    Ok(episodes)
}
