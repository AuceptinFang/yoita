# yoita 

本项目致力于为游戏noita提供一个声明式的mod管理器
## quick_start 

1. 按参考文件修改配置：

   `examples/steam-workshop.toml`

2. 准备一个可用的创意工坊下载接口地址，填入 `[steam].download_endpoint`

3. 运行：

```bash
cargo run -- examples/steam-workshop.toml
```

MVP 当前会做三件事：

- 解析配置文件
- 创建 `cache_dir` / `staging_dir` / `mount_dir`
- 下载所有启用的 mod 到 `cache_dir`

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


## 设计


### mod源 
#### Steam
steam 的创意工坊是本项目的主要mod来源，提供第三方的下载接口，但是需要配置项

#### 自定义  
用户可以在配置文件里自己指定下载url 

### 交互 
声明通过 yoita.toml文件完成

 
