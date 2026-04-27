# T3 · 配置层级加载（user / project / cli）

> 支持类 `git` 的多源配置：用户全局、项目级、显式路径，按层级合并。

## 完成状态
✅ 已实现并验证（`doctor` 能从 `./config.toml` 正确加载）。

## 思路与逻辑

### 现状问题
原 `RuntimeConfig::load(path)` 只支持一个文件。要做到"`cctraveler` 在任何目录都能跑"，必须支持：
- 用户级默认配置（API key 这种敏感信息只放一份）；
- 项目级覆盖（不同项目用不同模型、不同 scraper 端点）；
- CLI 显式 `--config` 兜底。

### 搜索顺序（later wins）
```
1. ~/.cctraveler/config.toml          ← 用户全局
2. ./.cctraveler/config.toml          ← 项目级
3. ./config.toml | ./cctraveler.toml  ← 仓库开发兼容
4. --config <path>                    ← 最高优先级
```
任一存在即可；全部缺失时报错并提示 `cctraveler init`。

### TOML 递归 merge
TOML 的合并要按 **table 节点**深度递归，不能简单字符串拼接：
```rust
fn merge_toml(a: &mut toml::Value, b: toml::Value) {
    match (a, b) {
        (Table(at), Table(bt)) => for (k, v) in bt {
            match at.get_mut(&k) {
                Some(av) => merge_toml(av, v),
                None     => { at.insert(k, v); }
            }
        },
        (slot, val) => *slot = val,  // 标量直接覆盖
    }
}
```
合并后用 `try_into()` 反序列化成 `RuntimeConfig`，沿用 serde 默认值机制处理缺字段。

### 向后兼容
保留旧的 `RuntimeConfig::load(path)` 和 `load_default()`，只新增 `load_layered(Option<&Path>)`。CLI 走新路径，老代码不受影响。

### 暴露的辅助函数
- `runtime::config::user_config_path()` → `~/.cctraveler/config.toml`
- `runtime::config::project_config_path()` → `.cctraveler/config.toml`

供 `init`/`doctor` 复用。

## 改动文件
- `crates/runtime/src/config.rs` — 新增 `load_layered` + `merge_toml` + 路径辅助函数

## 验证
```
$ cctraveler doctor
✔ 配置加载成功         # 从 ./config.toml
✔ API key 已配置 (长度 67)
  base_url: https://sub.sitin.ai
  model:    claude-opus-4-6
```

## 错误兜底
缺所有源时输出：
```
未找到配置文件。
请运行 `cctraveler init` 初始化配置，或在以下任一位置创建 config.toml：
  - /Users/.../.cctraveler/config.toml
  - ./.cctraveler/config.toml
  - ./config.toml
```
让用户立刻知道下一步该做什么。
