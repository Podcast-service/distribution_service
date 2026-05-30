use sqlx::PgPool;

use crate::auth_client::AuthClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub public_base_url: String,
    pub auth: AuthClient,
}
