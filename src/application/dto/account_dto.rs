use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::domain::entities::account::Account;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub account_number: String,
    pub owner_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub account_number: String,
    pub owner_name: String,
    pub account_type: String,
    pub balance: i64,
    pub updated_at: DateTime<Utc>,
}

impl From<Account> for AccountResponse {
    fn from(acc: Account) -> Self {
        Self {
            id: acc.id,
            account_number: acc.account_number,
            owner_name: acc.owner_name,
            account_type: acc.account_type,
            balance: acc.balance,
            updated_at: acc.updated_at,
        }
    }
}
