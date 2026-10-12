> This is an independently versioned plugin repository. Local builds use the stable Core SDK from this checkout; see [Core plugin repository layout](../README.md).

# 我的奇域插件

`my_wonderland` 是独立动态插件，用于查看并采集当前账号下奇域作品数据。它由自己的清单、契约、React UI 和 Rust stdio 后端组成；Core 不静态导入插件页面或业务实现。

## 功能

- 按当前账号和角色查看已采集的作品序列、统计与详情。
- 通过奇域官方接口更新作品信息，并把结果保存在插件私有目录。
- 只注册一个 Workspace Activity：`main`。插件 UI 直接填充 Core 提供的内容区，并通过 UI Host SDK 跟随宿主主题。

## Core 能力与账号边界

插件使用默认可用的 `core.account.snapshot` 和 `core.account.authed_get`，manifest v3 不声明宿主能力申请。账号快照与鉴权 GET 请求由 Core 代理，保留现有非凭据摘要和官方请求范围；Core 不把 Cookie 或其他账号凭据返回给插件。

首次使用前，在 Core 登录一个账号并启用插件。账号切换由 Core 管理，插件每次操作读取当前快照；账号不可用时插件显示业务错误，Core 工作台仍可使用。

## 构建和安装

在仓库根目录运行：

```powershell
pnpm build
```

生成的 Windows x86_64 MSVC debug 包位于 `target/my-wonderland-plugin`。启动 debug Core，在“设置 → 插件管理”安装该目录并启用插件，然后从 Workspace 的“我的奇域”主入口打开。

## 发布并登记到在线目录

本仓库的 GitHub Release 提供 Core 可安装的 `.wplug` 文件。发布工作流在 Windows x86_64 runner 上检出固定版本的 Core SDK/UI 包，在插件目录执行锁定依赖安装、UI 类型检查和 Rust release 构建，然后生成带 `checksums.json` 的 ZIP 格式 `.wplug`。工作流只在推送与 `package/manifest.json` 版本一致的 `v<version>` 标签时创建 Release。

目录只登记插件身份和 Ed25519 公钥；插件版本由 Release 附带的 `<id>-update.json` 自行维护。发布工作流使用仓库 Actions secret `PLUGIN_UPDATE_SIGNING_KEY` 对版本、下载地址、包 SHA-256、兼容性及插件 manifest 投影（含可选 dependencies）签名。首次登记时运行 `node scripts/generate-update-signing-key.mjs`，将 `privateSeedBase64` 保存为仓库 Actions secret，并将 `publicKeyHex` 登记到目录。私钥不得提交到仓库或写入日志。轮换密钥时，先用新密钥重新签署当前版本清单并发布附件，再更新目录中的公钥；两步之间客户端会暂时无法验证更新信息。

在 Core checkout 的 `plugins/my_wonderland` 路径开发或打包，可以使用其相对路径 SDK/UI 依赖：

```powershell
pnpm install --frozen-lockfile
pnpm typecheck
pnpm build:release
```

本地 release 包位于 `target/release/my_wonderland-<version>-windows-x86_64.wplug`。发布时先让 `package/manifest.json`、根 `package.json`、`ui/package.json` 和 Rust workspace 版本保持一致并合并到 `main`，再推送对应标签，例如 `v0.1.0`。GitHub Actions 会将 `.wplug` 附加到 [本仓库的 Releases](https://github.com/YueFChen/my_wonderland/releases)。

首次登记时，向 [Wonderland 插件目录](https://github.com/YueFChen/Wonderland_Plugin_Catalog) 提交 PR，在 `catalog/v2/plugins/{id}.json` 登记插件身份、稳定的 latest update-manifest 地址和 Ed25519 公钥。后续版本不需要改目录；Release 工作流自动附加签名清单，Core 据此发现并校验当前版本，再校验 `.wplug` 下载包及其内部 `checksums.json`。目录 CI 在登记 PR 时验证签名清单、Release 包、包内校验和与插件元数据。

发布构建使用的 Core SDK/UI 基线固定在 `.github/workflows/release.yml`。升级宿主兼容范围或采用新的 SDK/UI 能力时，应先确认目标 Core 版本，再同步更新该固定版本和清单兼容范围。

## UI 开发

UI 源码由 Vite 编译到插件仓库内的 `ui/dist`，不会直接写入 Core 的用户数据或插件安装目录。修改后在插件仓库根目录运行 `pnpm build`，生成 `target/my-wonderland-plugin`，再通过 Core 的“设置 → 插件管理”安装。

Core 按插件版本管理安装目录。同一版本已经安装时，先在插件管理中移除旧插件再安装新包，或提升 `package/manifest.json` 的版本号；移除时保留插件数据即可。清单、契约和 Rust 后端改动也使用同一构建与安装流程。

## 数据

新数据位于 `%APPDATA%\com.wonderland.assistant\plugin-data\my_wonderland\series-v2\`。旧目录中的内容属于开发测试样本，不会导入；首次启用后插件会从空状态开始。移除插件包与清理插件数据是分开的操作。

## 目录与契约

- `package/manifest.json`：manifest v3 插件身份、兼容范围、唯一 Activity、UI 集成和可选插件依赖。
- `package/contract.json`：`account_snapshot`、`series`、`collect` 的参数、结果与超时契约。
- `ui/src/main.tsx`：UI Host SDK 与插件后端之间的适配；业务页面在同目录的 React 组件中。
- `src/lib.rs`：不依赖 Tauri 的采集与本地序列业务逻辑。
- `src/main.rs`：协议后端及 Core 账号服务适配。

日常开发先改 contract 与对应适配，再同步更新前端 DTO。插件包由仓库根目录的 `scripts/build-plugin.mjs` 生成；debug 包不包含发布签名。

## Core 0.1.8 与远程访问

见 [CORE-COMPATIBILITY.md](CORE-COMPATIBILITY.md)，包括最低版本、远程声明和发布顺序。
