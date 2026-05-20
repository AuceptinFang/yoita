# yoita 

本项目致力于为游戏noita提供一个声明式的mod管理器
## quick_start 

```bash
cargo run -- yoita.toml
```

`[mods]` 现在按 cargo 风格工作：

```toml
[mods]
edit-always = {}
spell-lab = "1.0.0"
wanddbg = { workshop_id = "3454128340" }
custom-pack = { kind = "custom", url = "https://example.invalid/mod.zip" }
```

- 默认 source 是 `steam`
- 默认 version 是“最新”
- 只有名字和 steam 标识不一致时，才需要显式写 `workshop_id`

当前 `sync` 的行为是：

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
