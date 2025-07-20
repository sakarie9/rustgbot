use common::get_env_var;
use rand::Rng;
use regex::Regex;
use std::{
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

// ==== 文本 ====

pub const NGA_SUMMARY_MAX_LENGTH: usize = 800;
pub const NGA_SUMMARY_MAX_MAX_LENGTH: usize = 1000;

pub fn substring_desc(desc: &str) -> String {
    let chars: Vec<char> = desc.chars().collect();

    // 如果字符数没有超过最大长度，直接返回
    if chars.len() <= NGA_SUMMARY_MAX_LENGTH {
        return desc.trim().to_string();
    }

    // 在最大长度位置之后查找换行符
    let mut cr_pos = None;

    // 从 NGA_SUMMARY_MAX_LENGTH 位置开始查找换行符
    for i in NGA_SUMMARY_MAX_LENGTH..chars.len() {
        if chars[i] == '\n' {
            cr_pos = Some(i);
            break;
        }
    }

    match cr_pos {
        Some(pos) if pos < NGA_SUMMARY_MAX_MAX_LENGTH => {
            // 换行符在最大长度和极限长度之间，裁剪到换行符
            chars[..pos].iter().collect::<String>().trim().to_string()
        }
        _ => {
            // 没有找到合适的换行符，或换行符超过极限长度，直接截取到最大长度并添加省略号
            let truncated: String = chars[..NGA_SUMMARY_MAX_LENGTH].iter().collect();
            format!("{}……", truncated.trim())
        }
    }
}

// ==== 图片 ====

// 从内容中提取 NGA 图片链接
static IMG_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[img\](.*?)\[/img\]").unwrap());
pub fn get_nga_img_links(content: &str) -> Vec<String> {
    IMG_REGEX
        .captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| img_link_process(m.as_str())))
        .collect()
}

// 处理 NGA 图片链接
pub fn img_link_process(img_link: &str) -> String {
    let processed_link = if img_link.starts_with("http://") || img_link.starts_with("https://") {
        img_link.to_string()
    } else if img_link.len() >= 2 && img_link.starts_with("./") {
        format!("https://img.nga.178.com/attachments/{}", &img_link[2..])
    } else {
        img_link.to_string()
    };

    // 将低画质链接转换为高画质链接
    // 处理链接末尾的特殊后缀，删除倒数第二个点及其后面的内容
    if let Some(last_slash) = processed_link.rfind('/') {
        let (url_prefix, filename) = processed_link.split_at(last_slash + 1);

        // 查找最后两个点，删除倒数第二个点及其后面的内容
        if let Some(last_dot) = filename.rfind('.') {
            if let Some(second_last_dot) = filename[..last_dot].rfind('.') {
                format!("{}{}", url_prefix, &filename[..second_last_dot])
            } else {
                processed_link
            }
        } else {
            processed_link
        }
    } else {
        processed_link
    }
}

// ==== 正则替换 ====

// 正则替换简单内容
static HTML_ENTITY_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&(?:quot|amp|lt|gt|nbsp|apos);|<br/?>").unwrap());
pub fn replace_html_entities(text: &str) -> String {
    HTML_ENTITY_REGEX
        .replace_all(text, |caps: &regex::Captures| {
            match &caps[0] {
                "&quot;" => "\"",
                "&amp;" => "&",
                "&lt;" => "<",
                "&gt;" => ">",
                "&nbsp;" => " ",
                "&apos;" => "'",
                "<br/>" => "\n",
                "<br>" => "\n",
                _ => caps[0].to_string().leak(), // 不应该到达这里
            }
        })
        .into_owned()
}

// 移除HTML标签但保留文本内容
static HTML_TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
pub fn remove_html_tags(text: &str) -> String {
    HTML_TAG_REGEX.replace_all(text, "").to_string()
}

// 处理多行换行符
static NEWLINE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
pub fn normalize_newlines(text: &str) -> String {
    NEWLINE_REGEX.replace_all(text, "\n\n").to_string()
}

// ==== Cookie ====

pub fn get_nga_guest_cookie() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
        .saturating_sub(100);

    let mut rng = rand::rng();
    let random_num: u32 = rng.random_range(0..=0x100000);

    let random5 = format!("{:05x}", random_num);

    let uid = format!("guest0{:x}{}", timestamp, random5);

    format!("ngaPassportUid={};guestJs={}_igfndp", uid, timestamp)
}

pub fn get_nga_cookie() -> String {
    let uid = get_env_var("NGA_UID");
    let cid = get_env_var("NGA_CID");

    if uid.is_none() || cid.is_none() {
        return get_nga_guest_cookie();
    }

    format!(
        "ngaPassportUid={};ngaPassportCid={}",
        uid.unwrap(),
        cid.unwrap()
    )
}

// ==== URL 处理 ====

/// 当链接参数同时存在pid和opt时，删除opt参数
pub fn preprocess_url(url: &str) -> String {
    // 解析URL
    if let Ok(mut parsed_url) = url::Url::parse(url) {
        let query_pairs: Vec<(String, String)> = parsed_url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        
        // 检查是否同时存在 pid 和 opt 参数
        let has_pid = query_pairs.iter().any(|(k, _)| k == "pid");
        let has_opt = query_pairs.iter().any(|(k, _)| k == "opt");
        
        if has_pid && has_opt {
            // 重建查询字符串，排除 opt 参数
            let filtered_pairs: Vec<(String, String)> = query_pairs
                .into_iter()
                .filter(|(k, _)| k != "opt")
                .collect();
            
            // 清空原有查询参数
            parsed_url.set_query(None);
            
            // 重新添加过滤后的参数
            if !filtered_pairs.is_empty() {
                let query_string = filtered_pairs
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                parsed_url.set_query(Some(&query_string));
            }
            
            return parsed_url.to_string();
        }
    }
    
    // 如果解析失败或不需要处理，返回原URL
    url.to_string()
}
