# yoita 

本项目致力于为游戏noita提供一个声明式的mod管理器
## quick_start 

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

