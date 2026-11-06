use async_trait::async_trait;
use uuid::Uuid;
use crate::domain::entities::account::Account;
use crate::domain::entities::ledger::{
    ExecuteTransferCommand, SettlementBatchReceipt, TransferReceipt,
};
use crate::error::AppError;

#[async_trait]
pub trait AccountRepository: Sync + Send {
    async fn find_all(&self) -> Result<Vec<Account>, AppError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Account>, AppError>;
    async fn create(
        &self,
        account_number: String,
        owner_name: String,
    ) -> Result<Account, AppError>;
}

#[async_trait]
pub trait LedgerRepository: Sync + Send {
    async fn execute_transfer(
        &self,
        cmd: ExecuteTransferCommand,
    ) -> Result<TransferReceipt, AppError>;

    async fn settle_pending_journals(&self) -> Result<SettlementBatchReceipt, AppError>;
}
