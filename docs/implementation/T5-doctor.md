# T5 · `cctraveler doctor` 环境诊断

> 一键检查环境是否就绪：配置 / API key / SQLite / Scraper / Redis。

## 完成状态
✅ 已实现并实测通过：

```
$ cctraveler doctor
CCTraveler 环境诊断
───────────────────
✔ 配置加载成功
✔ API key 已配置 (长度 67)
  base_url: https://sub.sitin.ai
  model:    claude-opus-4-6
✔ SQLite 可写: data/cctraveler.db
⚠ Scraper 响应异常 (502 Bad Gateway): http://localhost:8300
  Redis 未启用 (可选)
───────────────────
✔ 核心检查通过
```

## 思路与逻辑

### 设计原则
1. **可在配置缺失时运行**：诊断本身要能告诉你"为什么没配置"，因此不能依赖配置加载成功。
2. **核心 vs 可选分级**：
   - 核心失败（无配置 / 无 key / db 不可写）→ 退出码非 0，CI 友好；
   - 可选警告（scraper 不通 / redis 未起）→ 用 `⚠` 标记，仍返回成功。
3. **每项独立**：一项失败不阻塞下一项，给出尽量完整的全景诊断。

### 检查项
| # | 检查 | 失败级别 | 标识 |
|---|---|---|---|
| 1 | 配置层级加载 | 致命（无法继续） | `✗` |
| 2 | API key（config 或 env） | 致命 | `✗` |
| 3 | SQLite 目录可写 + Database::open | 致命 | `✗` |
| 4 | Scraper `/healthz` | 警告（部分工具可降级） | `⚠` |
| 5 | Redis 状态 | 信息 | `·` |

### Scraper 健康探测
用 `reqwest::Client` 设 3s 超时，请求 `${base_url}/healthz`。区分三种状态：
- 200 → ✔
- 非 2xx → ⚠（标识具体 status，避免误以为完全断网）
- 网络错误 → ⚠（网络/DNS/拒连）

> 关键工程决策：故意 *不* 让 scraper 不通成为致命错误。Agent 即便没有 scraper 也能用 wiki/思考类工具，强行 fail 会阻碍调试。

### 退出码语义
- 全部 ✔ → exit 0
- 任一致命 ✗ → exit 1（`anyhow::bail!`）
- 仅 ⚠ → exit 0（用户已知问题）

## 改动文件
- `crates/cli/src/doctor.rs`（新增）
- `crates/cli/src/Cargo.toml` — 添加 `reqwest` 依赖
- `crates/cli/src/main.rs` — 路由 `Commands::Doctor → doctor::run()`

## 价值
1. 用户报错时第一反应可以先 `cctraveler doctor`，自助定位；
2. CI/容器健康检查可直接 `cctraveler doctor && start...`；
3. 也是后续 `cctraveler init` 完成后的"自我验证"出口。
