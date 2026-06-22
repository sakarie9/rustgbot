#[cfg(test)]
mod nga_tests {
    use crate::bbcode::RichBBCodeParser;
    use crate::page::escape_html;
    use crate::utils::*;
    use crate::*;
    use common::{SUMMARY_NORMAL_LIMIT, SUMMARY_TELEGRAM_LIMIT, substring_desc};
    use dotenv::dotenv;

    #[tokio::test]
    #[ignore = "需要网络，仅手动测试"]
    async fn test_get_nga_page() {
        dotenv().ok();
        let url = "https://bbs.nga.cn/read.php?pid=834397272"; // quote example
        // let url = "https://ngabbs.com/read.php?tid=44662667";
        // let url = "https://ngabbs.com/read.php?tid=44416669";
        // let url = "https://ngabbs.com/read.php?tid=21929866";
        // let url = "https://ngabbs.com/read.php?tid=41814733";
        // let page = NGAFetcher::fetch_page(url).await.ok().unwrap();
        let page = NGAFetcher::fetch_page(url).await;
        let page = if let Ok(p) = page {
            p
        } else {
            panic!("无法获取页面: {}", page.err().unwrap());
        };

        println!("标题: {}", page.title);
        println!("内容: {}", page.to_rich_html());
    }

    #[test]
    fn test_img_link_process() {
        // 测试已经是完整 URL 的情况
        let full_url = "https://example.com/image.jpg";
        assert_eq!(img_link_process(full_url), full_url);

        let http_url = "http://example.com/image.jpg";
        assert_eq!(img_link_process(http_url), http_url);

        // 测试需要处理的 NGA 图片链接
        let nga_link = "./mon_202301/01/abc123.jpg";
        let expected = "https://img.nga.178.com/attachments/mon_202301/01/abc123.jpg";
        assert_eq!(img_link_process(nga_link), expected);

        // 测试边界情况
        let short_link = "ab";
        assert_eq!(img_link_process(short_link), short_link);

        let empty_link = "";
        assert_eq!(img_link_process(empty_link), empty_link);

        // 测试特殊后缀处理 - .jpg.medium.jpg
        let medium_link = "https://img.nga.178.com/attachments/mon_202301/01/image.jpg.medium.jpg";
        let expected_medium = "https://img.nga.178.com/attachments/mon_202301/01/image.jpg";
        assert_eq!(img_link_process(medium_link), expected_medium);

        // 测试特殊后缀处理 - .jpg.thumb_s.jpg
        let thumb_link = "https://img.nga.178.com/attachments/mon_202301/01/image.jpg.thumb_s.jpg";
        let expected_thumb = "https://img.nga.178.com/attachments/mon_202301/01/image.jpg";
        assert_eq!(img_link_process(thumb_link), expected_thumb);

        // 测试其他文件格式的特殊后缀
        let png_link = "https://img.nga.178.com/attachments/test/image.png.medium.png";
        let expected_png = "https://img.nga.178.com/attachments/test/image.png";
        assert_eq!(img_link_process(png_link), expected_png);

        // 测试 NGA 相对链接 + 特殊后缀
        let nga_relative_link = "./mon_202301/01/test.jpg.thumb_s.jpg";
        let expected_nga_relative = "https://img.nga.178.com/attachments/mon_202301/01/test.jpg";
        assert_eq!(img_link_process(nga_relative_link), expected_nga_relative);

        // 测试只有一个点的文件名（不应该被处理）
        let single_dot = "https://example.com/image.jpg";
        assert_eq!(img_link_process(single_dot), single_dot);

        // 测试没有扩展名的文件（不应该被处理）
        let no_extension = "https://example.com/imagefile";
        assert_eq!(img_link_process(no_extension), no_extension);

        // 测试复杂的文件名
        let complex_link = "https://img.nga.178.com/attachments/path/my.image.file.jpg.medium.jpg";
        let expected_complex = "https://img.nga.178.com/attachments/path/my.image.file.jpg";
        assert_eq!(img_link_process(complex_link), expected_complex);

        // 测试没有路径分隔符的情况
        let no_slash = "image.jpg.medium.jpg";
        let expected_no_slash = "image.jpg.medium.jpg";
        assert_eq!(img_link_process(no_slash), expected_no_slash);
    }

    #[test]
    fn test_get_nga_guest_cookie() {
        let cookie = get_nga_guest_cookie();
        println!("Generated NGA guest cookie: {}", cookie);
        assert!(cookie.starts_with("ngaPassportUid=guest0"));
        assert!(cookie.contains(";guestJs="));
    }

    #[test]
    fn test_get_nga_cookie() {
        dotenv().ok();
        let cookie = get_nga_cookie();
        println!("Generated NGA cookie: {}", cookie);
        assert!(cookie.starts_with("ngaPassportUid="));
    }

    #[tokio::test]
    #[ignore = "需要网络，仅手动测试"]
    async fn test_get_nga_html() {
        dotenv().ok();
        // let url = "https://ngabbs.com/read.php?tid=21929866";
        let url = "https://ngabbs.com/read.php?tid=44416669";
        let html = get_nga_html(url).await;
        println!("Fetched NGA HTML: {}", html.unwrap_or_default());
    }

    #[test]
    fn test_parse_nga_page() {
        let html = r#"
            <html>
                <body>
                    <h3 id="postsubject0">Test Title</h3>
                    <p id="postcontent0">This is a test content.</p>
                </body>
            </html>
        "#;
        let page = parse_nga_page("test_url", html);
        assert!(page.is_some());
        let page = page.unwrap();
        let rich = page.to_rich_html();
        assert!(rich.contains("Test Title"));
        assert!(rich.contains("This is a test content."));
    }

    #[test]
    fn test_replace_html_entities() {
        // 测试 HTML 实体替换
        let input = "&quot;Hello&quot; &amp; &lt;world&gt; &nbsp;test&apos;";
        let expected = "\"Hello\" & <world>  test'";
        assert_eq!(replace_html_entities(input), expected);

        // 测试 BR 标签替换
        let input_br = "Line1<br/>Line2<br/>Line3";
        let expected_br = "Line1\nLine2\nLine3";
        assert_eq!(replace_html_entities(input_br), expected_br);

        // 测试空字符串
        assert_eq!(replace_html_entities(""), "");

        // 测试无需替换的字符串
        let unchanged = "This is a normal string";
        assert_eq!(replace_html_entities(unchanged), unchanged);
    }

    #[test]
    fn test_normalize_newlines() {
        // 测试多行换行符替换
        let input_newlines = "Line1\n\n\n\nLine2\n\n\n\n\nLine3";
        let expected_newlines = "Line1\n\nLine2\n\nLine3";
        assert_eq!(normalize_newlines(input_newlines), expected_newlines);

        // 测试空字符串
        assert_eq!(normalize_newlines(""), "");

        // 测试无需替换的字符串
        let unchanged = "This is a normal string with single\nlines";
        assert_eq!(normalize_newlines(unchanged), unchanged);
    }

    #[test]
    fn test_bbcode_parser_simple() {
        // 测试简单的粗体标签
        let input = "[b]Bold text[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<b>Bold text</b>");

        // 测试斜体标签
        let input = "[i]Italic text[/i]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<i>Italic text</i>");

        // 测试图片标签（Rich 解析器转为 <img> 标签）
        let input = "Before [img]test.jpg[/img] after";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "Before <img src=\"test.jpg\"/> after");
    }

    #[test]
    fn test_bbcode_parser_nested() {
        // 测试嵌套标签 - 这是新功能的核心测试
        let input = "[b]外层[i]内层斜体[/i]继续粗体[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<b>外层<i>内层斜体</i>继续粗体</b>");

        // 测试更复杂的嵌套
        let input = "[b]粗体[u]下划线[i]斜体[/i]继续下划线[/u]继续粗体[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(
            result,
            "<b>粗体<u>下划线<i>斜体</i>继续下划线</u>继续粗体</b>"
        );

        // 测试嵌套中包含图片标签
        let input = "[b]粗体[img]image.jpg[/img]继续粗体[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<b>粗体<img src=\"image.jpg\"/>继续粗体</b>");
    }

    #[test]
    fn test_bbcode_parser_url() {
        // 测试 URL 标签
        let input = "[url]https://example.com[/url]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(
            result,
            "<a href=\"https://example.com\">https://example.com</a>"
        );
    }

    #[test]
    fn test_bbcode_parser_quote() {
        // 测试引用标签（Rich 解析器用 <blockquote> 包裹，前后有段落分隔）
        let input = "[quote]引用内容[/quote]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "\n\n<blockquote>引用内容</blockquote>\n\n");
    }

    #[test]
    fn test_bbcode_parser_sticker() {
        // 测试表情标签（应该被移除）
        let input = "Hello [s:ac:赞同] world";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "Hello  world");

        // 测试另一个表情标签
        let input = "Test [s:ac:cry] more text";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "Test  more text");

        // 测试嵌套中的表情标签
        let input = "[b]粗体[s:ac:smile]继续粗体[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<b>粗体继续粗体</b>");
    }

    #[test]
    fn test_bbcode_parser_flash() {
        // 测试 flash 标签（应该被移除，只保留内容）
        let input = "[flash]https://www.bilibili.com/video/test[/flash]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "https://www.bilibili.com/video/test");

        // 测试带其他内容的 flash 标签
        let input = "查看视频: [flash]https://www.bilibili.com/video/BV123456[/flash] 精彩内容";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(
            result,
            "查看视频: https://www.bilibili.com/video/BV123456 精彩内容"
        );

        // 测试嵌套中的 flash 标签
        let input = "[b]粗体[flash]https://example.com/video[/flash]继续粗体[/b]";
        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();
        assert_eq!(result, "<b>粗体https://example.com/video继续粗体</b>");
    }

    #[test]
    fn test_clean_body_integration_with_nesting() {
        // 测试 RichContentCleaner 的完整清理流程
        // 注意：Rich 解析器不转义 HTML，&lt; 会被解码为 <
        let input = "&lt;b&gt;[b]粗体[i]斜体[/i]文本[/b] [img]test.jpg[/img] [url]https://example.com[/url]\n\n\n\n新行";
        let result = RichContentCleaner::clean(input);
        // &lt;b&gt; → <b>（解码后作为纯文本）
        assert!(result.contains("<b>"));
        assert!(result.contains("<b>粗体<i>斜体</i>文本</b>"));
        assert!(result.contains("<img src=\"test.jpg\"/>"));
        assert!(result.contains("<a href=\"https://example.com\">https://example.com</a>"));

        // 测试包含引用的复杂示例
        let complex_input = "[quote][b]粗体引用[i]斜体[/i][/b][/quote] &quot;文本&quot; [img]image.png[/img]\n\n\n\n新行";
        let complex_result = RichContentCleaner::clean(complex_input);
        assert!(complex_result.contains("<blockquote>"));
        assert!(complex_result.contains("<b>粗体引用<i>斜体</i></b>"));
        assert!(complex_result.contains("\"文本\""));
        assert!(complex_result.contains("<img src=\"image.png\"/>"));
    }

    #[test]
    fn test_clean_body_integration() {
        // 测试 RichContentCleaner 的完整清理流程
        let input = "&lt;b&gt;[b]粗体[i]斜体[/i]文本[/b] [img]test.jpg[/img] [url]https://example.com[/url]\n\n\n\n新行";
        let result = RichContentCleaner::clean(input);
        assert!(result.contains("<b>粗体<i>斜体</i>文本</b>"));
        assert!(result.contains("<img src=\"test.jpg\"/>"));
        assert!(result.contains("<a href=\"https://example.com\">"));

        // 测试引用标签
        let complex_input = "[quote][b]粗体引用[i]斜体[/i][/b][/quote] &quot;文本&quot; [img]image.png[/img]\n\n\n\n新行";
        let complex_result = RichContentCleaner::clean(complex_input);
        assert!(complex_result.contains("<blockquote>"));
        assert!(complex_result.contains("<b>粗体引用<i>斜体</i></b>"));
        assert!(complex_result.contains("\"文本\""));

        // 测试表情标签
        let sticker_input = "测试文本 [s:ac:赞同] 继续文本 [s:ac:cry] 结束";
        let sticker_result = RichContentCleaner::clean(sticker_input);
        assert_eq!(sticker_result, "测试文本  继续文本  结束");

        // 测试混合表情和其他标签
        let mixed_input = "[b]粗体[s:ac:smile]更多粗体[/b] [s:ac:赞同] 普通文本";
        let mixed_result = RichContentCleaner::clean(mixed_input);
        assert_eq!(mixed_result, "<b>粗体更多粗体</b>  普通文本");
    }

    #[test]
    fn test_performance_simple_vs_nested() {
        use std::time::Instant;

        // 测试简单标签的性能
        let simple_input = "[b]简单粗体[/b] [i]简单斜体[/i] [u]简单下划线[/u]".repeat(100);
        let start = Instant::now();
        for _ in 0..1000 {
            RichContentCleaner::clean(&simple_input);
        }
        let simple_duration = start.elapsed();
        println!("简单标签 1000次处理耗时: {:?}", simple_duration);

        // 测试嵌套标签的性能
        let nested_input = "[b]粗体[i]斜体[u]下划线[s]删除线[/s][/u][/i][/b]".repeat(100);
        let start = Instant::now();
        for _ in 0..1000 {
            RichContentCleaner::clean(&nested_input);
        }
        let nested_duration = start.elapsed();
        println!("嵌套标签 1000次处理耗时: {:?}", nested_duration);

        // 性能差异不应该超过 10 倍
        let ratio = nested_duration.as_nanos() as f64 / simple_duration.as_nanos() as f64;
        println!("嵌套/简单 性能比: {:.2}", ratio);
        assert!(ratio < 10.0, "嵌套处理性能下降过多，比例: {:.2}", ratio);
    }

    #[test]
    fn test_performance_deep_nesting() {
        use std::time::Instant;

        // 测试深度嵌套
        let mut deep_nested = String::new();
        let tags = vec!["b", "i", "u", "s", "del"];

        // 构建深度嵌套结构：[b][i][u][s][del]内容[/del][/s][/u][/i][/b]
        for tag in &tags {
            deep_nested.push_str(&format!("[{}]", tag));
        }
        deep_nested.push_str("深度嵌套内容");
        for tag in tags.iter().rev() {
            deep_nested.push_str(&format!("[/{}]", tag));
        }

        println!("深度嵌套测试字符串: {}", deep_nested);

        let start = Instant::now();
        for _ in 0..1000 {
            RichContentCleaner::clean(&deep_nested);
        }
        let duration = start.elapsed();
        println!("深度嵌套 1000次处理耗时: {:?}", duration);

        // 深度嵌套也应该在合理时间内完成（每次处理应该少于1ms）
        let avg_per_call = duration.as_nanos() / 1000;
        println!("平均每次处理耗时: {}ns", avg_per_call);
        assert!(
            avg_per_call < 1_000_000,
            "深度嵌套处理时间过长: {}ns",
            avg_per_call
        );
    }

    #[test]
    fn test_performance_large_input() {
        use std::time::Instant;

        // 测试大输入
        let large_input = format!(
            "这是一个很长的文本 {} [b]粗体内容[i]嵌套斜体[/i]继续粗体[/b] {} [img]image.jpg[/img] {} [s:ac:smile] {} [url]https://example.com[/url] {}",
            "普通文本".repeat(100),
            "更多文本".repeat(50),
            "中间文本".repeat(75),
            "结尾文本".repeat(25),
            "最终文本".repeat(150)
        );

        println!("大输入测试，字符数: {}", large_input.len());

        let start = Instant::now();
        for _ in 0..100 {
            RichContentCleaner::clean(&large_input);
        }
        let duration = start.elapsed();
        println!("大输入 100次处理耗时: {:?}", duration);

        // 大输入处理平均时间不应超过 10ms
        let avg_per_call = duration.as_millis() / 100;
        println!("平均每次处理耗时: {}ms", avg_per_call);
        assert!(avg_per_call < 10, "大输入处理时间过长: {}ms", avg_per_call);
    }

    #[test]
    fn test_performance_malformed_tags() {
        use std::time::Instant;

        // 测试畸形标签的处理性能（这些标签不会被解析为BBCode）
        let malformed_input =
            "[不完整标签 [b]正常[/b] [错误的标签] [i]正常斜体[/i] [/没有开始] 文本".repeat(50);

        let start = Instant::now();
        for _ in 0..1000 {
            RichContentCleaner::clean(&malformed_input);
        }
        let duration = start.elapsed();
        println!("畸形标签 1000次处理耗时: {:?}", duration);

        // 畸形标签处理不应该显著影响性能
        let avg_per_call = duration.as_nanos() / 1000;
        println!("平均每次处理耗时: {}ns", avg_per_call);
        assert!(
            avg_per_call < 2_000_000,
            "畸形标签处理时间过长: {}ns",
            avg_per_call
        );
    }

    #[test]
    fn test_performance_summary() {
        use std::time::Instant;

        println!("\n=== NGA BBCode 解析器性能报告 ===");

        // 测试数据
        let test_cases = vec![
            ("简单标签", "[b]粗体[/b] [i]斜体[/i]", 10000),
            ("嵌套标签", "[b]粗体[i]斜体[/i]文本[/b]", 10000),
            ("表情标签", "文本 [s:ac:赞同] [s:ac:cry] 文本", 10000),
            (
                "混合内容",
                "&quot;HTML&quot; [b]粗体[img]img.jpg[/img][/b] [s:ac:smile]",
                5000,
            ),
            (
                "深度嵌套",
                "[b][i][u][s][del]深层[/del][/s][/u][/i][/b]",
                5000,
            ),
        ];

        println!("\n📊 性能测试结果:");
        println!(
            "{:<12} {:<45} {:<12} {:<15} {:<15}",
            "测试类型", "输入示例", "迭代次数", "平均耗时(ns)", "每秒操作数"
        );
        println!("{}", "-".repeat(100));

        for (name, input, iterations) in test_cases {
            let start = Instant::now();
            for _ in 0..iterations {
                let _ = RichContentCleaner::clean(input);
            }
            let duration = start.elapsed();
            let avg_ns = duration.as_nanos() / iterations;
            let ops_per_sec = 1_000_000_000_u128.checked_div(avg_ns).unwrap_or(0);

            println!(
                "{:<12} {:<45} {:<12} {:<15} {:<15}",
                name,
                if input.len() > 40 {
                    format!("{}...", &input[..40])
                } else {
                    input.to_string()
                },
                iterations,
                avg_ns,
                ops_per_sec
            );
        }
    }

    #[test]
    fn test_substring_desc() {
        // 测试短文本，不需要截取
        let short_text = "这是一个短文本";
        assert_eq!(substring_desc(short_text), short_text);

        // 测试正好在正常限制内的文本
        let exact_length_text = "a".repeat(SUMMARY_NORMAL_LIMIT);
        let result = substring_desc(&exact_length_text);
        assert_eq!(result, exact_length_text);

        // 测试超过正常限制 → 全部内容放入可折叠引用
        let long_text = "a".repeat(SUMMARY_NORMAL_LIMIT + 100);
        let result = substring_desc(&long_text);
        assert!(
            result.starts_with("<blockquote "),
            "result starts with: {:?}",
            &result[..40.min(result.len())]
        );
        assert!(result.ends_with("</blockquote>"));
        assert!(result.ends_with("</blockquote>"));
        assert!(result.contains(&"a".repeat(SUMMARY_NORMAL_LIMIT + 100)));

        // 测试包含前后空白字符的文本（trim 行为）
        let text_with_spaces = format!("  {}  ", "内容".repeat(SUMMARY_NORMAL_LIMIT - 100));
        let result = substring_desc(&text_with_spaces);
        assert!(!result.starts_with(" "));
        assert!(result.starts_with("<blockquote "));
        assert!(result.ends_with("</blockquote>"));

        // 测试超过 Telegram 限制 → 截断后放入可折叠引用
        let oversized_text = "a".repeat(SUMMARY_TELEGRAM_LIMIT + 100);
        let result = substring_desc(&oversized_text);
        assert!(result.starts_with("<blockquote "));
        assert!(result.contains("……"));
        assert!(result.ends_with("</blockquote>"));
    }

    #[test]
    fn test_substring_desc_with_existing_blockquote() {
        // 内容已有 blockquote 标签且超过正常限制，不应再包裹新的 blockquote
        let content_with_bq = format!(
            "{}<blockquote>折叠内容</blockquote>",
            "a".repeat(SUMMARY_NORMAL_LIMIT + 100)
        );
        let result = substring_desc(&content_with_bq);
        // 不应额外包裹 blockquote
        assert_eq!(result, content_with_bq.trim());

        // 已有 blockquote 且超过 Telegram 限制 → 直接截断
        let oversized_with_bq = format!(
            "{}<blockquote>折叠内容</blockquote>",
            "b".repeat(SUMMARY_TELEGRAM_LIMIT + 100)
        );
        let result = substring_desc(&oversized_with_bq);
        assert!(
            !result.contains("<blockquote "),
            "should not wrap in extra blockquote"
        );
        assert!(result.ends_with("……"));
        // 不应包含完整的内容
        assert!(result.len() < oversized_with_bq.len());
    }

    #[test]
    fn test_preprocess_url_removes_opt_when_pid_exists() {
        let url = "https://example.com/path?pid=123&opt=456&other=789";
        let result = preprocess_url(url);
        assert_eq!(result, "https://example.com/path?pid=123&other=789");
    }

    #[test]
    fn test_preprocess_url_keeps_pid_when_no_opt() {
        let url = "https://example.com/path?pid=123&other=789";
        let result = preprocess_url(url);
        assert_eq!(result, url);
    }

    #[test]
    fn test_preprocess_url_pid_when_no_opt() {
        let url = "https://example.com/path?pid=123";
        let result = preprocess_url(url);
        assert_eq!(result, url);
    }

    #[test]
    fn test_bbcode_url_parsing() {
        // 测试带参数的URL: [url=https://x.com]推特[/url]
        let mut parser1 = RichBBCodeParser::new("[url=https://x.com]推特[/url]");
        let result1 = parser1.parse();
        assert_eq!(result1, "<a href=\"https://x.com\">推特</a>");

        // 测试不带参数的URL: [url]https://x.com[/url]
        let mut parser2 = RichBBCodeParser::new("[url]https://x.com[/url]");
        let result2 = parser2.parse();
        assert_eq!(result2, "<a href=\"https://x.com\">https://x.com</a>");

        // 测试混合内容
        let input = "访问[url=https://x.com]推特[/url]或者直接点击[url]https://github.com[/url]";
        let mut parser3 = RichBBCodeParser::new(input);
        let result3 = parser3.parse();
        let expected = "访问<a href=\"https://x.com\">推特</a>或者直接点击<a href=\"https://github.com\">https://github.com</a>";
        assert_eq!(result3, expected);

        // 测试嵌套标签
        let mut parser4 = RichBBCodeParser::new("[url=https://example.com][b]粗体链接[/b][/url]");
        let result4 = parser4.parse();
        assert_eq!(
            result4,
            "<a href=\"https://example.com\"><b>粗体链接</b></a>"
        );
    }

    #[test]
    fn test_table_parsing() {
        let input = r#"[table]
[tr]
  [td40]樋口枫[/td]
  [td40]VR关西圈立高校[/td]
  [td]滋贺[/td]
[/tr]
[tr]
  [td40]叶[/td]
  [td40]私立愿丘高校[/td]
  [td]京都[/td]
[/tr]
[tr]
  [td40]艾克斯·阿尔比欧[/td]
  [td40]英雄Academy[/td]
  [td]鸟取[/td]
[/tr]
[tr]
  [td40]葛叶[/td]
  [td40]神速高校[/td]
  [td]兵库[/td]
[/tr]
[tr]
  [td40]椎名唯华[/td]
  [td40]彩虹高校[/td]
  [td]岩手[/td]
[/tr]
[tr]
  [td40]雷奥斯·文森特[/td]
  [td40]青春豆猫学园[/td]
  [td]爱知[/td]
[/tr]
[tr]
  [td40]笹木咲[/td]
  [td40]Panda立Doja高校[/td]
  [td]宫崎[/td]
[/tr]
[tr]
  [td40]莉泽·赫露艾斯塔[/td]
  [td40]王立赫露艾斯塔高校[/td]
  [td]静冈[/td]
[/tr]
[tr]
  [td40]伊卜拉新[/td]
  [td40]帝国立Covas高校[/td]
  [td]秋田[/td]
[/tr]
[/table]"#;

        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();

        println!("用户提供的表格示例：");
        println!("原始输入:\n{}", input);
        println!("\n解析结果:\n{}", result);

        // 基本检查
        assert!(result.contains("樋口枫"));
        assert!(result.contains("VR关西圈立高校"));
    }

    #[test]
    fn test_table_simple() {
        let input = "[table][tr][td]第一列[/td][td]第二列[/td][/tr][/table]";

        let mut parser = RichBBCodeParser::new(input);
        let result = parser.parse();

        println!("简单表格输入: {}", input);
        println!("简单表格结果: {}", result);

        assert!(result.contains("第一列"));
        assert!(result.contains("第二列"));
    }

    #[test]
    fn test_collapse_tags() {
        // 测试带标题的 collapse 标签（Rich 解析器用 <details>）
        let input_with_title = "[collapse=详细内容]这是折叠的内容[/collapse]";
        let mut parser = RichBBCodeParser::new(input_with_title);
        let result = parser.parse();

        println!("带标题的collapse输入: {}", input_with_title);
        println!("带标题的collapse结果: {}", result);

        assert!(result.contains("<details>"));
        assert!(result.contains("<summary>详细内容</summary>"));
        assert!(result.contains("这是折叠的内容"));
        assert!(result.contains("</details>"));

        // 测试无标题的 collapse 标签（passthrough，直接输出内容）
        let input_without_title = "[collapse]这是折叠的内容[/collapse]";
        let mut parser2 = RichBBCodeParser::new(input_without_title);
        let result2 = parser2.parse();

        println!("无标题的collapse输入: {}", input_without_title);
        println!("无标题的collapse结果: {}", result2);

        assert_eq!(result2, "这是折叠的内容");
    }

    #[test]
    fn test_html_escape() {
        // 测试escape_html函数
        let test_cases = vec![
            ("<text>", "&lt;text&gt;"),
            ("a < b", "a &lt; b"),
            ("a > b", "a &gt; b"),
            ("a & b", "a &amp; b"),
            (
                "<script>alert('xss')</script>",
                "&lt;script&gt;alert('xss')&lt;/script&gt;",
            ),
            ("normal text", "normal text"),
            (
                "<a>test</a> & <b>data</b>",
                "&lt;a&gt;test&lt;/a&gt; &amp; &lt;b&gt;data&lt;/b&gt;",
            ),
        ];

        for (input, expected) in test_cases {
            let result = escape_html(input);
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_rich_html_title_escape() {
        // 测试标题包含HTML特殊字符时的转义
        let html = r#"
            <html>
                <body>
                    <h3 id="postsubject0">测试标题 &lt;text&gt; 包含尖括号</h3>
                    <p id="postcontent0">正常内容</p>
                </body>
            </html>
        "#;
        let page = parse_nga_page("https://bbs.nga.cn/test", html);
        assert!(page.is_some());
        let rich = page.unwrap().to_rich_html();
        // 验证标题中的尖括号被转义
        assert!(rich.contains("&lt;text&gt;"));
    }
}
