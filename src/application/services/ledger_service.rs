use std::sync::Arc;
use std::time::Duration;
use chrono::{NaiveTime, Utc};
use crate::application::dto::transfer_dto::{
    DepositRequest, SettlementResponse, TransferRequest, TransferResponse,
};
use crate::domain::entities::account::SYSTEM_MINT_ACCOUNT_ID;
use crate::domain::entities::ledger::ExecuteTransferCommand;
use crate::domain::repositories::LedgerRepository;
use crate::error::AppError;

#[derive(Clone)]
pub struct LedgerService {
    repo: Arc<dyn LedgerRepository>,
}

impl LedgerService {
    pub fn new(repo: Arc<dyn LedgerRepository>) -> Self {
        Self { repo }
    }

    /// Deposit (Minting new money into a customer wallet):
    /// In Double-Entry Accounting, money NEVER appears out of thin air.
    /// A Deposit is simply a transfer from the `SYSTEM_MINT` contra-account
    /// (`00000000-0000-0000-0000-000000000001`) to the customer's account!
    pub async fn deposit(&self, req: DepositRequest) -> Result<TransferResponse, AppError> {
        let transfer_req = TransferRequest {
            idempotency_key: req.idempotency_key,
            from_account_id: SYSTEM_MINT_ACCOUNT_ID,
            to_account_id: req.to_account_id,
            amount: req.amount,
            description: req.description,
        };

        self.transfer(transfer_req).await
    }

    pub async fn transfer(&self, req: TransferRequest) -> Result<TransferResponse, AppError> {
        if req.idempotency_key.trim().is_empty() {
            return Err(AppError::InvalidTransfer(String::from(
                "idempotency_key is required",
            )));
        }

        if req.amount <= 0 {
            return Err(AppError::InvalidTransfer(String::from(
                "transfer amount must be strictly positive",
            )));
        }

        if req.from_account_id == req.to_account_id {
            return Err(AppError::InvalidTransfer(String::from(
                "cannot transfer to the same account",
            )));
        }

        let cmd = ExecuteTransferCommand {
            idempotency_key: req.idempotency_key,
            from_account_id: req.from_account_id,
            to_account_id: req.to_account_id,
            amount: req.amount,
            description: req.description,
        };

        let receipt = self.repo.execute_transfer(cmd).await?;
        tracing::info!(
            journal_id = %receipt.journal_id,
            idempotency_key = %receipt.idempotency_key,
            from_account_id = %receipt.from_account_id,
            to_account_id = %receipt.to_account_id,
            amount = receipt.amount,
            status = receipt.status.as_str(),
            idempotent_replay = receipt.idempotent_replay,
            "ledger transfer processed"
        );
        Ok(receipt.into())
    }

    /// Settle all `PENDING_SETTLEMENT` journals by aggregating `SYSTEM_MINT` postings
    /// and updating `SYSTEM_MINT`'s materialized balance in a single atomic batch.
    pub async fn settle_eod(&self) -> Result<SettlementResponse, AppError> {
        let receipt = self.repo.settle_pending_journals().await?;
        tracing::info!(
            settled_journals_count = receipt.settled_journals_count,
            mint_net_delta = receipt.mint_net_delta,
            mint_new_balance = receipt.mint_new_balance,
            "EOD settlement batch executed"
        );
        Ok(receipt.into())
    }

    /// Spawns the background End-of-Day (EOD) Settlement Worker.
    /// - If `interval_override` is `Some(duration)`, runs periodically every `duration`.
    /// - If `None`, schedules execution at the next UTC End-of-Day (`00:00:00` UTC) daily.
    pub fn spawn_eod_settlement_worker(&self, interval_override: Option<Duration>) {
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                let wait_duration = match interval_override {
                    Some(d) => d,
                    None => duration_until_next_utc_midnight(),
                };

                tracing::info!(
                    next_run_in_secs = wait_duration.as_secs(),
                    "EOD settlement worker scheduled"
                );
                tokio::time::sleep(wait_duration).await;

                if let Err(e) = service.settle_eod().await {
                    tracing::error!(error = %e, "EOD settlement worker failed");
                }
            }
        });
    }
}

fn duration_until_next_utc_midnight() -> Duration {
    let now = Utc::now();
    let tomorrow = (now + chrono::Duration::days(1)).date_naive();
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap_or_default();
    let next_eod = tomorrow.and_time(midnight).and_utc();
    (next_eod - now)
        .to_std()
        .unwrap_or_else(|_| Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use uuid::Uuid;
    use crate::domain::entities::ledger::{JournalStatus, SettlementBatchReceipt, TransferReceipt};

    #[derive(Default)]
    struct MockLedgerState {
        pending_mint_postings: Vec<i64>,
        mint_balance: i64,
    }

    struct MockLedgerRepo {
        state: Mutex<MockLedgerState>,
    }

    #[async_trait]
    impl LedgerRepository for MockLedgerRepo {
        async fn execute_transfer(
            &self,
            cmd: ExecuteTransferCommand,
        ) -> Result<TransferReceipt, AppError> {
            let mut state = self.state.lock().unwrap();
            let status = if cmd.from_account_id == SYSTEM_MINT_ACCOUNT_ID {
                state.pending_mint_postings.push(-cmd.amount);
                JournalStatus::PendingSettlement
            } else if cmd.to_account_id == SYSTEM_MINT_ACCOUNT_ID {
                state.pending_mint_postings.push(cmd.amount);
                JournalStatus::PendingSettlement
            } else {
                JournalStatus::Settled
            };

            Ok(TransferReceipt {
                journal_id: Uuid::new_v4(),
                idempotency_key: cmd.idempotency_key,
                from_account_id: cmd.from_account_id,
                to_account_id: cmd.to_account_id,
                amount: cmd.amount,
                status,
                idempotent_replay: false,
                created_at: Utc::now(),
            })
        }

        async fn settle_pending_journals(&self) -> Result<SettlementBatchReceipt, AppError> {
            let mut state = self.state.lock().unwrap();
            let count = state.pending_mint_postings.len() as i64;
            let net_delta: i64 = state.pending_mint_postings.drain(..).sum();
            state.mint_balance += net_delta;

            Ok(SettlementBatchReceipt {
                settled_journals_count: count,
                mint_net_delta: net_delta,
                mint_new_balance: state.mint_balance,
                settled_at: Utc::now(),
            })
        }
    }

    #[tokio::test]
    async fn test_deposit_is_pending_settlement_until_eod() {
        let repo = Arc::new(MockLedgerRepo {
            state: Mutex::new(MockLedgerState {
                pending_mint_postings: Vec::new(),
                mint_balance: -1_500_000,
            }),
        });
        let service = LedgerService::new(repo);
        let alice_id = Uuid::from_u128(0x11111111_1111_1111_1111_111111111111);
        let bob_id = Uuid::from_u128(0x22222222_2222_2222_2222_222222222222);

        // 1. Intraday deposits do not lock/update SYSTEM_MINT -> status is PENDING_SETTLEMENT
        let dep1 = service
            .deposit(DepositRequest {
                idempotency_key: String::from("dep-1"),
                to_account_id: alice_id,
                amount: 250_000,
                description: String::from("Topup Alice"),
            })
            .await
            .unwrap();
        assert_eq!(dep1.status, JournalStatus::PendingSettlement);

        let dep2 = service
            .deposit(DepositRequest {
                idempotency_key: String::from("dep-2"),
                to_account_id: bob_id,
                amount: 100_000,
                description: String::from("Topup Bob"),
            })
            .await
            .unwrap();
        assert_eq!(dep2.status, JournalStatus::PendingSettlement);

        // 2. P2P transfer between customers settles immediately -> status is SETTLED
        let p2p = service
            .transfer(TransferRequest {
                idempotency_key: String::from("p2p-1"),
                from_account_id: alice_id,
                to_account_id: bob_id,
                amount: 50_000,
                description: String::from("Alice to Bob"),
            })
            .await
            .unwrap();
        assert_eq!(p2p.status, JournalStatus::Settled);

        // 3. EOD Settlement aggregates all pending SYSTEM_MINT postings (-250k + -100k = -350k)
        let settlement = service.settle_eod().await.unwrap();
        assert_eq!(settlement.settled_journals_count, 2);
        assert_eq!(settlement.mint_net_delta, -350_000);
        assert_eq!(settlement.mint_new_balance, -1_850_000);

        // 4. Subsequent EOD run with no new deposits settles 0 journals
        let empty_settlement = service.settle_eod().await.unwrap();
        assert_eq!(empty_settlement.settled_journals_count, 0);
        assert_eq!(empty_settlement.mint_net_delta, 0);
        assert_eq!(empty_settlement.mint_new_balance, -1_850_000);
    }
}
