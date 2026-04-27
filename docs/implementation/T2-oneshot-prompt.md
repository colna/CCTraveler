# T2 · `-p / --prompt` 一次性模式

> 支持 `cctraveler -p "上海到北京最便宜高铁"` 一行出结果，或通过 stdin 管道传入。

## 完成状态
✅ 已实现并通过编译。

## 思路与逻辑

### 动机
脚本/CI/快速查询场景下，进入交互 REPL 是负担。需要一个"管道友好"的非交互入口：
- 拿到回答 → 打印到 stdout → 退出（exit code 反映成败）；
- 不打 banner、不打工具调用统计噪音。

### 设计要点
1. **输入来源优先级**：`-p "..."` > stdin（当无参数且非 TTY 时自动读）。
2. **复用 ConversationRuntime**：和 REPL 共用同一个 `run_turn`，但只跑一轮；不调用 `save_session`（一次性场景无需历史）。
3. **不启动 PriceScheduler**：背景调度只在交互模式有意义，避免一次性命令拉起后台 task 增加冷启动耗时。
4. **静默输出**：只 `println!("{}", summary.assistant_text)`，工具调用统计、token 使用都不打印（如需可加 `--verbose`）。

### 关键代码
```rust
let user_input = match prompt {
    Some(p) if !p.trim().is_empty() => p,
    _ => { let mut buf = String::new();
           std::io::stdin().read_to_string(&mut buf)?; buf }
};
let summary = rt.run_turn(user_input.trim())?;
println!("{}", summary.assistant_text);
```

## 改动文件
- `crates/cli/src/oneshot.rs`（新增）
- `crates/cli/src/main.rs` — 路由 `(None, Some(p)) → oneshot::run`

## 用法
```bash
cctraveler -p "5月1号上海到北京高铁，二等座500内"
echo "查上海4月底4星酒店" | cctraveler -p ""    # stdin
```

## 后续
- 待加 `--json` 结构化输出（v0.4）；
- 待加 `--verbose` 打印工具调用细节；
- 待支持流式 token 输出（依赖 `run_turn_streaming`，v0.4）。
