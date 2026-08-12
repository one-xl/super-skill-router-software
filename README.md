# Super Skill Router

面向 Windows 的 AI Agent Skill 工作台。用于发现、下载完整 Skill 目录、安全扫描、部署、同步管理，并将自然语言需求转换为可执行 Prompt。

支持 Windows x64。桌面自动恢复面向 ChatGPT Desktop（Codex）与 Claude Code Desktop。

## 功能

- 通过本地静态索引和 SkillsMP 发现 Skill，安装时保留完整目录，而非只下载 `SKILL.md`。
- 安装前可选择跳过、快速扫描或深度扫描；扫描提供风险提示，安装决定权始终在用户。
- 部署到 Claude Code、Codex CLI、ChatGPT Desktop（Codex）和 Claude Desktop。
- 使用相关性匹配将需求转换为结构化 Prompt，支持手动增删命中的 Skill。
- 在管理页查看多端同步矩阵、Markdown 预览、一键卸载和本地 ZIP Skill 导入。
- 监控桌面 Agent 的错误终止，等待输入框恢复可发送后自动发送自定义恢复指令。

## 下载与安装

在 GitHub Releases 下载任一 Windows x64 安装包：

| 文件 | 用途 |
| --- | --- |
| `Super Skill Router_0.1.0_x64-setup.exe` | 推荐，大多数用户直接运行 |
| `Super Skill Router_0.1.0_x64_en-US.msi` | MSI 部署或企业软件管理 |

首次打开可配置 SkillsMP API Key，也可以稍后在“设置”页配置。未配置时仍可使用本地索引、管理、导入和手动部署。

## 部署目标

| 目标端 | 部署方式 | 状态 |
| --- | --- | --- |
| Claude Code CLI | 复制完整目录到 `%USERPROFILE%\\.claude\\skills\\` | 已部署 |
| Codex CLI | 复制完整目录到 `$CODEX_HOME\\skills\\` | 已部署 |
| ChatGPT Desktop（Codex） | 与 Codex CLI 共用 `$CODEX_HOME\\skills\\`，自动去重 | 已部署 |
| Claude Desktop | 打包 ZIP 并打开所在目录 | 待上传 |

Claude Desktop 没有本地 Skill 目录，因此只生成待上传 ZIP，不会显示为已安装。软件也不会写入 Codex 的 `vendor_imports\\skills` 目录。

## 安全扫描

SkillSpector 通过 sidecar 扫描完整下载或导入目录：

- **跳过扫描**：直接进入部署选择。
- **快速扫描**：裁剪静态规则、YARA、行为与 MCP 检查。
- **深度扫描**：完整 SkillSpector 加 LLM 语义分析，需要在设置页配置 OpenAI 或 Anthropic 兼容 API。

报告包含风险评分、`SAFE` / `CAUTION` / `DO NOT INSTALL` 建议和问题列表。静态扫描可能误报，不会强制拦截安装。

## 桌面 Agent 自动恢复

设置页可选择是否随软件启动监控，并可自定义恢复文本，默认内容：

```text
继续并恢复 todo-list
```

当同一轮任务发生重连耗尽、HTTP 错误、超时、服务不可用或拒绝等错误终止时，软件等待桌面对话框回到可发送状态后才注入并自动发送。正常完成与正常取消不会触发。

## 本地 ZIP Skill 导入

在“管理”页选择“导入压缩包”。应用会验证 ZIP 路径、大小与安全性，拒绝目录穿越、符号链接和多 Skill 压缩包；找到唯一 `SKILL.md` 根目录后完整解压，扫描并部署到所选目标端。

## 从源码运行

前置条件：Windows 10/11 x64、Node.js 20+、Rust stable、MSVC C++ Build Tools。

```powershell
npm install
npm run tauri dev
```

验证与构建：

```powershell
npm run build
Set-Location src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
Set-Location ..
npm run tauri build
```

## 索引更新

`indexer/crawl.py` 构建静态索引；[`.github/workflows/update-index.yml`](.github/workflows/update-index.yml) 每日运行。索引记录固定 commit SHA 与完整文件清单，安装时下载目标子目录而不是克隆整个仓库。

```powershell
python -m pip install -r indexer/requirements.txt
$env:GITHUB_TOKEN = "<GitHub token>"
python indexer/crawl.py
```

## 凭据与限制

- SkillsMP、深度扫描与 Prompt 精炼密钥保存到 Windows Credential Manager，不写入项目数据库或 Git。
- 搜索本地索引不调用 GitHub Code Search API。
- 仅支持 Windows x64。
- Claude Desktop 必须由用户在 Settings → Capabilities → Skills 上传软件生成的 ZIP。
- 桌面自动恢复依赖目标客户端当前可访问的 UI 结构，客户端更新后可能需要适配。
