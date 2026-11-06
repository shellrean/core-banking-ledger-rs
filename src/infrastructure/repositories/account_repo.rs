use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;
use crate::domain::entities::account::Account;
use crate::domain::repositories::AccountRepository;
use crate::error::AppError;

#[derive(Clone)]
pub struct AccountPostgresRepository {
    pool: PgPool,
}

impl AccountPostgresRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountRepository for AccountPostgresRepository {
    async fn find_all(&self) -> Result<Vec<Account>, AppError> {
        let rows = sqlx::query(
            r#"
            SELECT id, account_number, owner_name, account_type, balance, created_at, updated_at
            FROM accounts
            ORDER BY account_type DESC, account_number ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for r in rows {
            results.push(Account {
                id: r.get("id"),
                account_number: r.get("account_number"),
                owner_name: r.get("owner_name"),
                account_type: r.get("account_type"),
                balance: r.get("balance"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        }

        Ok(results)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Account>, AppError> {
        let row = sqlx::query(
            r#"
            SELECT id, account_number, owner_name, account_type, balance, created_at, updated_at
            FROM accounts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Account {
            id: r.get("id"),
            account_number: r.get("account_number"),
            owner_name: r.get("owner_name"),
            account_type: r.get("account_type"),
            balance: r.get("balance"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    async fn create(
        &self,
        account_number: String,
        owner_name: String,
    ) -> Result<Account, AppError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let r = sqlx::query(
            r#"
            INSERT INTO accounts (id, account_number, owner_name, account_type, balance, created_at, updated_at)
            VALUES ($1, $2, $3, 'CUSTOMER', 0, $4, $4)
            RETURNING id, account_number, owner_name, account_type, balance, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(account_number)
        .bind(owner_name)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(Account {
            id: r.get("id"),
            account_number: r.get("account_number"),
            owner_name: r.get("owner_name"),
            account_type: r.get("account_type"),
            balance: r.get("balance"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
    }
}
