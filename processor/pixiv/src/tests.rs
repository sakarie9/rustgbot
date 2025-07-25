#[cfg(test)]
mod tests {
    use crate::{
        get_pixiv,
        utils::{build_pixiv_caption, convert_to_proxy_url},
    };

    #[tokio::test]
    #[ignore = "需要网络，仅手动测试"]
    async fn test_get_pixiv() {
        // 使用一个公开的Pixiv作品ID进行测试
        // 注意：这个测试需要网络连接，在CI环境中可能失败
        let id = "116383713"; // normal
        // let id = "132616032"; // R18

        match get_pixiv(id).await {
            Ok(result) => {
                println!("获取成功:");
                println!("文本: {}", result.caption);
                println!("图片URL数量: {}", result.urls.len());
                for (i, url) in result.urls.iter().enumerate() {
                    println!("图片 {}: {}", i + 1, url);
                }
            }
            Err(e) => {
                println!("获取失败: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "需要网络，仅手动测试"]
    async fn test_r18_multi_page() {
        // 测试多张图片的R18内容
        let id = "126189425"; // 多张图片的R18作品

        match get_pixiv(id).await {
            Ok(result) => {
                println!("多张R18图片测试:");
                println!("图片数量: {}", result.urls.len());
                for (i, url) in result.urls.iter().enumerate() {
                    println!("图片 {}: {}", i + 1, url);

                    // 检查是否使用了正确的pixiv.cat格式
                    if url.contains("pixiv.cat") {
                        if result.urls.len() > 1 {
                            // 多张图片应该包含页码
                            assert!(url.contains(&format!("{}-", id)) || !url.contains("-"));
                        } else {
                            // 单张图片不应该包含页码
                            assert!(!url.contains("-"));
                        }
                    }
                }
            }
            Err(e) => {
                println!("多张R18图片测试失败: {}", e);
            }
        }
    }

    #[test]
    fn test_convert_to_proxy_url() {
        // 测试成功的URL转换情况
        let success_test_cases = vec![
            (
                "https://i.pximg.net/img-original/img/2023/12/25/12/00/00/114514_p0.jpg",
                "https://i.pixiv.cat/img-original/img/2023/12/25/12/00/00/114514_p0.jpg",
            ),
            (
                "https://i.pximg.net/c/600x1200_90_webp/img-master/img/2023/12/25/12/00/00/114514_p0_master1200.jpg",
                "https://i.pixiv.cat/c/600x1200_90_webp/img-master/img/2023/12/25/12/00/00/114514_p0_master1200.jpg",
            ),
            (
                "https://i.pximg.net/img-master/img/2023/01/01/00/00/00/123456_p0_master1200.jpg",
                "https://i.pixiv.cat/img-master/img/2023/01/01/00/00/00/123456_p0_master1200.jpg",
            ),
            // 测试非Pixiv域名的URL应该保持不变
            (
                "https://other-domain.com/image.jpg",
                "https://other-domain.com/image.jpg",
            ),
            (
                "https://example.com/path/to/image.png",
                "https://example.com/path/to/image.png",
            ),
        ];

        println!("测试成功的URL转换:");
        for (original, expected) in success_test_cases {
            let result = convert_to_proxy_url(original).expect("URL转换应该成功");
            assert_eq!(result, expected);
            println!("✓ 原URL: {}", original);
            println!("  代理URL: {}", result);
        }

        println!("\n测试边缘情况:");

        // 测试包含i.pximg.net但在不同位置的URL
        let edge_cases = vec![
            "https://subdomain.i.pximg.net/image.jpg",
            "https://example.com/i.pximg.net/path.jpg",
            "https://i.pximg.net.fake.com/image.jpg",
        ];

        for url in edge_cases {
            match convert_to_proxy_url(url) {
                Ok(result) => {
                    println!("✓ 边缘情况URL: {}", url);
                    println!("  转换结果: {}", result);
                    // 验证结果应该包含代理域名
                    if url.contains("i.pximg.net") {
                        assert!(result.contains("i.pixiv.cat"));
                    }
                }
                Err(e) => {
                    println!("✗ 边缘情况URL转换失败: {} - 错误: {}", url, e);
                }
            }
        }
    }

    #[test]
    fn test_build_pixiv_caption() {
        use crate::models::{PixivIllustBody, PixivTag, PixivTags, PixivUrls};

        // 测试完整信息的情况
        let body_with_all_info = PixivIllustBody {
            id: "123456".to_string(),
            title: "测试标题".to_string(),
            user_id: "654321".to_string(),
            user_name: "测试作者".to_string(),
            description: "<p>这是一个测试<br>描述</p>".to_string(),
            page_count: 1,
            urls: PixivUrls { original: None },
            tags: Some(PixivTags {
                tags: vec![
                    PixivTag {
                        tag: "标签1".to_string(),
                    },
                    PixivTag {
                        tag: "标签2".to_string(),
                    },
                    PixivTag {
                        tag: "标签3".to_string(),
                    },
                ],
            }),
            x_restrict: 0,
        };

        let result = build_pixiv_caption(&body_with_all_info).expect("应该成功构建文本");
        println!("完整信息测试结果:\n{}", result);

        assert!(result.contains("测试标题"));
        assert!(result.contains("测试作者"));
        assert!(result.contains("这是一个测试"));
        assert!(result.contains("#标签1, #标签2, #标签3"));

        // 测试只有基本信息的情况
        let body_basic = PixivIllustBody {
            id: "123456".to_string(),
            title: "简单标题".to_string(),
            user_id: "654321".to_string(),
            user_name: "简单作者".to_string(),
            description: "".to_string(), // 空描述
            page_count: 1,
            urls: PixivUrls { original: None },
            tags: None, // 无标签
            x_restrict: 0,
        };

        let result_basic = build_pixiv_caption(&body_basic).expect("应该成功构建基本文本");
        println!("\n基本信息测试结果:\n{}", result_basic);

        assert!(result_basic.contains("简单标题"));
        assert!(result_basic.contains("简单作者"));
        assert!(!result_basic.contains("描述")); // 不应该包含描述
        assert!(!result_basic.contains("标签")); // 不应该包含标签

        // 测试空标签列表的情况
        let body_empty_tags = PixivIllustBody {
            id: "123456".to_string(),
            title: "无标签作品".to_string(),
            user_id: "654321".to_string(),
            user_name: "作者名".to_string(),
            description: "有描述但无标签".to_string(),
            page_count: 1,
            urls: PixivUrls { original: None },
            tags: Some(PixivTags { tags: vec![] }), // 空标签列表
            x_restrict: 0,
        };

        let result_empty_tags = build_pixiv_caption(&body_empty_tags).expect("应该成功构建文本");
        println!("\n空标签测试结果:\n{}", result_empty_tags);

        assert!(result_empty_tags.contains("有描述但无标签"));
        assert!(!result_empty_tags.contains("标签:")); // 不应该包含标签行
    }
}
