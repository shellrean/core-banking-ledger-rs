-- 1. Accounts Table
-- Notice the CHECK constraint:
-- - 'CUSTOMER' accounts can NEVER have a negative balance (balance >= 0).
-- - 'SYSTEM' accounts (like SYSTEM_MINT / Settlement) act as the contra-account
--   for minting deposits, keeping the entire ledger strictly zero-sum (SUM(balance) == 0).
CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY,
    account_number VARCHAR(32) NOT NULL UNIQUE,
    owner_name VARCHAR(100) NOT NULL,
    account_type VARCHAR(20) NOT NULL DEFAULT 'CUSTOMER' CHECK (account_type IN ('SYSTEM', 'CUSTOMER')),
    balance BIGINT NOT NULL DEFAULT 0 CHECK (account_type = 'SYSTEM' OR balance >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Journals Table (Immutable transaction header)
CREATE TABLE IF NOT EXISTS journals (
    id UUID PRIMARY KEY,
    description TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. Postings Table (Immutable Double-Entry legs: sum of postings per journal MUST equal 0)
CREATE TABLE IF NOT EXISTS postings (
    id UUID PRIMARY KEY,
    journal_id UUID NOT NULL REFERENCES journals(id),
    account_id UUID NOT NULL REFERENCES accounts(id),
    amount BIGINT NOT NULL, -- Negative (-amount) for outflow, Positive (+amount) for inflow
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_postings_journal_id ON postings(journal_id);
CREATE INDEX IF NOT EXISTS idx_postings_account_id ON postings(account_id);

-- 4. Idempotency Keys Table (Prevents double-spending / double-minting on network retries)
CREATE TABLE IF NOT EXISTS idempotency_keys (
    idempotency_key VARCHAR(128) PRIMARY KEY,
    journal_id UUID NOT NULL REFERENCES journals(id),
    from_account_id UUID NOT NULL REFERENCES accounts(id),
    to_account_id UUID NOT NULL REFERENCES accounts(id),
    amount BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. Seed Data (Zero-Sum Genesis State with matching Journals, Postings & Idempotency Keys)
INSERT INTO accounts (id, account_number, owner_name, account_type, balance)
VALUES
    ('00000000-0000-0000-0000-000000000001', 'SYS-MINT-001', 'System Mint / Settlement', 'SYSTEM', -1500000),
    ('11111111-1111-1111-1111-111111111111', 'ACC-1001', 'Alice', 'CUSTOMER', 1000000),
    ('22222222-2222-2222-2222-222222222222', 'ACC-1002', 'Bob', 'CUSTOMER', 500000)
ON CONFLICT (id) DO NOTHING;

INSERT INTO journals (id, description)
VALUES
    ('00000000-0000-0000-0000-000000000101', 'Genesis Deposit: Alice Initial Funding'),
    ('00000000-0000-0000-0000-000000000102', 'Genesis Deposit: Bob Initial Funding')
ON CONFLICT (id) DO NOTHING;

INSERT INTO postings (id, journal_id, account_id, amount)
VALUES
    -- Journal 101: SYS-MINT-001 (-1,000,000) -> Alice (+1,000,000)
    ('00000000-0000-0000-0000-000000000201', '00000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000000001', -1000000),
    ('00000000-0000-0000-0000-000000000202', '00000000-0000-0000-0000-000000000101', '11111111-1111-1111-1111-111111111111', 1000000),
    -- Journal 102: SYS-MINT-001 (-500,000) -> Bob (+500,000)
    ('00000000-0000-0000-0000-000000000203', '00000000-0000-0000-0000-000000000102', '00000000-0000-0000-0000-000000000001', -500000),
    ('00000000-0000-0000-0000-000000000204', '00000000-0000-0000-0000-000000000102', '22222222-2222-2222-2222-222222222222', 500000)
ON CONFLICT (id) DO NOTHING;

INSERT INTO idempotency_keys (idempotency_key, journal_id, from_account_id, to_account_id, amount)
VALUES
    ('seed-genesis-alice-001', '00000000-0000-0000-0000-000000000101', '00000000-0000-0000-0000-000000000001', '11111111-1111-1111-1111-111111111111', 1000000),
    ('seed-genesis-bob-001', '00000000-0000-0000-0000-000000000102', '00000000-0000-0000-0000-000000000001', '22222222-2222-2222-2222-222222222222', 500000)
ON CONFLICT (idempotency_key) DO NOTHING;
