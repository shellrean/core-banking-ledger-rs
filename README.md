# Core Banking Ledger

A double-entry core banking ledger API written in Rust using Axum, Tokio, SQLx, and PostgreSQL.

This project explores how to build a reliable financial ledger that stays consistent under concurrent load without sacrificing throughput on hot system accounts.

## How It Works

### Double-Entry Bookkeeping
Every transaction creates an immutable journal entry with two balanced posting legs (one negative outflow, one positive inflow) whose sum is always zero.

Customer accounts cannot hold a negative balance. New money enters the system through deposits, which are recorded as transfers from a system contra-account (System Mint) to the customer account. Once all journals are settled, the sum of all account balances across the database is zero.

### Avoiding Hot-Account Contention (EOD Settlement)
In a standard double-entry setup, every deposit debits the System Mint account. Under heavy traffic, locking and updating that single row on every deposit turns concurrent requests into a serial queue.

To solve this, deposits are split into two phases:

1. **Intraday Deposit (Real-Time)**  
   When a customer deposits funds, the transaction only locks and updates the customer account row. Both posting legs are still written to the ledger immediately, but the journal is marked as pending settlement. The customer can use their balance right away without waiting on a lock for the System Mint row.

2. **End-of-Day Settlement (Batch)**  
   A background worker runs at the end of the day (or on demand via an API endpoint) to claim all pending journals, sum the posting legs belonging to the System Mint account, and update the System Mint balance in a single atomic query.

```mermaid
sequenceDiagram
    participant Client
    participant API as Ledger API
    participant DB as PostgreSQL
    participant Worker as EOD Worker

    Client->>API: Deposit 250,000 to Alice
    API->>DB: Lock Alice account row
    API->>DB: Insert journal (PENDING_SETTLEMENT) & 2 postings
    API->>DB: Update Alice balance (+250,000)
    API-->>Client: 200 OK

    Worker->>DB: Claim PENDING_SETTLEMENT journals
    Worker->>DB: Sum System Mint postings & update balance once
    Worker->>DB: Mark journals as SETTLED
```

### Deadlock-Free Peer-to-Peer Transfers
Peer-to-peer transfers between customers settle immediately. To prevent deadlocks when two accounts transfer to each other at the same time (for example, Alice pays Bob while Bob pays Alice), the repository always locks the two account rows in deterministic UUID order.

### Idempotency
Every deposit and transfer requires an idempotency key. If a client retries a request with the same key and payload, the API returns the original receipt without applying the transaction twice. Using the same key with a different payload returns a conflict error.

## Project Structure

The codebase is organized into four layers:

```text
src/
├── domain/          Entities and repository traits
├── application/     Business services, DTOs, and the EOD settlement worker
├── infrastructure/  PostgreSQL repository implementations and DB pool setup
└── interface/       Axum HTTP routes, handlers, and logging middleware
```

## Getting Started

### Prerequisites
- Rust (1.85+)
- PostgreSQL (14+)

### Setup

1. Create a `.env` file in the root directory:

```env
DATABASE_URL=postgres://postgres:postgres@localhost:5432/core_banking_db
SERVER_ADDR=localhost:9090

# Optional: run the settlement worker every N seconds instead of waiting until 00:00 UTC
# EOD_SETTLEMENT_INTERVAL_SECS=60
```

2. Run the server (database migrations run automatically on startup):

```bash
cargo run
```

3. Run the test suite:

```bash
cargo test
```

## API Overview

You can find ready-to-run requests in [sample.http](./sample.http).

| Method | Path | Description |
| :--- | :--- | :--- |
| GET | `/accounts` | List all accounts and their balances |
| GET | `/accounts/{id}` | Get a single account by ID |
| POST | `/accounts` | Create a new customer account (starts at 0 balance) |
| POST | `/deposits` | Deposit funds into a customer account |
| POST | `/transfers` | Transfer funds between two accounts |
| POST | `/settlements/eod` | Run the EOD settlement batch for pending journals |

### Example Flow

Deposit funds into Alice's account:

```bash
curl -X POST http://localhost:9090/deposits \
  -H "Content-Type: application/json" \
  -d '{
    "idempotency_key": "dep-alice-topup-001",
    "to_account_id": "11111111-1111-1111-1111-111111111111",
    "amount": 250000,
    "description": "Bank Settlement Top-Up #001"
  }'
```

Transfer funds from Alice to Bob:

```bash
curl -X POST http://localhost:9090/transfers \
  -H "Content-Type: application/json" \
  -d '{
    "idempotency_key": "tx-alice-to-bob-001",
    "from_account_id": "11111111-1111-1111-1111-111111111111",
    "to_account_id": "22222222-2222-2222-2222-222222222222",
    "amount": 150000,
    "description": "Invoice Payment #001"
  }'
```

Trigger EOD settlement to reconcile the System Mint balance:

```bash
curl -X POST http://localhost:9090/settlements/eod
```

## License

MIT
