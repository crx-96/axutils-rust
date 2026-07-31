# SMTP 邮件能力发布说明草稿

## 兼容性与依赖

- 最低支持 Rust 版本从 1.76 提升到 1.85。这是下游可见的兼容性变化，使用 Rust 1.76—1.84
  的项目升级 `axutils` 后需要先升级工具链。
- 新增显式 `lettre` feature，最低依赖版本为包含 SMTP 响应读取缓冲区修复的
  `lettre 0.11.22`；manifest 使用最低兼容版本约束，允许 Cargo 解析后续兼容的
  `0.11.x` 版本；默认 feature 仍为空。
- 新增显式 `tokio` feature。同步邮件只需要 `lettre`；异步邮件必须显式同时启用
  `lettre,tokio`。单独启用 `tokio` 不会导出邮件 API 或自动启用 `lettre`。
- 邮件 TLS 后端固定为 Rustls、`ring` 和 `webpki-roots`，常见 Linux 构建不需要 OpenSSL
  开发包、`pkg-config`、CMake、Go 或邮件专用系统 CA 包。

## 新增 API

- `EmailConfig`：校验 SMTP DNS 主机名、端口、凭据、发件地址、显示名和命令超时。
- `EmailMessage`：构造受限的纯文本或 HTML 邮件，支持一个或多个 `To` 收件人。
- `EmailClient`：可创建多个独立账号实例，复用同步连接池；组合启用 `tokio` 时还提供
  `send_async`；异步连接池在首次发送时于调用方 runtime 中惰性初始化，不会创建内部
  runtime。
- `EmailUtils`：一次初始化、不可重置的单默认账号全局入口；多账号应使用 `EmailClient`。
- `EmailError`：提供不回显密码、主题、正文、用户名、完整主机名或地址的稳定错误分类。

## 安全边界与迁移提示

- 只支持强制 SMTPS 和强制 STARTTLS，不支持明文 SMTP、机会式 STARTTLS、跳过证书校验、
  自签名证书或企业私有 CA relay。
- 配置、邮件头和消息正文有明确字节/数量上限；主题、显示名和地址中的控制字符会被拒绝。
- `EmailMessage` 会被发送方法消费；同步发送是阻塞 I/O，异步发送需要调用方已有 Tokio
  runtime，缺少 runtime 时返回客户端错误而不是 panic。该版本不提供自动重试、后台队列、
  附件、抄送、密送、模板、DKIM、OAuth2 或收件能力。
- 不要把应用专用密码或 SMTP 授权码硬编码、写入 SMTP URL、命令行、日志或 Git；真实测试
  配置固定放在本地 `config/email-test.toml`，该目录整体被 `.gitignore` 忽略，且不随 crate
  发布。

## 平台与部署说明

CI 目标覆盖 Windows、Linux、macOS 的 Rust 1.85 和 stable，并且验证过程不连接外部 SMTP relay。README 提供 Debian/
Ubuntu、Alpine、Fedora/RHEL、Windows 和 macOS 的基础构建工具清单，以及消费方自行替换的
Debian/Ubuntu、Alpine 多阶段 Docker 模板。`axutils` 是 library crate，本项目不提供业务
Dockerfile、容器产物或已验证的具体应用镜像；Alpine/musl、ARM64、WASM、Android、iOS 和
FreeBSD 不属于首期承诺范围。

真实 SMTP 测试只有在用户提供本地配置并明确授权发送后才运行；未授权时保持 `#[ignore]`，
不能将缺少真实凭据描述为普通单元测试失败。
