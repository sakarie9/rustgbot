# RusTGBot - Rust Telegram Bot

一个用 Rust 开发的多功能 Telegram Bot，主要用于链接处理和内容优化。

## 功能特性

🔗 **链接处理**

- **X/Twitter 链接优化**: 自动将 `x.com` 和 `twitter.com` 链接转换为 `fxtwitter.com`，提供更好的预览体验
- **B站短链接净化**: 解析 `b23.tv` 短链接，返回清理过追踪参数的原始链接
- **NGA 论坛预览**: 抓取 NGA 论坛帖子内容并生成图文预览

## 快速开始

### 环境要求

- Rust 1.70+
- 有效的 Telegram Bot Token

### 安装运行

1. **获取二进制**

    从 Releases 下载对应环境的二进制文件

2. **配置环境**

   ```bash
   # 创建 .env 文件并添加 Bot Token
   echo "TELOXIDE_TOKEN=your_bot_token_here" > .env
   ```

### Docker 部署

从项目中下载 compose.yaml，修改环境变量

```bash
docker compose up
```

## 使用方法

Bot 会自动监听群组和私聊中的消息，当检测到支持的链接时会自动处理：

### X/Twitter 链接

- **输入**: `https://x.com/user/status/123456`
- **输出**: `https://fxtwitter.com/user/status/123456`

### B站短链接

- **输入**: `https://b23.tv/abcd123`
- **输出**: `https://www.bilibili.com/video/BV1234567890`

### NGA 论坛链接

- **输入**: `https://bbs.nga.cn/read.php?tid=12345`
- **输出**: 帖子标题、内容摘要和相关图片

### 配置选项

| 环境变量 | 说明 | 必需 |
|---------|------|------|
| `TELOXIDE_TOKEN` | Telegram Bot Token | ✅ |
| `NGA_UID` | NGA cookie 用于登录可看的帖子的访问 | ❌ |
| `NGA_UID` | NGA cookie 用于登录可看的帖子的访问 | ❌ |
