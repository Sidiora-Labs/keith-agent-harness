<h1 align="center">Keith</h1>

<p align="center">
  <strong>第一个真正进化的智能体——不是靠提示词技巧,而是通过修改、测试并安全地升级自身的执行框架（harness）。</strong>
</p>

<p align="center">
  Keith 把真实经验转化为对其底层机器的改变——也就是决定它如何推理、选择工具、
  管理上下文并完成工作的那套机制——而不是在提示词里再加一条注释。每个新版本都在
  隔离环境中构建,对照当前 Keith 进行测试,只有在表现更好且不越过你设定的安全
  边界时才会被采纳。
</p>

<p align="center">
  <a href="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml"><img src="https://github.com/Sidiora-Labs/keith-agent/actions/workflows/ci.yml/badge.svg" alt="CI 状态"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/releases/latest"><img src="https://img.shields.io/github/v/release/Sidiora-Labs/keith-agent?display_name=tag" alt="最新发布"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/stargazers"><img src="https://img.shields.io/github/stars/Sidiora-Labs/keith-agent?style=flat" alt="GitHub Stars"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="许可证: Apache-2.0"></a>
  <a href="https://github.com/Sidiora-Labs/keith-agent/pkgs/container/keith-agent"><img src="https://img.shields.io/badge/container-GHCR-blue" alt="GHCR 镜像"></a>
</p>

<p align="center">
  <a href="docs/installation.md">快速开始</a> ·
  <a href="docs/deployment.md">部署</a> ·
  <a href="docs/crate-guide.md">架构</a> ·
  <a href="CONTRIBUTING.md">参与贡献</a> ·
  <a href="SECURITY.md">安全</a>
</p>

<p align="center">
  <img src="docs/assets/keith-harness-repair.png" alt="Keith 在隔离环境中测试对自身执行框架的修复" width="1100">
</p>

<p align="center"><sub>从真实工作中学习。打造更好的执行框架。在上线前证明改进有效。</sub></p>

> [!IMPORTANT]
> Keith 仍是预发布软件。核心系统已经可用,但接口、存储和打包方式在 1.0
> 之前仍可能变化。

## 试一试

最简单的方式是 Docker:

```bash
cp .env.example .env
# 在 .env 中设置 KEITH_WEB_LOGIN_SECRET 和至少一个模型提供商的密钥。
docker compose up --build
```

打开 <http://localhost:7341>。Keith 将其状态保存在 `keith-data` 卷中,并可以
在当前工作树的 `/workspace` 路径下工作。

想从源码开发?

```bash
./keith doctor
./keith setup
./keith dev
```

请参阅[安装指南](docs/installation.md),其中包含 TUI、提供商配置、升级、备份
与服务管理等内容。

## Keith 如何进化

每一次运行给 Keith 带来的不仅仅是一份日志。它会产生关于整个智能体工作情况的
证据:推理路径、工具选择、上下文使用、延迟、成本、恢复过程以及最终结果。

一次硬性失败可能暴露进化的机会,但一次浪费的工具调用、一次缓慢的恢复、一个本应
更好的结果,同样可以。

Keith 把最强的机会转化为一个可验证的假设,在远离线上系统的环境中构建若干候选
框架,并使用提案者看不到的任务将其与当前版本进行对比。只有当候选版本带来可衡量
的改进且没有引入回归时,它才会被采用。

```text
经验 → 机会 → 可验证的假设 → 候选框架
     → 留出评估集评估 → 灰度 → 观察 → 保留或回滚
```

这就是 Keith 背后的核心赌注:智能体不仅应该完成工作,还应该具备一种安全、
可审计的方式来变得更能完成工作。

### 在它工作的同时进行纠正

Keith Computer 是一个可见的、隔离的桌面。实时观察运行过程,一键接管键盘
和鼠标,解决问题后再交还控制权。独占式控制租约可以避免你与 Keith 争抢同一
个屏幕。

### 教它工作,而不是再写一个提示词

当你演示一项任务时,Keith 会记录其中有用的结构:屏幕状态、键盘和鼠标操作、
UI 目标、应用上下文、时序、叙述、文件、剪贴板活动以及每一次控制权交接。
它把这些证据转化成一个可编辑的 **TaskRecipe**,其中包含输入、检查点、审批
步骤、恢复步骤、版本与回滚。

你给 Keith 的不是一份简单的录屏,而是一段它可以重放并改进的工作。

### 让它修复执行框架,而不是修改规则

只有当候选方案无法移动"球门柱"时,自我修复才有意义。Keith 的修复候选不能
编辑评判器、留出的测试用例、凭据、Keith 可以记住或透露的内容、你的审批规则、
发布检查、晋升闸门或回滚路径。

选择 Keith 可以走多远:

- **仅建议**   解释提议的修复,然后等待。
- **影子测试**   构建并测试候选,但不晋升任何候选。
- **自主修复**   在你设定的范围内进行灰度、观察和回滚。

每种模式下,受保护的规则都处于修复候选不可触及的位置。

### 一个 Keith,而不是一堆机器人

Web UI、TUI、兼容 OpenAI 的 API、原生 API、ACP 客户端、消息渠道、连接的应用
和计算机,都连接到同一个由守护进程拥有的智能体。专业化的 worker 可以在后台
研究、编码或操作工具,而不必把产品变成一个需要你去管理各种"人格"的仪表板。

会话在客户端断开后仍可存活。承诺、等待、计划任务、目标和正在运行的会话可以在
重启后恢复。在终端开始,到 Slack 跟进,再到浏览器收尾——不需要创建三个互不相关
的助手。

## 你能用 Keith 做什么

| 你想… | Keith 可以… |
| --- | --- |
| 委派一项浏览器或桌面任务 | 在有界面或无界面计算机中工作,你可以观察、暂停或接管 |
| 教会一个可重复的工作流 | 把一次现场演示转化为可编辑、可重放的 TaskRecipe |
| 停止重复同一个智能体失败 | 诊断执行框架、对比候选修复、灰度赢家,并可回滚 |
| 使用你自己的模型 | 通过 OpenAI、Anthropic、OpenRouter、Ollama 或其他受支持的提供商路由配置 |
| 从任何地方访问同一个智能体 | 同时服务 Web、TUI、ACP、API、Discord、Slack、Telegram、WhatsApp、Teams、Google Chat、邮件和 Matrix 客户端 |
| 连接真实服务 | 使用需审批的连接应用、Composio、MCP 服务器,以及能力受限的 WASI 插件 |
| 基于 Keith 进行构建 | 使用兼容 OpenAI 的 `/v1` API,或类型化的原生 `/platform/v1` API |
| 部署在你自己的基础设施上 | 通过 Docker、Kubernetes、Railway、Fly.io、DigitalOcean、Azure、AWS 或 Google Cloud 部署 |

## 同一个运行时,多种入口

```text
Web · TUI · OpenAI API · Platform API · ACP · 渠道
                         │
                      agentd
              会话 · 策略 · 恢复
                         │
                 租用模式下的智能体 worker
                         │
       模型 · 工具 · 插件 · CUA · 连接的应用
```

`agentd` 掌握事实来源。客户端只是渲染它的会话和生命周期,而不是各自维护自己的
状态。Worker 在租约下执行回合,领域 crate 把策略与外部适配器分离开。

阅读 [crate 指南](docs/crate-guide.md) 和
[依赖边界](docs/architecture/dependency-boundaries.md) 以了解完整的系统结构。

## API 与扩展

Keith 暴露两个 HTTP 表面:

- **兼容 OpenAI 的 `/v1`** ,用于现有 SDK 和 Open WebUI 等工具。
- **原生 `/platform/v1`** ,用于需要 Keith 会话、生命周期、审批、制品和实时
  事件的受信客户端。

扩展可以作为能力受限的 WASI 组件、MCP 服务器、技能或需要审批的连接应用来运行。
ACP 客户端可以通过内置的 stdio 服务器进行连接。参阅
[OpenAI 兼容性](docs/openai-compatibility.md) 和
[平台集成](docs/platform-integration.md)。

## 安全

> [!WARNING]
> Keith 可以运行命令、修改文件、控制浏览器,并以你授予的权限调用外部服务。
> 请使用一个可以被检查和恢复的工作空间。模型输出、渠道消息、抓取的页面、
> 插件、技能、MCP 服务器和修复候选均视为不可信。

除非你额外配置 TLS、强身份认证和明确的网络策略,否则请将 Web UI 和 API
保留在 loopback 上。针对 Web 登录、API、模型提供商和发布签名使用不同的
密钥。切勿在公开 issue 中发布凭据或未经脱敏的运行轨迹。

请通过 GitHub 的
[安全公告表单](https://github.com/Sidiora-Labs/keith-agent/security/advisories/new)
私下报告漏洞。请阅读 [SECURITY.md](SECURITY.md) 了解信任模型、报告范围和
上报规则。

## 部署

Keith 以单一有状态 OCI 镜像发布,支持 Docker Compose、带 Helm 的 Kubernetes、
Railway、Fly.io、DigitalOcean Kubernetes、Azure Kubernetes Service、Amazon EKS
以及 Google Kubernetes Engine Autopilot。

```bash
./keith deploy kubernetes --render
./keith deploy railway
./keith deploy fly --app my-keith
./keith deploy aws --cluster keith --region us-east-1
```

云命令默认只输出计划。只有当传入 `--execute` 并设置
`KEITH_DEPLOY_APPROVED=YES` 时,部署才会真正改变基础设施。在将 Keith 暴露到
宿主机外部之前,请先通读完整的[部署指南](docs/deployment.md)。

## 开发与扩展

`./keith` 命令是贡献者的统一入口,涵盖环境配置、本地服务、检查、测试、发布
构建、容器镜像、脚手架以及部署计划。Rust 构建产物不会写入工作树。

```bash
./keith check
./keith test
./keith image keith-agent:dev
./keith scaffold plugin my-plugin
./keith scaffold skill my-skill
```

需要 Rust 1.93、Node.js 22.22、Corepack 和 Git。在提交 Pull Request 之前,
请阅读 [CONTRIBUTING.md](CONTRIBUTING.md),并写明你实际运行过的命令和真实
用户路径。

## 文档

| 目标 | 从这里开始 |
| --- | --- |
| 安装、配置提供商或运行 TUI | [安装与生命周期](docs/installation.md) |
| 使用 Docker 或云提供商运行 | [部署指南](docs/deployment.md) |
| 连接 OpenAI SDK 或 Open WebUI | [OpenAI 兼容性](docs/openai-compatibility.md) |
| 集成受信的原生客户端 | [平台集成](docs/platform-integration.md) |
| 理解工作区结构 | [Crate 指南](docs/crate-guide.md) |
| 发布前的质量验证 | [发布验证](docs/release-qualification.md) |
| 寻求帮助或报告问题 | [支持](SUPPORT.md) |

## 社区

- 在 [GitHub Discussions](https://github.com/Sidiora-Labs/keith-agent/discussions)
  中提问和分享想法。
- 通过 [issue 模板](https://github.com/Sidiora-Labs/keith-agent/issues/new/choose)
  报告可复现的 Bug。
- 安全问题请私下报告,不要发到公开 issue。
- 在所有项目空间中遵守[行为准则](CODE_OF_CONDUCT.md)。

## 许可证

Keith 基于 [Apache License 2.0](LICENSE) 许可证发布。
