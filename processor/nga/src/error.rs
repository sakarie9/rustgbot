//! NGA 模块错误类型定义

/// NGA 模块的错误类型
#[derive(Debug)]
pub enum NGAError {
    /// 网络请求错误
    Network(reqwest::Error),
    /// 页面解析错误
    Parse(String),
    /// HTTP 状态码错误
    Http { status: u16, message: String },
}

impl std::fmt::Display for NGAError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "网络请求失败: {}", e),
            Self::Parse(msg) => write!(f, "解析页面失败: {}", msg),
            Self::Http { status, message } => write!(f, "HTTP 错误 {}: {}", status, message),
        }
    }
}

impl std::error::Error for NGAError {}

impl From<reqwest::Error> for NGAError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error)
    }
}

impl From<anyhow::Error> for NGAError {
    fn from(error: anyhow::Error) -> Self {
        Self::Parse(error.to_string())
    }
}

pub type NGAResult<T> = std::result::Result<T, NGAError>;
