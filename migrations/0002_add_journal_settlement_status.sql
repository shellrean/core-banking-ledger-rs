-- Add settlement lifecycle status to journals to eliminate hot-account contention on SYSTEM_MINT.
-- Intraday deposits record both posting legs immediately with status = 'PENDING_SETTLEMENT'
-- while deferring the row-lock & balance update on SYSTEM_MINT until the EOD Settlement Worker runs.
ALTER TABLE journals
    ADD COLUMN IF NOT EXISTS status VARCHAR(24) NOT NULL DEFAULT 'SETTLED'
        CHECK (status IN ('PENDING_SETTLEMENT', 'SETTLED')),
    ADD COLUMN IF NOT EXISTS settled_at TIMESTAMPTZ NULL DEFAULT NOW();

-- Partial index so the EOD Settlement Worker only scans unsettled journals in O(K) time
CREATE INDEX IF NOT EXISTS idx_journals_pending_settlement
    ON journals (created_at)
    WHERE status = 'PENDING_SETTLEMENT';
