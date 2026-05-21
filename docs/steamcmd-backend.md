# SteamCMD Backend 接口文档

本文档定义 `yoita` 在使用 `steamcmd` 作为 Steam Workshop 下载后端时，需要依赖的外部命令接口、内部 Rust 接口、配置项和结果约定。

适用范围：

- Steam 源 mod 的下载
- Steam Workshop 内容的本地定位
- `steamcmd` 输出结果的解析
- 下载结果向 `staging` 和 `mount_dir` 的传递

本文档不覆盖：

- Noita mod 解包和目录归一化
- `custom` 源下载
- GUI 交互

## 1. 目标

`yoita` 需要做到：

1. 给定 Noita `app_id` 和 Workshop `published_file_id`
2. 若本地不存在，则主动调用 `steamcmd` 下载
3. 定位下载结果所在路径
4. 将结果交给后续 `staging -> mount_dir` 流程

当前约束：

- Noita 的 Steam `app_id` 为 `881100`
- Workshop item 的下载结果不保证都是目录
- 不能仅依赖 `steamcmd` 进程退出码判断成功

## 2. 外部接口

`yoita` 需要直接依赖的 `steamcmd` 命令只有以下几个。

### 2.1 `force_install_dir <path>`

作用：

- 设置 SteamCMD 的下载根目录

要求：

- 该命令应在 `login` 之前执行
- `path` 由 `yoita` 控制，建议落到工作区，例如 `.yoita/steamcmd`

下载完成后，Workshop 内容将位于：

```text
<force_install_dir>/steamapps/workshop/content/<app_id>/<published_file_id>/
```

### 2.2 `login anonymous`

作用：

- 匿名登录 SteamCMD

使用策略：

- 作为默认登录方式
- 对公开的 Noita Workshop item 优先尝试该方式

### 2.3 `login <username> <password>`

作用：

- 使用 Steam 账号登录

使用策略：

- 仅在匿名登录失败，且用户明确提供凭据时使用
- 不建议将密码明文写入 `yoita.toml`
- 更适合从环境变量或外部凭据源读取

### 2.4 `workshop_download_item <appid> <PublishedFileId>`

作用：

- 下载指定 Workshop item

当前固定参数：

- `appid = 881100`
- `PublishedFileId` 来自 mod 配置中的 `id`（兼容 `workshop_id`）

示例：

```text
workshop_download_item 881100 2194781427
```

### 2.5 `quit`

作用：

- 结束 SteamCMD 会话

## 3. 自动化方式

`yoita` 应支持两种调用形式，但实现时优先采用脚本模式。

### 3.1 命令行模式

```bash
steamcmd \
  +force_install_dir /abs/path/.yoita/steamcmd \
  +login anonymous \
  +workshop_download_item 881100 2194781427 \
  +quit
```

适用场景：

- 调试
- 手工复现问题

### 3.2 脚本模式

建议由 `yoita` 写入临时脚本，再通过 `+runscript` 执行。

脚本示例：

```text
@ShutdownOnFailedCommand 1
@NoPromptForPassword 1
force_install_dir /abs/path/.yoita/steamcmd
login anonymous
workshop_download_item 881100 2194781427
quit
```

执行方式：

```bash
steamcmd +runscript /abs/path/.yoita/steamcmd/download.txt
```

优点：

- 避免命令行参数转义问题
- 输出更稳定
- 后续切换账号登录时更容易扩展

## 4. 脚本控制项

### 4.1 `@ShutdownOnFailedCommand 1`

作用：

- 脚本内任意命令失败后立即停止

要求：

- `yoita` 生成的脚本默认写入该项

### 4.2 `@NoPromptForPassword 1`

作用：

- 禁止 SteamCMD 在密码缺失时进入交互式等待

要求：

- `yoita` 生成的脚本默认写入该项

## 5. 下载结果约定

`steamcmd` 下载成功后，`yoita` 需要返回一个统一结构，而不是直接把路径字符串散落在业务代码里。

建议的内部结果类型：

```rust
pub struct SteamWorkshopItem {
    pub app_id: u32,
    pub workshop_id: String,
    pub content_root: PathBuf,
    pub content_kind: SteamContentKind,
}

pub enum SteamContentKind {
    Directory,
    SingleFile,
}
```

约定：

- `content_root` 指向实际可消费的本地路径
- 若下载目录中存在多个文件，则 `content_root` 为该 item 目录
- 若下载目录中只有一个文件，则 `content_root` 为该文件路径
- `content_kind` 用于告诉后续流程当前结果是目录还是单文件

原因：

- Noita Workshop item 实际上并不保证都是 mod 目录
- 至少需要把“目录”和“单文件”分开，否则后续 `mount` 行为无法稳定定义

## 6. 内部 Rust 接口

`yoita` 内部建议将 SteamCMD 后端封装为单独类型。

当前代码中已经落地的基础抽象按职责拆在 `src/steam/` 下：

- 领域标识类型：
  - [types.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/types.rs:1)
  - `SteamAppId`
  - `WorkshopItemId`
  - `WorkshopItemRef`
- 领域结果类型：
  - `WorkshopItemDetails`
  - `WorkshopItemContent`
  - `WorkshopContentKind`
- 传输层抽象：
  - [transport.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/transport.rs:1)
  - `HttpRequester`
  - `CommandRunner`
- 语义层抽象：
  - [provider.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/provider.rs:1)
  - `WorkshopMetadataProvider`
  - `WorkshopContentProvider`
- 聚合入口：
  - [service.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/service.rs:1)
  - `SteamServices`
- SteamCMD 运行时配置：
  - [steamcmd.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/steamcmd.rs:1)
  - `SteamCmdConfig`
  - `SteamCmdScript`
- 内容类型判定：
  - [content.rs](/home/auceptin_fang/programs/rust/yoita/src/steam/content.rs:1)
  - `content_kind_for_path`

这里的设计意图是：

- 内部逻辑只依赖 `dyn WorkshopMetadataProvider` / `dyn WorkshopContentProvider`
- 真正的 HTTP 请求细节下沉到 `dyn HttpRequester`
- 真正的进程执行细节下沉到 `dyn CommandRunner`

这样做的好处是，后续即使从 `steamcmd` 切换到 `steamworks-rs`，或同时支持两者，也不需要改业务层同步流程。

### 6.1 配置接口

```rust
pub struct SteamCmdConfig {
    pub steamcmd_path: PathBuf,
    pub force_install_dir: PathBuf,
    pub app_id: SteamAppId,
    pub login: SteamLoginMode,
    pub timeout: Duration,
}

pub enum SteamLoginMode {
    Anonymous,
    Account {
        username: String,
        password_env: String,
    },
}
```

约定：

- `steamcmd_path` 为可执行文件路径
- `force_install_dir` 为 SteamCMD 工作根目录
- `app_id` 默认为 `881100`
- `timeout` 用于限制外部进程挂起
- 账号密码建议通过环境变量提供，而不是直接存配置文件

### 6.2 主接口

当前代码中还没有把 `SteamCmdClient` 真正实现出来；目前已稳定的边界是：

- `dyn CommandRunner`
- `dyn WorkshopContentProvider`

后续若需要把 SteamCMD 后端收束成单独类型，推荐外形如下：

```rust
pub struct SteamCmdClient {
    config: SteamCmdConfig,
    runner: Arc<dyn CommandRunner>,
}

impl SteamCmdClient {
    pub fn new(config: SteamCmdConfig) -> Result<Self>;

    pub fn workshop_item_path(&self, workshop_id: WorkshopItemId) -> PathBuf;

    pub fn is_workshop_item_present(&self, workshop_id: WorkshopItemId) -> bool;

    pub fn build_script(&self, workshop_id: WorkshopItemId) -> SteamCmdScript;

    pub async fn ensure_downloaded(
        &self,
        request: WorkshopContentRequest,
    ) -> Result<WorkshopItemContent>;

    pub async fn run_download(
        &self,
        workshop_id: WorkshopItemId,
    ) -> Result<SteamCmdRunResult>;
}
```

职责：

- `workshop_item_path`
  - 计算目标目录路径
- `is_workshop_item_present`
  - 本地检查是否已存在可用下载结果
- `build_script`
  - 生成本次下载用的脚本文本
- `run_download`
  - 执行 SteamCMD 并采集 stdout/stderr
- `ensure_downloaded`
  - 对外主入口
  - 若本地已有结果则直接返回
  - 若本地不存在则触发下载并返回统一结果

### 6.3 进程结果接口

```rust
pub struct SteamCmdRunResult {
    pub exit_status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
```

用途：

- 记录原始输出
- 在错误信息中向上游报告真实上下文

## 7. 成功与失败判定

不能只看 exit code。

原因：

- `steamcmd` 在某些失败场景下仍可能以 `0` 退出
- 例如无效 Workshop item 可能打印 `ERROR! Download item ... failed (No match)`，但进程仍成功结束

建议的成功判定顺序：

1. 进程未超时
2. 输出中包含成功标记
3. 目标路径存在
4. 目标路径非空

建议匹配的成功标记：

- `Success. Downloaded item`

建议匹配的失败标记：

- `ERROR!`
- `failed (No match)`
- `No subscription`

建议实现：

```rust
pub enum SteamCmdDownloadStatus {
    Downloaded,
    AlreadyPresent,
    NotFound,
    NoSubscription,
    TimedOut,
    CommandFailed,
}
```

## 8. 推荐下载流程

`ensure_downloaded(workshop_id)` 的建议流程：

1. 校验 `workshop_id` 非空
2. 计算 `<force_install_dir>/steamapps/workshop/content/881100/<id>`
3. 若本地已存在可用结果，则直接返回
4. 生成 SteamCMD 脚本
5. 调用 `steamcmd +runscript ...`
6. 捕获 stdout/stderr
7. 解析成功或失败状态
8. 重新扫描目标目录
9. 将结果归一化为 `SteamWorkshopItem`

## 9. 配置文件建议

建议后续在 `yoita.toml` 中将 SteamCMD 配置补成这样：

```toml
[steam]
backend = "steamcmd"
steamcmd_path = "/usr/bin/steamcmd"
force_install_dir = ".yoita/steamcmd"
app_id = 881100
timeout_secs = 300
login = "anonymous"
```

若需要账号登录：

```toml
[steam]
backend = "steamcmd"
steamcmd_path = "/usr/bin/steamcmd"
force_install_dir = ".yoita/steamcmd"
app_id = 881100
timeout_secs = 300
login = "account"
username = "your_steam_name"
password_env = "YOITA_STEAM_PASSWORD"
```

对应的 mod 配置保持不变：

```toml
[mods]
wanddbg = { id = "2572385079" }
```

## 10. 已知限制

- `steamcmd` 首次启动会自更新，冷启动开销较大
- Linux 下需要 32 位运行库
- Valve 文档建议不要以 `root` 运行
- 使用账号登录时，Steam 图形客户端与 SteamCMD 可能发生登录互斥
- 下载结果不保证是可直接挂载的 Noita mod 目录

## 11. 参考资料

- SteamCMD:
  - https://developer.valvesoftware.com/wiki/SteamCMD
- Steam Workshop Implementation Guide:
  - https://partner.steamgames.com/doc/features/workshop/implementation
- ISteamRemoteStorage Web API:
  - https://partner.steamgames.com/doc/webapi/ISteamRemoteStorage
- IPublishedFileService:
  - https://partner.steamgames.com/doc/webapi/IPublishedFileService

## 12. 当前结论

对 `yoita` 来说，`steamcmd` 后端的最小实现范围应当是：

1. 生成脚本
2. 调用 SteamCMD
3. 判断下载结果
4. 返回统一的本地路径结果

这已经足够支撑：

- `steam` source 的主动下载
- `state.toml` 中记录本地来源路径
- 后续 `staging` 与 `mount_dir` 的接入
