# T11 · `install.sh` + Release CI

> 一行安装 + tag 即发布的完整闭环：用户敲 `curl ... | sh` 拿到二进制，开发者推 `vX.Y.Z` tag 自动构建 4 平台。

## 完成状态
✅ `scripts/install.sh` 写好（detect_target 单测通过）；`.github/workflows/release.yml` 矩阵构建 + 自动发布配置完成。需要在仓库创建一次 tag 才能首次跑通。

## 思路与逻辑

### 安装脚本设计
**目标**：让首次用户的体验是
```bash
curl -fsSL https://raw.githubusercontent.com/colna/CCTraveler/main/scripts/install.sh | sh
```
之后即可 `cctraveler init` → `cctraveler`。

**关键决策**：
1. **POSIX-ish bash**：`set -euo pipefail` 防止悄悄继续；不依赖任何非内置工具（除 curl/tar/install/uname）。
2. **平台探测显式列举**：不写"假设 linux x86_64"这种暗坑，命中四种支持组合，否则明确报错并指向 `cargo install`。
3. **可覆盖**：`CCTRAVELER_VERSION=v0.4.0`、`CCTRAVELER_INSTALL_DIR=...` 让 CI/容器部署能精准锁版与改路径。
4. **资产命名约定**：`cctraveler-${TAG}-${TARGET}.tar.gz`，与 release.yml 的 packaging 步骤一一对应。
5. **PATH 检测**：安装到 `~/.local/bin`（XDG 友好）后，若不在 `$PATH` 中则打印一行 `export PATH=...` 提示，**不**自动改 shell 配置——避免污染用户 dotfiles。
6. **失败友好**：`mktemp -d` + `trap 'rm -rf' EXIT` 保证半路失败不留垃圾。

### Release workflow 设计
**触发**：`push: tags: ['v*']` + `workflow_dispatch`（手动调试）。

**支持目标**：
| target | runner | 构建方式 |
|---|---|---|
| `x86_64-unknown-linux-gnu` | ubuntu-latest | cargo |
| `aarch64-unknown-linux-gnu` | ubuntu-latest | cross |
| `x86_64-apple-darwin` | macos-13（intel runner） | cargo |
| `aarch64-apple-darwin` | macos-latest | cargo |

**关键决策**：
- **macOS 双架构走双 runner** 而不是 lipo 合并：现在 GitHub Actions 提供 m-series runner 直接出 native arm64，且 intel runner 也长期可用，二者并行 < 4 分钟，比 lipo 后处理简单。
- **Linux aarch64 用 cross**：避免在 ubuntu runner 上配 sysroot；`cross install` 出的 docker 容器开箱即用。
- **Swatinem/rust-cache**：`key: ${{ matrix.target }}` 让每个目标各自缓存；4 个并行 job 互不踩。
- **校验和**：每个 tarball 同步生成 `.sha256`，install 阶段（v0.4.x 后续）可加校验提示。
- **发布权限**：`permissions: contents: write`（只授必要权限，不开 actions:write 等）。

### 与设计文档的对应
对应原 `docs/cctraveler-cli-design.md` 第 3 节安装分发的 P0：
- ✔ 一行安装脚本
- ✔ GitHub Actions 矩阵构建
- ✔ release tag 触发上传
- ⏳ Homebrew tap（v0.4.x，需另开 tap repo）
- ⏳ npm 包装（v0.5）
- ⏳ crates.io 发布（v0.4.0 跟 tag 一并）

## 改动文件
- `scripts/install.sh`（新增，已 chmod +x）
- `.github/workflows/release.yml`（新增）

## 验证
```bash
$ bash -c '... detect_target'   # 仅函数
aarch64-apple-darwin            # 在本机 m-series 上正确
```
完整端到端验证需在仓库推一个 tag（例如 `git tag v0.4.0-rc1 && git push --tags`）触发 workflow，等 4 个 job 完成后访问 release page。

## 后续
- 把 `install.sh` 同时镜像到一个短域名（如 `cctraveler.dev/install.sh`）避免 raw.githubusercontent CDN 节点不稳；
- `install.sh` 增加 sha256 校验、Tor / 代理友好开关；
- Homebrew tap：`brew install colna/tap/cctraveler`，release.yml 末尾追加 `brew bump-formula-pr` 步骤；
- 同步发布到 crates.io：在 release job 里加 `cargo publish` 步骤（依赖 `CARGO_REGISTRY_TOKEN` secret）。
