use serde::Deserialize;

/// Pixiv OAuth 令牌响应
#[derive(Debug, Deserialize)]
pub struct PixivTokenResponse {
    pub access_token: String,
    pub expires_in: u64,
}

/// Pixiv OAuth 错误响应
#[derive(Debug, Deserialize)]
pub struct PixivTokenError {
    pub error: String,
    pub error_description: Option<String>,
}

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
    pub original: Option<String>,
}

/// Pixiv 多页响应
#[derive(Debug, Deserialize)]
pub struct PixivPagesResponse {
    pub error: bool,
    pub body: Option<Vec<PixivPageInfo>>,
}

#[derive(Debug, Deserialize)]
pub struct PixivPageInfo {
    pub urls: PixivUrls,
}

/// Pixiv App API 响应结构
#[derive(Debug, Deserialize)]
pub struct PixivAppApiResponse {
    pub illust: PixivAppIllust,
}

#[derive(Debug, Deserialize)]
pub struct PixivAppIllust {
    pub meta_pages: Option<Vec<PixivAppMetaPage>>,
    pub meta_single_page: Option<PixivAppMetaSinglePage>,
}

#[derive(Debug, Deserialize)]
pub struct PixivAppMetaPage {
    pub image_urls: PixivAppImageUrls,
}

#[derive(Debug, Deserialize)]
pub struct PixivAppImageUrls {
    pub original: String,
}

#[derive(Debug, Deserialize)]
pub struct PixivAppMetaSinglePage {
    pub original_image_url: String,
}
