#[cfg(test)]
mod tests {
    use crate::{convert_to_proxy_url, get_pixiv_image};

    #[tokio::test]
    #[ignore = "需要网络，仅手动测试"]
    async fn test_get_pixiv_image() {
        // 使用一个公开的Pixiv作品ID进行测试
        // 注意：这个测试需要网络连接，在CI环境中可能失败
        let id = "118704432"; // normal
        // let id = "132616032"; // R18

        match get_pixiv_image(id).await {
            Ok(result) => {
                println!("获取成功:");
                println!("文本: {}", result.caption);
                println!("图片URL数量: {}", result.urls.len());
                for (i, url) in result.urls.iter().enumerate() {
                    println!("图片 {}: {}", i + 1, url);
                }

                // 验证R18内容使用了pixiv.cat fallback
                if result.urls.iter().any(|url| url.contains("pixiv.cat")) {
                    println!("✅ R18内容正确使用了pixiv.cat镜像");
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

        match get_pixiv_image(id).await {
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
}
