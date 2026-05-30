use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub cover_image_url: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub owner_user_id: Uuid,
    pub owner_username: String,
    pub owner_language: String,
}

#[derive(Debug, Clone)]
pub struct Episode {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub cover_image_url: Option<String>,
    pub audio_url_file: String,
    pub audio_size_bytes: Option<i64>,
    pub duration_seconds: Option<i32>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub position: i32,
    pub author_name: String,
    pub category_name: Option<String>,
}
