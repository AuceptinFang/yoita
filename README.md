# yoita 

本项目致力于为游戏noita提供一个声明式的mod管理器
## quick_start 

```bash
cargo run -- yoita.toml
```

一个最小可用的 `yoita.toml`：

```toml
[steam]
[mods]
edit-always = {}  # 支持mod名搜索
spell-lab = "2297568811"  # 事实上workshop_id才是真正的表示符
wanddbg = { id = "2572385079" } # Workshop_id 这么写也可以
custom-pack = { kind = "custom", url = "https://example.invalid/mod.zip" }  # 也可以自己提供下载url
```

- 默认 source 是 `steam`
- 字符串短写等价于 `{ id = "..." }`
- 空 inline table 会把 mod 名当作默认 id
- 显式指定 Steam 工坊项时，优先写 `id`

## yoita.toml 当前支持

- `[config]` 支持 `cache_dir`、`staging_dir`、`mount_dir`
- `[steam]` 支持 `steamcmd` 后端，以及 `steamcmd_path`、`force_install_dir`、`app_id`、`timeout_secs`
- `[steam]` 支持 `login = "anonymous"` 和 `login = "account"`；账号登录时配合 `username`、`password_env`
- `[mods]` 支持 Steam mod 的三种写法：`name = "1234"`、`name = { id = "1234" }`、`name = {}`
- `[mods]` 支持 `enabled = false` 禁用单个 mod
- `[mods]` 支持 `kind = "custom"` 加 `url = "..."` 的自定义下载源

当前 `sync` 的行为：

1. `yoita.toml` 只描述期望的 mod 集合
2. `.yoita/state.toml` 记录上一次同步到工作区的结果
3. `sync` 会：
   - 下载或定位每个启用 mod 的本地源路径
   - 同步到 `mount_dir`
   - 清理那些已经不在配置里、但曾经由 yoita 写入 `mount_dir` 的结果

当前不会做激进清理：

- 不删除 Steam 默认目录里的缓存
- 不退订 Workshop item
- 不删除不属于 yoita 状态文件记录的 `mount_dir` 内容

## docs

- SteamCMD backend 接口文档：`docs/steamcmd-backend.md`

## yoita.toml todo

- 支持本地目录或本地压缩包作为 mod source
- 继续收敛 `yoita.toml` 写法，只保留一套推荐格式
