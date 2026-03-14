-- Core schema for MVP (auth + cases).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    real_name     TEXT,
    status        TEXT NOT NULL DEFAULT 'active',
    created_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);

CREATE TABLE IF NOT EXISTS roles (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    description TEXT,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS user_roles (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_code  TEXT NOT NULL REFERENCES roles(code),
    granted_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, role_code)
);

CREATE INDEX IF NOT EXISTS idx_user_roles_user ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_role ON user_roles(role_code);

-- Seed default roles.
INSERT OR IGNORE INTO roles (id, code, name, description) VALUES
  ('role_admin', 'admin', '管理员', '系统管理员，拥有全部权限'),
  ('role_host', 'host_lawyer', '主办律师', '案件负责人，管理案件和团队'),
  ('role_assistant', 'lawyer_assistant', '律师助理', '协助处理案件'),
  ('role_guest', 'guest', '访客', '只读权限');

CREATE TABLE IF NOT EXISTS cases (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    owner_id    TEXT NOT NULL REFERENCES users(id),
    status      TEXT NOT NULL DEFAULT 'active',
    tags        TEXT, -- JSON array
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_cases_owner ON cases(owner_id);
CREATE INDEX IF NOT EXISTS idx_cases_status ON cases(status);
CREATE INDEX IF NOT EXISTS idx_cases_created_at ON cases(created_at);

CREATE TABLE IF NOT EXISTS case_members (
    id           TEXT PRIMARY KEY,
    case_id      TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_in_case TEXT NOT NULL, -- owner/member/viewer
    joined_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    joined_by    TEXT NOT NULL,
    UNIQUE(case_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_case_members_case ON case_members(case_id);
CREATE INDEX IF NOT EXISTS idx_case_members_user ON case_members(user_id);

