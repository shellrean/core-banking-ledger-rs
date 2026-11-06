use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SYSTEM_MINT_ACCOUNT_ID: Uuid = Uuid::from_u128(1); // 00000000-0000-0000-0000-000000000001

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub id: Uuid,
    pub account_number: String,
    pub owner_name: String,
    pub account_type: String, // "SYSTEM" or "CUSTOMER"
    pub balance: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
