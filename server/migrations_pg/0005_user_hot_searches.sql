-- User-scoped hot searches to avoid cross-user leakage of sensitive keywords.
CREATE TABLE IF NOT EXISTS user_hot_searches (
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    keyword       TEXT NOT NULL,
    search_count  BIGINT NOT NULL DEFAULT 0,
    last_searched TEXT,
    updated_at    TEXT NOT NULL DEFAULT utc_text(),
    PRIMARY KEY (user_id, keyword)
);

CREATE INDEX IF NOT EXISTS idx_user_hot_searches_user_count
  ON user_hot_searches(user_id, search_count DESC);
