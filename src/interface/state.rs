use crate::application::services::account_service::AccountService;
use crate::application::services::ledger_service::LedgerService;

#[derive(Clone)]
pub struct AppState {
    pub account_service: AccountService,
    pub ledger_service: LedgerService,
}
