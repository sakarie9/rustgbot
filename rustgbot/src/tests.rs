#[cfg(test)]
mod main_tests {
    use crate::*;
    use common::LinkProcessor;
    use processor_x::XLinkProcessor;

    #[tokio::test]
    async fn test_unified_interface() {
        let processors: Vec<Box<dyn LinkProcessor>> = vec![
            Box::new(XLinkProcessor),
            Box::new(BiliBiliProcessor),
            Box::new(NGALinkProcessor),
            Box::new(PixivLinkProcessor),
        ];

        let test_text = "Check out this tweet: https://x.com/user/status/123456789 and this bilibili video: https://b23.tv/abc123 and this pixiv art: https://pixiv.net/artworks/987654321";

        for processor in &processors {
            println!("Testing processor: {}", processor.name());

            for captures in processor.regex().captures_iter(test_text) {
                let matched_url = captures.get(0).unwrap().as_str();
                println!("  Found match: {}", matched_url);

                // 这里只是测试接口，不实际调用网络请求
                println!("  Regex pattern matches correctly");
            }
        }
    }

    #[tokio::test]
    async fn test_regex_patterns() {
        let test_cases = vec![
            ("https://x.com/user/status/123456789", "X/Twitter"),
            ("https://twitter.com/user/status/987654321", "X/Twitter"),
            ("https://b23.tv/abc123", "BiliBili"),
            ("https://bili2233.cn/xyz789", "BiliBili"),
            ("https://pixiv.net/artworks/123456", "Pixiv"),
            ("https://www.pixiv.net/artworks/789012", "Pixiv"),
            ("https://bbs.nga.cn/read.php?tid=123456", "NGA"),
            ("https://ngabbs.com/read.php?tid=789012", "NGA"),
        ];

        let processors: Vec<Box<dyn LinkProcessor>> = vec![
            Box::new(XLinkProcessor),
            Box::new(BiliBiliProcessor),
            Box::new(NGALinkProcessor),
            Box::new(PixivLinkProcessor),
        ];

        for (test_url, expected_processor) in test_cases {
            println!("Testing URL: {}", test_url);
            let mut found = false;

            for processor in &processors {
                if processor.regex().is_match(test_url) {
                    println!("  Matched by: {}", processor.name());
                    assert_eq!(
                        processor.name(),
                        expected_processor,
                        "URL {} should be handled by {} but was matched by {}",
                        test_url,
                        expected_processor,
                        processor.name()
                    );
                    found = true;
                    break;
                }
            }

            assert!(found, "URL {} was not matched by any processor", test_url);
        }
    }
}
