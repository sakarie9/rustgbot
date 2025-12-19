use serde::Deserialize;

/// Pixiv Ajax API 响应
#[derive(Debug, Deserialize)]
pub struct PixivApiResponse {
    pub error: bool,
    pub message: String,
    pub body: Option<PixivIllustBody>,
}

/// Pixiv 作品信息
#[derive(Debug, Deserialize)]
pub struct PixivIllustBody {
    pub id: String,
    pub title: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "userName")]
    pub user_name: String,
    pub description: String,
    #[serde(rename = "pageCount")]
    pub page_count: u32,
    pub urls: PixivUrls,
    pub tags: Option<PixivTags>,
    #[serde(rename = "xRestrict")]
    pub x_restrict: u32,
}

#[derive(Debug, Deserialize)]
pub struct PixivTags {
    pub tags: Vec<PixivTag>,
}

#[derive(Debug, Deserialize)]
pub struct PixivTag {
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct PixivUrls {
    pub regular: Option<String>,
    // pub original: Option<String>,
}
