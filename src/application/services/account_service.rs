use std::sync::Arc;
use uuid::Uuid;
use crate::application::dto::account_dto::{AccountResponse, CreateAccountRequest};
use crate::domain::repositories::AccountRepository;
use crate::error::AppError;

#[derive(Clone)]
pub struct AccountService {
    repo: Arc<dyn AccountRepository>,
}

impl AccountService {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }

    pub async fn list_accounts(&self) -> Result<Vec<AccountResponse>, AppError> {
        let accounts = self.repo.find_all().await?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    pub async fn get_account(&self, id: Uuid) -> Result<AccountResponse, AppError> {
        let account = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("account {} not found", id)))?;
        Ok(account.into())
    }

    pub async fn create_account(
        &self,
        req: CreateAccountRequest,
    ) -> Result<AccountResponse, AppError> {
        // New customer accounts always start at 0 balance.
        // Any initial funds must come through a Double-Entry Deposit from SYSTEM_MINT!
        let account = self
            .repo
            .create(req.account_number, req.owner_name)
            .await?;

        Ok(account.into())
    }
}
