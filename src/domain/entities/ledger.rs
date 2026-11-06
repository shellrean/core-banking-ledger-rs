use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JournalStatus {
    PendingSettlement,
    Settled,
}

impl JournalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JournalStatus::PendingSettlement => "PENDING_SETTLEMENT",
            JournalStatus::Settled => "SETTLED",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "PENDING_SETTLEMENT" => JournalStatus::PendingSettlement,
            _ => JournalStatus::Settled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostingLeg {
    pub id: Uuid,
    pub account_id: Uuid,
    pub amount: i64, // Negative for debit/outflow, Positive for credit/inflow
}

#[derive(Debug, Clone)]
pub struct ExecuteTransferCommand {
    pub idempotency_key: String,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub amount: i64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferReceipt {
    pub journal_id: Uuid,
    pub idempotency_key: String,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub amount: i64,
    pub status: JournalStatus,
    pub idempotent_replay: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementBatchReceipt {
    pub settled_journals_count: i64,
    pub mint_net_delta: i64,
    pub mint_new_balance: i64,
    pub settled_at: DateTime<Utc>,
}
