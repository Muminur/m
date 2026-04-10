use crate::database::Database;
use crate::error::AppError;
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;

/// A prompt template stored in the database.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub prompt: String,
    pub action_type: String,
    pub is_builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// List all templates from the database.
pub fn list_templates(db: &Database) -> Result<Vec<AiTemplate>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, prompt, action_type, is_builtin, created_at, updated_at
         FROM ai_templates ORDER BY name",
    )?;

    let templates = stmt
        .query_map([], |row| {
            Ok(AiTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                prompt: row.get(3)?,
                action_type: row.get(4)?,
                is_builtin: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(templates)
}

/// Get a single template by ID.
pub fn get_template(db: &Database, id: &str) -> Result<Option<AiTemplate>, AppError> {
    let conn = db.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, prompt, action_type, is_builtin, created_at, updated_at
         FROM ai_templates WHERE id = ?1",
    )?;

    let result = stmt
        .query_row(params![id], |row| {
            Ok(AiTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                prompt: row.get(3)?,
                action_type: row.get(4)?,
                is_builtin: row.get::<_, i32>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .optional()?;

    Ok(result)
}

/// Create a new template.
pub fn create_template(
    db: &Database,
    name: &str,
    description: Option<&str>,
    prompt: &str,
    action_type: &str,
) -> Result<AiTemplate, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let conn = db.get()?;

    conn.execute(
        "INSERT INTO ai_templates (id, name, description, prompt, action_type, is_builtin)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![id, name, description, prompt, action_type],
    )?;

    // Read back to get timestamps
    drop(conn);
    get_template(db, &id)?.ok_or_else(|| AppError::StorageError {
        code: crate::error::StorageErrorCode::DatabaseError,
        message: "Template created but not found".into(),
    })
}

/// Update an existing template.
pub fn update_template(
    db: &Database,
    id: &str,
    name: &str,
    description: Option<&str>,
    prompt: &str,
) -> Result<(), AppError> {
    let conn = db.get()?;
    let rows = conn.execute(
        "UPDATE ai_templates SET name = ?2, description = ?3, prompt = ?4,
         updated_at = strftime('%s', 'now') WHERE id = ?1 AND is_builtin = 0",
        params![id, name, description, prompt],
    )?;

    if rows == 0 {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Template '{}' not found or is built-in", id),
        });
    }

    Ok(())
}

/// Delete a template (only user-created, not built-in).
pub fn delete_template(db: &Database, id: &str) -> Result<(), AppError> {
    let conn = db.get()?;
    let rows = conn.execute(
        "DELETE FROM ai_templates WHERE id = ?1 AND is_builtin = 0",
        params![id],
    )?;

    if rows == 0 {
        return Err(AppError::StorageError {
            code: crate::error::StorageErrorCode::DatabaseError,
            message: format!("Template '{}' not found or is built-in", id),
        });
    }

    Ok(())
}

/// Render a template prompt by substituting `{{key}}` with values.
pub fn render_template(prompt: &str, vars: &HashMap<String, String>) -> String {
    let mut result = prompt.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}

/// Extension trait to make `query_row` return `Option` on not found.
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template() {
        let mut vars = HashMap::new();
        vars.insert("transcript".into(), "Hello world".into());
        vars.insert("duration".into(), "60".into());
        let result = render_template("Summarize ({{duration}}s): {{transcript}}", &vars);
        assert_eq!(result, "Summarize (60s): Hello world");
    }

    #[test]
    fn test_render_template_no_vars() {
        let vars = HashMap::new();
        let result = render_template("No substitution needed", &vars);
        assert_eq!(result, "No substitution needed");
    }
}
