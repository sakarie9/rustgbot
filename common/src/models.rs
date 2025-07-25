/// 处理器解析结果
#[derive(Debug, Clone)]
pub struct ProcessorResultMedia {
    pub caption: String,
    pub urls: Vec<String>,
    pub spoiler: bool,
}

/// 统一的处理器结果类型
#[derive(Debug, Clone)]
pub enum ProcessorResult {
    /// 纯文本结果
    Text(String),
    /// 图片结果（包含图片URL和描述文本）
    Media(ProcessorResultMedia),
}

/// 统一的处理器错误类型
#[derive(Debug, Clone)]
pub struct ProcessorError {
    pub message: String,
    pub source: Option<String>,
}

impl std::fmt::Display for ProcessorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source {
            Some(source) => write!(f, "{}: {}", self.message, source),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ProcessorError {}

impl ProcessorError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(message: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: Some(source.into()),
        }
    }
}

impl From<anyhow::Error> for ProcessorError {
    fn from(error: anyhow::Error) -> Self {
        ProcessorError::new(error.to_string())
    }
}

impl From<reqwest::Error> for ProcessorError {
    fn from(error: reqwest::Error) -> Self {
        ProcessorError::with_source("网络请求失败", error.to_string())
    }
}

/// 统一的处理器结果类型别名
pub type ProcessorResultType = Result<ProcessorResult, ProcessorError>;

/// 统一的处理器trait
#[async_trait::async_trait]
pub trait LinkProcessor: Send + Sync {
    /// 获取正则表达式模式字符串
    fn pattern(&self) -> &'static str;
    
    /// 获取匹配的正则表达式（用于详细匹配）
    fn regex(&self) -> &regex::Regex;
    
    /// 处理匹配的链接并返回结果
    /// captures: 正则表达式的捕获组
    async fn process_captures(&self, captures: &regex::Captures<'_>) -> ProcessorResultType;
    
    /// 获取处理器名称
    fn name(&self) -> &'static str;
}