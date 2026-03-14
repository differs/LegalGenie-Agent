-- User-scoped hot searches to avoid cross-user leakage of sensitive keywords.

CREATE TABLE IF NOT EXISTS user_hot_searches (
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    keyword       TEXT NOT NULL,
    search_count  INTEGER NOT NULL DEFAULT 0,
    last_searched DATETIME,
    updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, keyword)
);

CREATE INDEX IF NOT EXISTS idx_user_hot_searches_user_count
  ON user_hot_searches(user_id, search_count DESC);

