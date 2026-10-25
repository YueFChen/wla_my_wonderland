# 我的奇域

`wla_my_wonderland` 管理国服米哈游账号与国际服 HoYoverse 账号，查看奇域作品、统计趋势和评价样本。两服共用采集、序列和图表，凭据与数据按来源和角色分区；Core 提供通用浏览器会话、HTTP／代理及运行环境。

源码仓库和本地目录统一为 `wla_my_wonderland`，开发版本沿用 `0.1.4`。从旧 `my_wonderland` 接续时，账号重新登录即可，采集历史可按下方方式导入；旧插件数据不会被自动移动或删除。

## 历史导入与导出

页面顶部可导出全部 `series-v2` 采集历史，或选择 JSON 文件合并导入。备份不包含账号记录、Cookie、浏览器 profile 或登录密钥；重新登录原账号后即可读取相同账号／角色的历史。

导入保留历史点位和已归档作品，同一日期采用较新采集记录的值；重复导入不会增加采集次数。文件中任一角色范围或数据格式无效时，在写入前拒绝整份文件。采集期间会明确提示操作占用。

旧插件没有导出按钮时，可以直接选择旧 `series-v2/<account>/<region>/<uid>.json`。也可批量生成同格式备份；此命令只读取指定旧目录的 `series-v2`，不会读取账号登录资料，且不覆盖已有输出文件：

```powershell
node scripts/export-history.mjs --data-dir 'D:\Backup\my_wonderland' --output 'D:\Backup\my-wonderland-history.json'
```

远程访问时选择访问设备的文件，导出由 Core 交付下载。备份包含作品统计与角色标识，应按自己的业务资料保存。

## 使用与数据

启用插件后，在“我的奇域 → 账号管理”选择国服或国际服并登录。页面先读取本地记录，再按角色的官方 UTC+8 数据刷新时间判断是否采集，也可手动更新；旧资料尚无刷新配置时沿用原本地日期规则。重新激活保留的标签会复查采集周期，切换角色会丢弃旧请求响应。

数据位于 Core 数据根目录的 `plugin-data/wla_my_wonderland/series-v2/`，更新或覆盖安装继续沿用。卸载时保留数据即可保留记录。图表只汇总有效值，评价区展示接口实际返回的样本。

## 国服与国际服

- 添加账号时使用所选平台的官方页面和独立浏览器 profile；凭据先通过对应奇匠中心验证，再加密保存，Cookie 不返回前端。
- 使用配套 Core 的会话弹窗完成第三方登录跳转。Google 等身份提供方可能按其政策拒绝内嵌浏览器；弹窗支持不代表已通过真实授权验证，也不会复用系统浏览器现有登录。
- 国服沿用原账号 key、密文与历史路径；国际服使用 `global-<官方账号ID>` 作为本地 key，同数字 ID 可以共存。界面展示官方 ID，不展示内部前缀。
- 国际服通过奇匠中心读取已入驻角色，支持美服、欧服、亚服、台港澳服；未入驻角色须先在官方中心办理，插件不自动替用户入驻。
- 角色的创作者资料与刷新时间分别保存。采集显式指定 UID 和区服，不调用官网的角色切换或其他写入业务接口。
- 两服共用总览、作品分页、详情、趋势和评价模型。国际服数据请求使用 `hk4e_global` 与 `sg-api-hoyocreators.hoyoverse.com`；国服配置保持 `hk4e_cn`。

## 界面

桌面与远程页面跟随 Core 的预设或背景采样主题，使用共享控件和边框；账号管理与趋势明细共用一个非模态侧栏，遗忘账号仍需模态确认。图表及数据表保留内部滚动，适配窄容器。

## 开发与打包

在 Core checkout 的 `plugins/wla_my_wonderland` 目录运行：

```powershell
pnpm install --frozen-lockfile
pnpm run check
pnpm run check:rust
pnpm run check:bindings
pnpm run test:backend
pnpm run build
pnpm run build:release
```

Debug 目录包位于 `target/wla-my-wonderland-plugin`，供 Debug Core 的开发目录入口安装。Release `.wplug` 位于 `target/wla_my_wonderland-<version>-windows-x86_64.wplug`。通过插件管理覆盖安装即可测试；本地开发无需递增版本，也不要求先卸载。

线上成品使用 `pnpm run package:release`，由作者构建工具使用原身份完成包内签名。缺少原密钥时正式打包失败；本地候选可以未签名。分发管理插件只验签、上传和发布到 Cloudflare 渠道，不代管作者私钥。同一在线版本的内容固定，本地重建候选不覆盖已登记版本。

新 ID 经作者明确确认建立独立签名身份，公钥为 `0ba0e72677dd45cc23c762238de2c3fc6ef8a9633885d028fd5b9ae89a647d3a`；旧 ID 的密钥和 GitHub Secret 保持不变。本机使用 Windows DPAPI 作者密钥库。签名工作流只供配置好的私有构建仓库使用，要求新 ID 专用的 `WLA_MY_WONDERLAND_SIGNING_KEY` Secret 与 `WLA_MY_WONDERLAND_SIGNING_PUBLIC_KEY` 公钥变量；公开源码仓库不会运行签名步骤，也不会复用旧 ID 的 Secret。

## 契约与兼容

- `package/manifest.json`：manifest v3、固定作者资料、兼容范围及 UI 入口。
- `package/contract.json`：账号快照、登录／取消／同步／默认账号／遗忘，以及 `series`、`collect`、`import_history`、`export_history`；账号管理方法 localOnly。`account_login` 可传 `platform: "cn" | "global"`，省略兼容国服。
- `ui/src/main.tsx`：UI Bridge 适配与宿主主题跟随。
- `src/lib.rs`：采集与本地序列；`src/platform.rs`：两服配置；`src/app.rs`：stdio 调度；`src/account/` 与 `src/mihoyo/`：账号存储及平台业务；`src/login.rs`：登录编排。

要求配套 Core `>=0.2.0 <0.3.0`。账号 Cookie 仅在本插件后端处理，使用通用 DPAPI 原语加密，普通 HTTP 负责传输；不回传 UI。远程共享须由宿主显式启用，插件在宿主机运行。详见 [Core 兼容说明](CORE-COMPATIBILITY.md)。

## 旧账号接续

首次运行从 `WONDERLAND_HOST_DATA_DIR/account` 导入元数据及原 DPAPI 密文，保持原始账号 key 和当前默认账号；`series-v2` 不改格式或路径。原目录保留，导入标记防止在遗忘账号或明确删除插件数据后自动恢复。迁移前备份不作为当前账号服务；需要回退时在插件停止后恢复匹配目录。密文仍绑定原 Windows 用户，不能仅复制到另一用户后保证有效。

新凭据和账号位于 `plugin-data/wla_my_wonderland/account`，网页登录 profile 位于自身 `.browser-profiles/`。切换采集角色保存在当前访问设备偏好，不改变其他设备的选择；“设为默认”只调整未保存选择时的初始默认值。远程支持查看与采集，不允许登录、同步、遗忘或修改宿主默认账号。无桌面 CLI 复用已有账号，无法交互时明确失败。

插件使用 on_demand 启动策略：启用表示可用，首次打开页面或调用方法时才启动后端；后台实例在本次 Core 运行期间保留，不随标签关闭立即退出。普通 RPC 由 Core 跟踪，不额外持久化恢复标记或改变业务数据。
