use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct OperationLogNew {
    pub user_id: String,
    pub user_name: String,
    pub case_id: Option<String>,
    pub action: String,
    pub module: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub target_title: Option<String>,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
    pub changed_fields: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: Option<String>,
}

pub fn spawn_operation_log(pool: SqlitePool, mut log: OperationLogNew) {
    if log.changed_fields.is_none() {
        log.changed_fields = extract_changed_fields(&log.old_value, &log.new_value);
    }

    tokio::spawn(async move {
        if let Err(e) = insert_operation_log(&pool, log).await {
            tracing::warn!(error = %e, "insert operation log failed");
        }
    });
}

async fn insert_operation_log(pool: &SqlitePool, log: OperationLogNew) -> anyhow::Result<()> {
    let id = Uuid::new_v4().to_string();
    let old_value = log
        .old_value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;
    let new_value = log
        .new_value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()?;

    // Keep metadata small to avoid unexpected DB bloat.
    let user_name = truncate(&log.user_name, 200);
    let action = truncate(&log.action, 32);
    let module = truncate(&log.module, 32);
    let target_type = truncate(&log.target_type, 64);
    let target_id = log.target_id.map(|s| truncate(&s, 128));
    let target_title = log.target_title.map(|s| truncate(&s, 500));
    let changed_fields = log.changed_fields.map(|s| truncate(&s, 1000));
    let ip_address = log.ip_address.map(|s| truncate(&s, 100));
    let user_agent = log.user_agent.map(|s| truncate(&s, 1000));
    let request_id = log.request_id.map(|s| truncate(&s, 128));

    sqlx::query(
        r#"
        INSERT INTO operation_logs (
            id,
            user_id,
            user_name,
            case_id,
            action,
            module,
            target_type,
            target_id,
            target_title,
            old_value,
            new_value,
            changed_fields,
            ip_address,
            user_agent,
            request_id
        )
        VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
        )
        "#,
    )
    .bind(id)
    .bind(log.user_id)
    .bind(user_name)
    .bind(log.case_id)
    .bind(action)
    .bind(module)
    .bind(target_type)
    .bind(target_id)
    .bind(target_title)
    .bind(old_value)
    .bind(new_value)
    .bind(changed_fields)
    .bind(ip_address)
    .bind(user_agent)
    .bind(request_id)
    .execute(pool)
    .await?;

    Ok(())
}

fn extract_changed_fields(old_value: &Option<Value>, new_value: &Option<Value>) -> Option<String> {
    let (Some(Value::Object(old)), Some(Value::Object(new))) = (old_value, new_value) else {
        return None;
    };

    let mut keys = old.keys().chain(new.keys()).cloned().collect::<Vec<_>>();
    keys.sort();
    keys.dedup();

    let mut changed = Vec::new();
    for k in keys {
        if old.get(&k) != new.get(&k) {
            changed.push(k);
        }
    }

    if changed.is_empty() {
        None
    } else {
        Some(changed.join(","))
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}
