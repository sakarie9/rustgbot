use common::get_env_var;
use rand::Rng;
use regex::Regex;
use std::{
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

pub const NGA_UA: &str = "NGA_skull/6.0.5(iPhone10,3;iOS 12.0.1)";

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
/// 删除可能存在的page参数
pub fn preprocess_url(url: &str) -> String {
    // 解析URL
    if let Ok(mut parsed_url) = url::Url::parse(url) {
        let mut has_pid = false;
        let mut has_opt = false;
        let mut has_page = false;
        let mut needs_rebuild = false;
        
        // 在一次循环中检查参数并过滤
        let filtered_pairs: Vec<(String, String)> = parsed_url
            .query_pairs()
            .filter_map(|(k, v)| {
                let key = k.as_ref();
                // 检查是否存在 pid、opt 和 page
                if key == "pid" {
                    has_pid = true;
                }
                if key == "opt" {
                    has_opt = true;
                }
                if key == "page" {
                    has_page = true;
                }
                
                // page 参数总是删除
                if key == "page" {
                    needs_rebuild = true;
                    return None;
                }
                
                // 当同时存在 pid 和 opt 时，删除 opt
                if key == "opt" && has_pid {
                    needs_rebuild = true;
                    return None;
                }
                
                Some((k.into_owned(), v.into_owned()))
            })
            .collect();

        // 如果需要重建URL（page存在 或 pid和opt同时存在）
        if needs_rebuild || (has_pid && has_opt) {
            parsed_url.set_query(None);
            
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
