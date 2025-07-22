#[cfg(test)]
mod main_tests {
    use crate::*;

    #[test]
    fn test_process_x_link_valid_x_com() {
        // 测试 x.com 链接
        let captures = vec!["https://x.com/testuser/status/9876543210", "testuser", "9876543210"];
        let result = process_x_link(&captures);
        
        assert_eq!(result.original, "https://x.com/testuser/status/9876543210");
        
        match result.processed {
            Some(BotResponse::Text(processed)) => {
                assert_eq!(processed, "https://fxtwitter.com/testuser/status/9876543210");
            }
            _ => panic!("Expected Text response"),
        }
    }

    #[test]
    fn test_process_x_link_insufficient_captures() {
        // 测试捕获组不足的情况
        let captures = vec!["https://x.com/user/status/"];
        let result = process_x_link(&captures);
        
        assert_eq!(result.original, "https://x.com/user/status/");
        
        match result.processed {
            Some(BotResponse::Error(error)) => {
                assert!(error.contains("Failed to process X link"));
                assert!(error.contains("https://x.com/user/status/"));
            }
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_x_link_regex_pattern() {
        // 测试正则表达式是否正确匹配各种X链接格式
        let regex = regex::Regex::new(REGEX_X_LINK).unwrap();

        // 应该匹配的链接
        let valid_links = vec![
            "https://twitter.com/user/status/123456",
            "https://www.twitter.com/user/status/123456",
            "https://x.com/user/status/123456",
            "http://twitter.com/user/status/123456",
            "twitter.com/user/status/123456",
            "x.com/user/status/123456",
        ];

        for link in valid_links {
            assert!(regex.is_match(link), "Should match: {}", link);
            let captures = regex.captures(link).unwrap();
            assert_eq!(captures.len(), 3); // 全匹配 + 2个捕获组
        }

        // 不应该匹配的链接
        let invalid_links = vec![
            "https://fixupx.com/user/status/123456", // 应该被 \b 边界阻止
            "https://twitter.com/user/tweet/123456", // 不是 status
            "https://x.com/user/status/", // 缺少状态ID
            "https://x.com/status/123456", // 缺少用户名
        ];

        for link in invalid_links {
            assert!(!regex.is_match(link), "Should not match: {}", link);
        }
    }
}