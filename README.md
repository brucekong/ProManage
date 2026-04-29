# ProStation

ProStation 是一个面向本地开发工作流的桌面项目管理工具。它基于 Vue 3、Naive UI、Pinia 和 Tauri 2 构建，用来集中管理多个前端、后端或工作区项目，快速启动命令、查看运行状态、处理端口冲突，并在同一个界面中查看实时终端输出。

## 功能特性

- 项目看板：集中展示本地项目、运行状态、分组、备注、收藏和 Git 分支状态。
- 项目导入：支持手动添加项目、拖拽文件夹或工作区文件，也可以读取 `package.json` 中的 scripts。
- 多目标启动：自动识别 `dev`、`start`、`serve`、`build` 以及常见的前端、后端脚本，并按 Web、API、任务等类型展示。
- 实时终端：使用 PTY 承载运行进程，支持输出流、快捷输入、窗口尺寸同步和日志保留。
- 端口管理：可配置端口范围，启动前检测端口占用，并支持释放冲突端口。
- Git 状态：展示项目所在仓库的分支、未提交变更、ahead/behind 信息。
- IDE 集成：支持通过命令别名打开 VS Code 或 Antigravity。
- 应用更新：内置 Tauri updater 配置，可从 GitHub Releases 更新源检查和安装新版本。
- 多语言：支持英文和中文界面。

## 技术栈

- 前端：Vue 3、TypeScript、Vite、Pinia、Naive UI、xterm.js
- 桌面端：Tauri 2、Rust
- 后端能力：进程管理、PTY、端口检测、配置持久化、日志、系统托盘、通知、更新器
- 发布：GitHub Actions + `tauri-action`

## 环境要求

请先安装以下环境：

- Node.js 20 或更高版本
- npm
- Rust stable
- Tauri 2 所需的系统依赖

macOS 下通常需要安装 Xcode Command Line Tools：

```bash
xcode-select --install
```

如需了解不同系统的 Tauri 依赖，请参考 Tauri 官方文档。

## 快速开始

安装依赖：

```bash
npm install
```

启动桌面开发环境：

```bash
npm run tauri dev
```

只启动前端开发服务器：

```bash
npm run dev
```

类型检查并构建前端：

```bash
npm run build
```

预览前端构建产物：

```bash
npm run preview
```

构建桌面应用：

```bash
npm run tauri build
```

## 使用方式

1. 打开 ProStation 后，在项目页点击添加，或将项目文件夹、`.code-workspace`、`.agworkspace` 文件拖入窗口。
2. 对普通项目，可以读取 `package.json` scripts，也可以手动填写启动命令。
3. 对工作区项目，ProStation 会优先从工作区文件和脚本命令中推断可用目录。
4. 在项目卡片中点击对应目标启动或停止进程。
5. 选中项目后，可在底部实时终端查看输出，并向进程发送输入。
6. 在设置页调整端口范围、日志保留天数、语言、主题、IDE 命令，并手动检查应用更新。

## 脚本说明

| 命令 | 说明 |
| --- | --- |
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 执行 `vue-tsc` 类型检查并构建前端 |
| `npm run preview` | 预览前端构建产物 |
| `npm run tauri` | 调用 Tauri CLI，可搭配 `dev`、`build` 等子命令 |
| `npm run version:bump -- <version>` | 同步更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 的版本号 |
| `npm run release:prepare -- <version>` | 更新版本号并创建对应的 `v<version>` Git tag |

## 项目结构

```text
.
├── src/                     # Vue 前端源码
│   ├── api/                 # Tauri command 调用封装
│   ├── components/          # 项目卡片、终端、表单等组件
│   ├── stores/              # Pinia 状态管理
│   ├── types/               # 前端类型定义
│   ├── utils/               # 端口检测、通知、运行时辅助逻辑
│   └── views/               # 项目、日志、设置视图
├── src-tauri/               # Tauri / Rust 桌面端源码
│   ├── src/                 # commands、进程、端口、配置、日志等模块
│   ├── capabilities/        # Tauri capability 配置
│   ├── icons/               # 应用图标资源
│   └── tauri.conf.json      # Tauri 应用配置
├── scripts/                 # 版本和发布辅助脚本
└── .github/workflows/       # GitHub Actions 发布流程
```

## 配置说明

应用配置会在 Tauri 应用数据目录中持久化，主要包含：

- `scan_dirs`：扫描目录列表
- `port_range_start` / `port_range_end`：自动端口范围
- `log_retention_days`：日志保留天数
- `theme` / `language`：主题与语言
- `minimize_to_tray`：关闭窗口时最小化到托盘
- `auto_restore`：启动时自动恢复项目
- `auto_check_updates`：启动时自动检查更新，由应用配置管理，不在设置页展示
- `update_endpoint` / `updater_pubkey`：更新源和 Tauri updater 公钥，由应用内置，不在设置页展示
- `ide_vscode_command` / `ide_antigravity_command`：IDE 命令或别名

项目配置包含名称、路径、启动命令、端口、分组、备注、依赖项目、环境变量、是否收藏、是否显示构建脚本等信息。

## 发布流程

准备新版本：

```bash
npm run release:prepare -- 0.1.12
```

确认变更后推送 tag：

```bash
git push origin v0.1.12
```

推送 `v*` tag 后，`.github/workflows/release.yml` 会在 macOS runner 上构建 Tauri 应用，并发布 GitHub Release。工作流会生成 updater JSON，因此发布前需要在仓库 Secrets 中配置：

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

## 开发提示

- 默认 Tauri 开发地址为 `http://localhost:1420`，由 `src-tauri/tauri.conf.json` 配置。
- 当前打包目标包含 macOS `app` 和 `dmg`。
- 如果需要修改自动更新地址或公钥，请同时关注默认配置和 Tauri 配置；公钥不在设置页展示。
- 运行进程由 Rust 侧统一管理，前端通过 `src/api/commands.ts` 调用 Tauri commands。

## License

当前仓库尚未声明许可证。如需开源或分发，请先补充明确的 License 文件。
