# 我的奇域

`my_wonderland` 查看当前账号的奇域作品、统计趋势和评价样本，采集结果按账号与角色保存在宿主机。插件管理米哈游账号、登录、资料与鉴权，Core 仅提供通用浏览器会话、HTTP／代理及运行环境。

## 使用与数据

启用插件后，在“我的奇域 → 账号管理”登录或管理米哈游账号。页面先读取本地记录，当天尚未采集时自动刷新，也可手动更新。账号切换会丢弃旧请求的响应，避免混入其他角色的数据。

数据位于 Core 数据根目录的 `plugin-data/my_wonderland/series-v2/`，更新或覆盖安装继续沿用。卸载时保留数据即可保留记录。图表只汇总有效值，评价区展示接口实际返回的样本。

## 界面

桌面与远程页面跟随 Core 的预设或背景采样主题，使用共享控件和边框；账号管理与趋势明细共用一个非模态侧栏，遗忘账号仍需模态确认。图表及数据表保留内部滚动，适配窄容器。

## 开发与打包

在 Core checkout 的 `plugins/my_wonderland` 目录运行：

```powershell
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
pnpm run build:release
```

Debug 目录包位于 `target/my-wonderland-plugin`，供 Debug Core 的开发目录入口安装。Release `.wplug` 位于 `target/my_wonderland-<version>-windows-x86_64.wplug`。通过插件管理覆盖安装即可测试；本地开发无需递增版本，也不要求先卸载。

线上成品使用 `pnpm run package:release`，由作者构建工具使用原身份完成包内签名。缺少原密钥时正式打包失败；本地候选可以未签名。分发管理插件只验签、上传和发布到 Cloudflare 渠道，不代管作者私钥。同一在线版本的内容固定，本地重建候选不覆盖已登记版本。

## 契约与兼容

- `package/manifest.json`：manifest v3、固定作者资料、兼容范围及 UI 入口。
- `package/contract.json`：账号快照、登录／取消／同步／默认账号／遗忘，以及 `series`、`collect`；账号管理方法 localOnly。
- `ui/src/main.tsx`：UI Bridge 适配与宿主主题跟随。
- `src/lib.rs`：采集与本地序列；`src/app.rs`：stdio 调度；`src/account/` 与 `src/mihoyo/`：账号存储及平台业务；`src/login.rs`：登录编排。

要求配套 Core `>=0.2.0 <0.3.0`。账号 Cookie 仅在本插件后端处理，使用通用 DPAPI 原语加密，普通 HTTP 负责传输；不回传 UI。远程共享须由宿主显式启用，插件在宿主机运行。详见 [Core 兼容说明](CORE-COMPATIBILITY.md)。

## 旧账号接续

首次运行从 `WONDERLAND_HOST_DATA_DIR/account` 导入元数据及原 DPAPI 密文，保持原始账号 key 和当前默认账号；`series-v2` 不改格式或路径。原目录保留，导入标记防止在遗忘账号或明确删除插件数据后自动恢复。迁移前备份不作为当前账号服务；需要回退时在插件停止后恢复匹配目录。密文仍绑定原 Windows 用户，不能仅复制到另一用户后保证有效。

新凭据和账号位于 `plugin-data/my_wonderland/account`，网页登录 profile 位于自身 `.browser-profiles/`。切换采集角色保存在当前访问设备偏好，不改变其他设备的选择；“设为默认”只调整未保存选择时的初始默认值。远程支持查看与采集，不允许登录、同步、遗忘或修改宿主默认账号。无桌面 CLI 复用已有账号，无法交互时明确失败。

插件使用 on_demand 启动策略：启用表示可用，首次打开页面或调用方法时才启动后端；后台实例在本次 Core 运行期间保留，不随标签关闭立即退出。普通 RPC 由 Core 跟踪，不额外持久化恢复标记或改变业务数据。
