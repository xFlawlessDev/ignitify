use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RefreshTokenRecord {
    pub user_id: String,
    pub family_id: String,
    pub expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum RotateRefreshTokenOutcome {
    Rotated(RefreshTokenRecord),
    Missing,
    Expired,
    Reused { user_id: String, family_id: String },
}
