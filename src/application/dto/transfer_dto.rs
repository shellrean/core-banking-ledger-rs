use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::domain::entities::ledger::{JournalStatus, SettlementBatchReceipt, TransferReceipt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    pub idempotency_key: String,
    pub to_account_id: Uuid,
    pub amount: i64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    pub idempotency_key: String,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub amount: i64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResponse {
    pub journal_id: Uuid,
    pub idempotency_key: String,
    pub from_account_id: Uuid,
    pub to_account_id: Uuid,
    pub amount: i64,
    pub status: JournalStatus,
    pub idempotent_replay: bool,
    pub created_at: DateTime<Utc>,
}

impl From<TransferReceipt> for TransferResponse {
    fn from(receipt: TransferReceipt) -> Self {
        Self {
            journal_id: receipt.journal_id,
            idempotency_key: receipt.idempotency_key,
            from_account_id: receipt.from_account_id,
            to_account_id: receipt.to_account_id,
            amount: receipt.amount,
            status: receipt.status,
            idempotent_replay: receipt.idempotent_replay,
            created_at: receipt.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementResponse {
    pub settled_journals_count: i64,
    pub mint_net_delta: i64,
    pub mint_new_balance: i64,
    pub settled_at: DateTime<Utc>,
}

impl From<SettlementBatchReceipt> for SettlementResponse {
    fn from(receipt: SettlementBatchReceipt) -> Self {
        Self {
            settled_journals_count: receipt.settled_journals_count,
            mint_net_delta: receipt.mint_net_delta,
            mint_new_balance: receipt.mint_new_balance,
            settled_at: receipt.settled_at,
        }
    }
}
