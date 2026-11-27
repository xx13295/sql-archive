use serde::Serialize;

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub diagnostic: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn user(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            diagnostic: None,
        }
    }

    pub fn internal(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            diagnostic: Some("本地操作未完成；未包含仓储内容或敏感信息。".into()),
        }
    }

    pub fn io(context: impl Into<String>, error: impl std::fmt::Display) -> Self {
        Self::internal("IO_ERROR", format!("{}：{error}", context.into()))
    }
}
