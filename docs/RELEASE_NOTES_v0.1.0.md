# Super Skill Router v0.1.0

首个 Windows x64 公开版本。

## 新增

- 本地索引与 SkillsMP 按需搜索。
- 完整 Skill 目录下载、扫描、打包和部署，不再只处理 `SKILL.md`。
- 可选跳过、快速和深度安全扫描；报告只提供风险提示，不强制阻止安装。
- 多端部署：Claude Code、Codex CLI、ChatGPT Desktop（Codex）和 Claude Desktop。
- Claude Desktop ZIP 打包与待上传指引。
- 需求转 Prompt：相关性匹配、缺口检测、手动 Skill 选择、实时预览与复制。
- 管理同步矩阵、按应用筛选、Markdown 预览、一键卸载和本地安装记录。
- 本地 ZIP 导入：安全解压完整 Skill 树，扫描后部署到所选目标端。
- 桌面 Agent 自动恢复：识别 ChatGPT Desktop（Codex）的错误终止，等待输入框可发送后自动发送自定义恢复指令。

## 安装包

- `Super Skill Router_0.1.0_x64-setup.exe`：推荐的 NSIS 安装程序。
- `Super Skill Router_0.1.0_x64_en-US.msi`：MSI 安装包。

## 验证

- `npm run build`
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`：53 passed，4 ignored

## 注意

- 仅支持 Windows x64。
- Claude Desktop 仅获得待上传 ZIP，不会显示为已安装。
- Codex CLI 与 ChatGPT Desktop 共用 `$CODEX_HOME\\skills`，部署时自动去重。
