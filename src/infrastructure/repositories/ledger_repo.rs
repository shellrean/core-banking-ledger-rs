use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use crate::domain::entities::account::SYSTEM_MINT_ACCOUNT_ID;
use crate::domain::entities::ledger::{
    ExecuteTransferCommand, JournalStatus, SettlementBatchReceipt, TransferReceipt,
};
use crate::domain::repositories::LedgerRepository;
use crate::error::AppError;

#[derive(Clone)]
pub struct LedgerPostgresRepository {
    pool: PgPool,
}

impl LedgerPostgresRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LedgerRepository for LedgerPostgresRepository {
    async fn execute_transfer(
        &self,
        cmd: ExecuteTransferCommand,
    ) -> Result<TransferReceipt, AppError> {
        let mut tx = self.pool.begin().await?;

        // =========================================================================
        // STEP 1: IDEMPOTENCY GUARD
        // Check if this idempotency_key was already processed.
        // If found, verify parameters match and return the existing receipt safely
        // without deducting or minting the balance a second time.
        // =========================================================================
        let existing_key = sqlx::query(
            r#"
            SELECT ik.journal_id, ik.from_account_id, ik.to_account_id, ik.amount, ik.created_at, j.status
            FROM idempotency_keys ik
            INNER JOIN journals j ON j.id = ik.journal_id
            WHERE ik.idempotency_key = $1
            "#,
        )
        .bind(&cmd.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = existing_key {
            let saved_from: Uuid = row.get("from_account_id");
            let saved_to: Uuid = row.get("to_account_id");
            let saved_amount: i64 = row.get("amount");
            let saved_status: String = row.get("status");

            if saved_from != cmd.from_account_id
                || saved_to != cmd.to_account_id
                || saved_amount != cmd.amount
            {
                return Err(AppError::IdempotencyConflict(String::from(
                    "idempotency_key already used with different transfer payload",
                )));
            }

            return Ok(TransferReceipt {
                journal_id: row.get("journal_id"),
                idempotency_key: cmd.idempotency_key,
                from_account_id: saved_from,
                to_account_id: saved_to,
                amount: saved_amount,
                status: JournalStatus::from_db_str(&saved_status),
                idempotent_replay: true,
                created_at: row.get("created_at"),
            });
        }

        // =========================================================================
        // STEP 2 & 3: HOT-ACCOUNT-AWARE DETERMINISTIC LOCKING & BALANCE INVARIANT
        // - If the transfer involves `SYSTEM_MINT_ACCOUNT_ID` (Hot Contra-Account),
        //   we DO NOT lock `SYSTEM_MINT_ACCOUNT_ID` intraday! We only lock the
        //   customer account and mark the journal as `PENDING_SETTLEMENT` so the
        //   EOD Settlement Worker updates `SYSTEM_MINT` in a single batch.
        // - If both accounts are non-mint accounts (P2P Transfer), we lock both in
        //   deterministic UUID order (`SELECT ... FOR UPDATE`) to prevent AB-BA
        //   deadlocks and mark the journal immediately as `SETTLED`.
        // =========================================================================
        let journal_status = if cmd.from_account_id == SYSTEM_MINT_ACCOUNT_ID {
            // Deposit / Minting: Lock ONLY the destination customer account
            let _recipient_row = sqlx::query(
                r#"
                SELECT id FROM accounts WHERE id = $1 FOR UPDATE
                "#,
            )
            .bind(cmd.to_account_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("account {} not found", cmd.to_account_id)))?;

            JournalStatus::PendingSettlement
        } else if cmd.to_account_id == SYSTEM_MINT_ACCOUNT_ID {
            // Burn / Redemption to System Mint: Lock ONLY the sender customer account
            let sender_row = sqlx::query(
                r#"
                SELECT id, account_type, balance FROM accounts WHERE id = $1 FOR UPDATE
                "#,
            )
            .bind(cmd.from_account_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("account {} not found", cmd.from_account_id))
            })?;

            let sender_type: String = sender_row.get("account_type");
            let sender_balance: i64 = sender_row.get("balance");
            if sender_type != "SYSTEM" && sender_balance < cmd.amount {
                return Err(AppError::InsufficientBalance(format!(
                    "insufficient funds: available {}, required {}",
                    sender_balance, cmd.amount
                )));
            }

            JournalStatus::PendingSettlement
        } else {
            // Standard P2P Transfer: Deterministic UUID-sorted locking on both accounts
            let (first_lock_id, second_lock_id) = if cmd.from_account_id < cmd.to_account_id {
                (cmd.from_account_id, cmd.to_account_id)
            } else {
                (cmd.to_account_id, cmd.from_account_id)
            };

            let first_row = sqlx::query(
                r#"
                SELECT id, account_type, balance FROM accounts WHERE id = $1 FOR UPDATE
                "#,
            )
            .bind(first_lock_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("account {} not found", first_lock_id)))?;

            let second_row = sqlx::query(
                r#"
                SELECT id, account_type, balance FROM accounts WHERE id = $1 FOR UPDATE
                "#,
            )
            .bind(second_lock_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("account {} not found", second_lock_id)))?;

            let (sender_type, sender_balance): (String, i64) =
                if first_lock_id == cmd.from_account_id {
                    (first_row.get("account_type"), first_row.get("balance"))
                } else {
                    (second_row.get("account_type"), second_row.get("balance"))
                };

            if sender_type != "SYSTEM" && sender_balance < cmd.amount {
                return Err(AppError::InsufficientBalance(format!(
                    "insufficient funds: available {}, required {}",
                    sender_balance, cmd.amount
                )));
            }

            JournalStatus::Settled
        };

        // =========================================================================
        // STEP 4: DOUBLE-ENTRY LEDGER RECORDING (IMMUTABLE JOURNAL & POSTINGS)
        // Every financial movement (whether Deposit/Mint or P2P Transfer) MUST
        // have two balanced legs whose sum equals 0:
        // Leg 1 (Source Outflow)      = -cmd.amount
        // Leg 2 (Destination Inflow)  = +cmd.amount
        // (-cmd.amount) + (+cmd.amount) == 0
        // =========================================================================
        let journal_id = Uuid::new_v4();
        let now = Utc::now();
        let settled_at = match journal_status {
            JournalStatus::PendingSettlement => None,
            JournalStatus::Settled => Some(now),
        };

        sqlx::query(
            r#"
            INSERT INTO journals (id, description, status, settled_at, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(journal_id)
        .bind(&cmd.description)
        .bind(journal_status.as_str())
        .bind(settled_at)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let debit_leg_amount = -cmd.amount;
        let credit_leg_amount = cmd.amount;
        debug_assert_eq!(debit_leg_amount + credit_leg_amount, 0);

        // Insert Outflow Leg (-amount)
        sqlx::query(
            r#"
            INSERT INTO postings (id, journal_id, account_id, amount, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(journal_id)
        .bind(cmd.from_account_id)
        .bind(debit_leg_amount)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Insert Inflow Leg (+amount)
        sqlx::query(
            r#"
            INSERT INTO postings (id, journal_id, account_id, amount, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(journal_id)
        .bind(cmd.to_account_id)
        .bind(credit_leg_amount)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // =========================================================================
        // STEP 5: UPDATE NON-HOT ACCOUNT BALANCES & RECORD IDEMPOTENCY KEY
        // Customer balances are updated immediately in real-time;
        // `SYSTEM_MINT_ACCOUNT_ID` balance is deferred to EOD settlement.
        // =========================================================================
        if cmd.from_account_id != SYSTEM_MINT_ACCOUNT_ID {
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance = balance - $1, updated_at = $2
                WHERE id = $3
                "#,
            )
            .bind(cmd.amount)
            .bind(now)
            .bind(cmd.from_account_id)
            .execute(&mut *tx)
            .await?;
        }

        if cmd.to_account_id != SYSTEM_MINT_ACCOUNT_ID {
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance = balance + $1, updated_at = $2
                WHERE id = $3
                "#,
            )
            .bind(cmd.amount)
            .bind(now)
            .bind(cmd.to_account_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            INSERT INTO idempotency_keys (
                idempotency_key, journal_id, from_account_id, to_account_id, amount, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(&cmd.idempotency_key)
        .bind(journal_id)
        .bind(cmd.from_account_id)
        .bind(cmd.to_account_id)
        .bind(cmd.amount)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(TransferReceipt {
            journal_id,
            idempotency_key: cmd.idempotency_key,
            from_account_id: cmd.from_account_id,
            to_account_id: cmd.to_account_id,
            amount: cmd.amount,
            status: journal_status,
            idempotent_replay: false,
            created_at: now,
        })
    }

    async fn settle_pending_journals(&self) -> Result<SettlementBatchReceipt, AppError> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();

        // Atomically claim all PENDING_SETTLEMENT journals using FOR UPDATE SKIP LOCKED,
        // aggregate their SYSTEM_MINT postings, and apply the net delta to SYSTEM_MINT's
        // materialized balance in a single database round-trip.
        let row = sqlx::query(
            r#"
            WITH claimed_journals AS (
                UPDATE journals
                SET status = 'SETTLED', settled_at = $1
                WHERE id IN (
                    SELECT id FROM journals
                    WHERE status = 'PENDING_SETTLEMENT'
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING id
            ),
            mint_summary AS (
                SELECT
                    COUNT(DISTINCT cj.id)::BIGINT AS journal_count,
                    COALESCE(SUM(p.amount), 0)::BIGINT AS net_delta
                FROM claimed_journals cj
                LEFT JOIN postings p
                    ON p.journal_id = cj.id AND p.account_id = $2
            )
            UPDATE accounts a
            SET balance = a.balance + ms.net_delta,
                updated_at = CASE WHEN ms.journal_count > 0 THEN $1 ELSE a.updated_at END
            FROM mint_summary ms
            WHERE a.id = $2
            RETURNING ms.journal_count, ms.net_delta, a.balance
            "#,
        )
        .bind(now)
        .bind(SYSTEM_MINT_ACCOUNT_ID)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(SettlementBatchReceipt {
            settled_journals_count: row.get("journal_count"),
            mint_net_delta: row.get("net_delta"),
            mint_new_balance: row.get("balance"),
            settled_at: now,
        })
    }
}
