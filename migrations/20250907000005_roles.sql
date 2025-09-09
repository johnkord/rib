-- Generic role assignments keyed by provider-specific subject key
-- Example subject keys: "discord:123456789", "btc:bc1qxyz..."
CREATE TABLE IF NOT EXISTS user_roles (
    subject TEXT PRIMARY KEY,
    role TEXT NOT NULL CHECK (role IN ('user','moderator','admin')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Optional future index examples (not created now):
-- CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_user_roles_role ON user_roles(role);
