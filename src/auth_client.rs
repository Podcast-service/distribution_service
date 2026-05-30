//! Tiny HTTP client for Auth-service.
//!
//! Auth-service exposes per-user endpoints behind a JWT issued for that
//! same user, so there is no way to look up another user's email through
//! the public API. distribution_service therefore needs a small
//! service-to-service endpoint added to Auth-service:
//!
//! ```text
//! GET /internal/users/{user_id}
//!     Authorization: Bearer <INTERNAL_API_TOKEN>
//!
//! 200 OK
//! { "id": "<uuid>", "email": "user@example.com" }
//! ```
//!
//! Reference Go handler (add to Auth-service):
//!
//! ```go
//! // internal/server/internal_users.go
//! type internalUser struct {
//!     ID    string `json:"id"`
//!     Email string `json:"email"`
//! }
//!
//! func (s *Server) GetInternalUser(w http.ResponseWriter, r *http.Request) {
//!     if r.Header.Get("Authorization") != "Bearer "+s.cfg.InternalAPIToken {
//!         http.Error(w, "forbidden", http.StatusForbidden); return
//!     }
//!     id := chi.URLParam(r, "user_id")
//!     u, err := s.users.GetByID(r.Context(), id)
//!     if errors.Is(err, repo.ErrNotFound) { http.NotFound(w, r); return }
//!     if err != nil { http.Error(w, err.Error(), 500); return }
//!     _ = json.NewEncoder(w).Encode(internalUser{ID: u.ID, Email: u.Email})
//! }
//! // r.Get("/internal/users/{user_id}", srv.GetInternalUser)
//! ```
//!
//! Until that endpoint is deployed, this client returns Ok(None) on any
//! transport / 404 error so the RSS still builds (with `<itunes:email>`
//! omitted). A warning is logged so the gap is visible.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct AuthClient {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    base_url: String,
    api_token: Option<String>,
    cache: RwLock<HashMap<Uuid, CachedEmail>>,
    ttl: Duration,
}

#[derive(Clone)]
struct CachedEmail {
    value: Option<String>,
    expires_at: Instant,
}

#[derive(Deserialize)]
struct InternalUser {
    email: Option<String>,
}

impl AuthClient {
    pub fn new(base_url: String, api_token: Option<String>, ttl: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .expect("reqwest client init");

        Self {
            inner: Arc::new(Inner {
                http,
                base_url: base_url.trim_end_matches('/').to_string(),
                api_token,
                cache: RwLock::new(HashMap::new()),
                ttl,
            }),
        }
    }

    /// Returns the user's email if known, or `None` if the endpoint is
    /// unavailable / user not found. Never errors — RSS generation
    /// should not block on auth-service downtime.
    pub async fn get_email(&self, user_id: Uuid) -> Option<String> {
        if let Some(cached) = self.cache_get(user_id).await {
            return cached;
        }

        let value = self.fetch_email(user_id).await;

        self.cache_put(user_id, value.clone()).await;
        value
    }

    async fn cache_get(&self, user_id: Uuid) -> Option<Option<String>> {
        let guard = self.inner.cache.read().await;
        match guard.get(&user_id) {
            Some(entry) if entry.expires_at > Instant::now() => Some(entry.value.clone()),
            _ => None,
        }
    }

    async fn cache_put(&self, user_id: Uuid, value: Option<String>) {
        let mut guard = self.inner.cache.write().await;
        guard.insert(
            user_id,
            CachedEmail {
                value,
                expires_at: Instant::now() + self.inner.ttl,
            },
        );
    }

    async fn fetch_email(&self, user_id: Uuid) -> Option<String> {
        let url = format!("{}/internal/users/{}", self.inner.base_url, user_id);
        let mut req = self.inner.http.get(&url);
        if let Some(token) = &self.inner.api_token {
            req = req.bearer_auth(token);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<InternalUser>().await {
                Ok(u) => u.email,
                Err(err) => {
                    tracing::warn!(error = %err, user_id = %user_id, "auth-service: bad json");
                    None
                }
            },
            Ok(resp) if resp.status().as_u16() == 404 => None,
            Ok(resp) => {
                tracing::warn!(
                    status = %resp.status(),
                    user_id = %user_id,
                    "auth-service: non-2xx"
                );
                None
            }
            Err(err) => {
                tracing::warn!(error = %err, user_id = %user_id, "auth-service: transport error");
                None
            }
        }
    }
}
