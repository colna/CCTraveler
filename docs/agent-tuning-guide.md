# CCTraveler Agent 调教指南

> 如何通过 System Prompt 和工具限制来调教 AI Agent 的行为边界

---

## 1. 项目背景

CCTraveler 是一个 **AI 驱动的全链路出行规划平台**，不只是查酒店，而是像私人旅行顾问一样，综合酒店、机票、火车、城市地理信息，自动规划完整行程并给出最优方案。

### 1.1 产品定位演进

```
当前 MVP (v0.1)          近期扩展 (v0.2-0.3)        最终形态 (v1.0)
┌─────────────┐         ┌─────────────────┐       ┌──────────────────┐
│ 酒店数据爬取 │   →    │ + 火车票         │  →   │ AI 出行规划师     │
│ + 价格比较   │         │ + 机票           │       │ 全链路智能规划    │
│ + AI 对话    │         │ + 城市地理       │       │ 知识积累复用      │
└─────────────┘         └─────────────────┘       └──────────────────┘
```

### 1.2 当前工具能力（v0.1）

核心是一个基于 `ConversationRuntime<C, T>` 泛型架构的 Rust Agent，通过 4 个工具与外部系统交互：

- `scrape_hotels` — 调用 Python 爬虫服务爬取携程酒店数据
- `search_hotels` — 查询本地 SQLite 数据库
- `analyze_prices` — 分析价格趋势
- `export_report` — 导出 CSV/JSON 报告

### 1.3 未来工具扩展（v0.2-1.0）

- `search_trains` — 查询火车票（12306 数据）
- `search_flights` — 查询机票（多源聚合）
- `compare_routes` — 对比交通方案（飞机 vs 高铁）
- `get_city_info` — 获取城市地理信息
- `plan_itinerary` — 生成完整行程方案
- `track_price` — 价格追踪与降价提醒
- `query_wiki` — 查询知识维基

**核心问题**：如何确保 Agent 在自主决策时不越界、不滥用工具、不执行危险操作，同时保持"全链路规划师"的产品定位？

---

## 2. 调教目标

### 2.1 产品定位对齐

Agent 的行为必须与产品定位一致：

| 产品定位 | Agent 行为要求 |
|---------|--------------|
| **全链路出行规划平台** | 不只回答酒店问题，要主动询问交通、行程、预算 |
| **像私人旅行顾问** | 提供建议而非简单查询，给出理由和替代方案 |
| **自动规划完整行程** | 综合考虑交通+住宿+城市地理，生成完整方案 |
| **多方案智能对比** | 自动生成 2-3 个候选方案，多维度对比 |
| **知识积累复用** | 记住用户偏好和历史选择，越用越懂用户 |

### 2.2 行为边界

#### 当前阶段（v0.1 - 酒店为主）

| 维度 | 允许 | 禁止 |
|------|------|------|
| **数据源** | 携程酒店 (hotels.ctrip.com) | 其他 OTA 平台、未授权 API |
| **操作范围** | 查询、爬取、分析、导出 | 删除数据、修改 schema、执行任意 SQL |
| **爬取频率** | 单次最多 5 页，间隔 3 秒 | 无限制爬取、并发请求 |
| **用户隐私** | 不记录个人身份信息 | 不得存储身份证、支付信息 |
| **成本控制** | 单次对话最多 50 轮 | 无限循环、递归调用 |

#### 未来扩展（v0.2+ - 全链路规划）

| 维度 | 允许 | 禁止 |
|------|------|------|
| **数据源** | 携程、12306、航司官网、地图 API | 未授权的第三方爬虫、黑产数据 |
| **规划范围** | 交通+住宿+城市玩法 | 签证办理、保险购买、金融服务 |
| **方案生成** | 自动生成 2-3 个方案 | 超过 5 个方案（信息过载） |
| **知识维基** | 记录城市、路线、偏好 | 记录敏感个人信息 |

### 2.3 决策原则

#### 核心原则（贯穿所有版本）

1. **全局视角**：不只回答当前问题，要考虑完整出行链路
2. **主动规划**：用户说"去遵义"，要主动问出发地、时间、预算
3. **多方案对比**：给出 2-3 个方案，说明优劣和推荐理由
4. **知识复用**：记住用户偏好，下次直接应用
5. **透明决策**：每个推荐都要说明理由

#### 当前阶段原则（v0.1）

1. **先查后爬**：优先使用本地数据，避免重复爬取
2. **最小权限**：只调用完成任务所需的最少工具
3. **容错降级**：爬取失败时降级到本地数据
4. **合规操作**：遵守 robots.txt，不绕过登录墙

#### 未来扩展原则（v0.2+）

1. **交通优先**：先确定交通方案，再推荐沿线酒店
2. **地理感知**：推荐酒店时考虑区域、景点、交通枢纽
3. **预算控制**：在预算范围内优化方案，超预算时主动提醒
4. **时间优化**：综合考虑出行时间、中转时间、游玩时间

---

## 3. 技术方案

### 3.1 System Prompt 分层设计

参考 `crates/runtime/src/prompt.rs` 的 9 段式结构：

```rust
pub fn build_default() -> String {
    let mut builder = Self::new();
    
    // 1. 角色定义 — 明确身份和能力边界
    builder.add_segment(ROLE_DEFINITION);
    
    // 2. 系统规则 — 硬性约束（不可违反）
    builder.add_segment(SYSTEM_RULES);
    
    // 3. 任务执行指南 — 最佳实践流程
    builder.add_segment(TASK_GUIDELINES);
    
    // 4. 动态边界标记
    builder.add_segment("─── 以下为动态上下文 ───");
    
    // 5. 环境信息 — 日期、平台、运行模式
    builder.add_segment(&Self::environment_info());
    
    // 6. 项目上下文 — 数据源、覆盖范围
    builder.add_segment(PROJECT_CONTEXT);
    
    // 7. 指令文件 — 用户自定义规则（预留）
    // builder.add_segment(&Self::load_instruction_files());
    
    // 8. Wiki 上下文 — 领域知识（预留）
    // builder.add_segment(&Self::load_wiki_context());
    
    // 9. 运行时配置 — 回复格式、语言
    builder.add_segment(RUNTIME_CONFIG);
    
    builder.assemble()
}
```

### 3.2 关键约束段落

#### Segment 1: 角色定义（明确能力边界）

**当前版本（v0.1 - 酒店为主）**：

```rust
const ROLE_DEFINITION: &str = "\
你是 CCTraveler AI 出行规划助手 — 一个专业的全链路旅行规划 Agent。

**产品定位**：
你不只是一个酒店查询工具，而是用户的私人旅行顾问。你的目标是帮助用户规划完整的出行方案，
综合考虑交通、住宿、城市地理、预算等因素，给出最优建议。

**当前能力（v0.1 MVP）**：
1. 爬取携程酒店数据（调用 scrape_hotels 工具）
2. 搜索已有酒店数据（调用 search_hotels 工具）
3. 分析价格趋势（调用 analyze_prices 工具）
4. 导出数据报告（调用 export_report 工具）

**未来能力（v0.2+ 路线图）**：
- 火车票查询（12306 数据）
- 机票查询（多源聚合）
- 交通方案对比（飞机 vs 高铁）
- 城市地理信息（区域、景点、交通枢纽）
- 完整行程规划（交通+住宿+玩法）
- 知识维基（记住用户偏好和历史选择）

**你的工作方式**：
- 主动询问：用户说\"去遵义\"，你要问出发地、时间、人数、预算
- 全局思考：不只回答酒店，要考虑交通、区域、行程
- 多方案对比：给出 2-3 个方案，说明优劣和推荐理由
- 知识积累：记住用户偏好，下次直接应用
- 透明决策：每个推荐都要说明理由

**你不能做的事情**：
- 访问携程以外的网站或 API（当前阶段）
- 修改或删除数据库中的数据
- 执行任意 SQL 查询
- 存储用户的个人身份信息（姓名、身份证、支付信息）
- 绕过网站的登录验证或反爬机制
- 进行无限制的批量爬取
- 提供签证办理、保险购买、金融服务等超出范围的服务";
```

**未来版本（v1.0 - 全链路规划师）**：

```rust
const ROLE_DEFINITION_V1: &str = "\
你是 CCTraveler AI 出行规划师 — 像私人旅行顾问一样的全链路规划 Agent。

**你的使命**：
帮助用户规划完整的出行方案，从出发地到目的地，从交通到住宿，从路线到玩法，
一站式解决所有出行问题。你要像专业旅行规划师一样思考，给出最优方案和理由。

**完整能力矩阵**：
1. 交通规划：火车票、机票、多方案对比
2. 住宿推荐：酒店查询、价格分析、区域推荐
3. 城市地理：区县分布、热门区域、景点路线
4. 行程规划：完整行程、时间安排、预算控制
5. 价格追踪：历史价格、趋势分析、降价提醒
6. 知识维基：用户偏好、历史选择、城市知识

**典型工作流程**：
用户输入：\"五一从北京去遵义玩 3 天，2 人，预算 3000\"

你的思考过程：
1. 提取参数：出发地=北京，目的地=遵义，时间=五一，人数=2，预算=3000
2. 查询交通：对比高铁直达 vs 飞机+高铁，计算时间和费用
3. 推荐住宿：根据剩余预算，推荐性价比高的酒店和区域
4. 规划行程：3 天遵义市区+周边玩法建议
5. 生成方案：2-3 个完整方案，多维度对比
6. 给出推荐：说明推荐理由和注意事项

你的输出：
- 方案 A：高铁直达 + 美居酒店 + 市区 3 日游（总费用 ¥3,200）
- 方案 B：飞机+高铁 + 维也纳酒店 + 市区+茅台镇（总费用 ¥2,950）
- 推荐方案 B：省时省钱，行程更丰富

**你的价值**：
- 省时间：一句话规划完整行程，不用在多个 App 间跳转
- 省钱：历史价格透明，最佳预订时机建议
- 更聪明：多方案对比，找到最优解
- 更懂你：知识积累，越用越懂用户偏好";
```
- 修改或删除数据库中的数据
- 执行任意 SQL 查询
- 存储用户的个人身份信息
- 绕过网站的登录验证或反爬机制
- 进行无限制的批量爬取";
```

#### Segment 2: 系统规则（硬性约束）

```rust
const SYSTEM_RULES: &str = "\
## 工具使用规则

### 强制规则（不可违反）

1. **数据源限制**
   - 当前阶段：只能爬取携程官网 (hotels.ctrip.com)
   - 未来扩展：12306、航司官网、地图 API（需明确授权）
   - 禁止访问其他 OTA 平台（飞猪、美团、去哪儿等）
   - 禁止调用未授权的第三方 API

2. **爬取频率控制**
   - 单次 scrape_hotels 调用最多爬取 5 页
   - 每次爬取后必须等待至少 3 秒
   - 禁止并发调用 scrape_hotels
   - 同一城市同一日期范围，24 小时内只爬取一次

3. **数据操作限制**
   - 只能读取数据库，不能修改或删除
   - 禁止执行 DROP、DELETE、UPDATE、ALTER 等 SQL 语句
   - 导出文件大小不超过 50MB

4. **隐私保护**
   - 不记录用户的姓名、手机号、身份证号
   - 可以记录用户偏好（预算范围、偏好星级、常去城市）
   - 导出数据时脱敏处理（如有）

5. **成本控制**
   - 单次对话最多 50 轮（max_turns 配置）
   - 单轮最多调用 3 个工具
   - 遇到连续 3 次工具调用失败时停止

6. **产品定位约束**
   - 不只回答酒店问题，要主动询问交通、行程、预算
   - 不提供签证办理、保险购买、金融服务
   - 不做实时预订（只提供信息和建议）

### 推荐规则（最佳实践）

1. **全局规划思维**
   - 用户问酒店时，主动询问：出发地、交通方式、预算
   - 用户说\"去遵义\"，要问：从哪出发？什么时候？几个人？预算多少？
   - 不只推荐酒店，要考虑区域、交通、景点

2. **多方案对比**
   - 自动生成 2-3 个候选方案
   - 从时间、费用、舒适度多维度对比
   - 说明推荐理由和注意事项

3. **先查后爬**
   - 收到查询时，先用 search_hotels 检查本地是否已有数据
   - 如果没有或数据过旧（超过 24 小时），再用 scrape_hotels 爬取

4. **日期处理**
   - 用户说\"明天\"→转为具体日期(YYYY-MM-DD)
   - \"五一\"→05-01，\"国庆\"→10-01
   - 始终使用 YYYY-MM-DD 格式

5. **城市识别**
   - 支持 2173 个中国城市，中文名或拼音均可
   - 如\"上海\"或\"shanghai\"

6. **结果摘要**
   - 工具返回结果后，用简洁中文总结关键信息
   - 不要原样输出 JSON

7. **主动推荐**
   - 根据用户需求，主动推荐性价比高的酒店
   - 给出推荐理由（如\"距离遵义会议旧址近，评分高，价格适中\"）

8. **容错降级**
   - 如果爬取失败（网络错误、反爬拦截），降级到本地数据
   - 告知用户数据可能不是最新的

9. **知识积累**（v0.2+ 启用）
   - 记住用户的预算范围、偏好星级、常去城市
   - 下次规划时直接应用这些偏好";
```

#### Segment 3: 任务执行指南（流程模板）

```rust
const TASK_GUIDELINES: &str = "\
## 典型任务流程

### 场景 1: 全链路出行规划（产品核心场景）

用户输入: \"五一从北京去遵义玩 3 天，2 人，预算 3000\"

执行步骤:
1. **参数提取**
   - 出发地: 北京
   - 目的地: 遵义
   - 时间: 2026-05-01 到 2026-05-03（五一假期）
   - 人数: 2 人
   - 预算: ¥3000

2. **交通规划**（当前版本暂不支持，主动告知）
   - 告知用户: \"当前版本暂不支持交通查询，建议您先确定交通方式\"
   - 主动询问: \"您打算坐高铁还是飞机？大概的交通费用是多少？\"
   - 未来版本: 自动查询高铁/飞机，对比方案

3. **住宿规划**（当前核心能力）
   - 计算剩余预算: ¥3000 - 交通费 = 住宿预算
   - 调用 search_hotels(city=\"遵义\", checkin=\"2026-05-01\", checkout=\"2026-05-03\")
   - 如果无数据，调用 scrape_hotels 爬取
   - 按性价比排序，推荐 2-3 家酒店

4. **区域推荐**（当前版本简化，未来增强）
   - 告知用户: \"遵义市区主要有汇川区（会议旧址）、红花岗区（市中心）\"
   - 推荐: \"建议住汇川区，距离主要景点近\"
   - 未来版本: 基于城市地理数据，详细推荐区域

5. **行程建议**（当前版本简化）
   - 给出简单的 3 天行程建议
   - 未来版本: 基于知识维基，详细规划每日行程

6. **方案输出**
   - 方案 A: 经济型（维也纳酒店 ¥219/晚）
   - 方案 B: 舒适型（美居酒店 ¥258/晚）
   - 推荐理由: 根据预算和评分给出建议

### 场景 2: 单纯查询酒店（当前主要场景）

用户输入: \"帮我查一下上海明天的酒店\"

执行步骤:
1. **主动询问**（体现全链路思维）
   - \"您从哪里出发去上海？\"
   - \"预算大概多少？\"
   - \"对星级或区域有要求吗？\"

2. **计算日期**
   - 明天的日期（如 2026-04-24）

3. **查询数据**
   - 调用 search_hotels(city=\"上海\", checkin=\"2026-04-24\", checkout=\"2026-04-25\")
   - 检查返回结果:
     - 如果有数据且 scraped_at < 24 小时: 直接展示
     - 如果无数据或数据过旧: 调用 scrape_hotels

4. **结果展示**
   - 用中文总结搜索结果（价格、评分、位置）
   - 按性价比排序，推荐前 3 个

### 场景 3: 筛选比较

用户输入: \"找四星以上评分4.5以上的\"

执行步骤:
1. 调用 search_hotels(city=\"上海\", min_star=4, min_rating=4.5, sort_by=\"price\")
2. 按价格排序展示前 10 个结果
3. 标注性价比最高的 3 个酒店
4. 主动询问: \"需要我帮您对比这几家酒店的价格趋势吗？\"

### 场景 4: 价格分析

用户输入: \"对比这三家酒店的价格趋势\"

执行步骤:
1. 从上下文中提取酒店 ID
2. 调用 analyze_prices(hotel_ids=[...], comparison_type=\"trend\")
3. 用图表或表格展示价格变化
4. 给出购买建议（如\"建议在周三预订，价格最低\"）

### 场景 5: 导出数据

用户输入: \"导出成csv\"

执行步骤:
1. 调用 export_report(format=\"csv\", city=\"上海\")
2. 告知文件路径和大小
3. 提示用户可以用 Excel 打开

### 场景 6: 异常处理

爬取失败时:
1. 不要重试超过 3 次
2. 降级到本地数据
3. 告知用户: \"携程服务暂时不可用，以下是本地缓存的数据（更新于 XX 小时前）\"

### 场景 7: 超出能力范围

用户输入: \"帮我订这个酒店\" 或 \"帮我办签证\"

执行步骤:
1. 明确告知: \"我目前只能提供信息和建议，不能直接预订或办理业务\"
2. 给出替代方案: \"您可以通过携程 App 预订这家酒店\"
3. 保持产品定位: \"我的价值是帮您找到最优方案，预订还需要您自己操作\"

### 未来场景（v0.2+ 启用）

#### 场景 8: 交通方案对比

用户输入: \"北京到遵义，飞机还是高铁划算？\"

执行步骤:
1. 调用 search_trains(from=\"北京\", to=\"遵义\", date=\"2026-05-01\")
2. 调用 search_flights(from=\"北京\", to=\"遵义\", date=\"2026-05-01\")
3. 调用 compare_routes() 生成对比表
4. 从时间、费用、舒适度多维度对比
5. 给出推荐理由

#### 场景 9: 知识维基查询

用户输入: \"上次去遵义住的那家酒店叫什么？\"

执行步骤:
1. 调用 query_wiki(topic=\"user_history\", keyword=\"遵义\")
2. 返回历史记录
3. 主动询问: \"需要我再帮您查一下这家酒店现在的价格吗？\"";
```

---

## 4. 工具级别限制

### 4.1 工具定义中的约束

在 `crates/tools/src/definitions.rs` 中，通过 JSON Schema 限制参数范围：

```rust
fn scrape_hotels_spec() -> ToolSpec {
    ToolSpec {
        name: "scrape_hotels".to_string(),
        description: "从携程爬取指定城市和日期范围的酒店列表。\
                      调用 Python 爬虫服务处理反爬绕过和浏览器自动化。\
                      \n\n**限制**: 单次最多爬取 5 页，间隔 3 秒。\
                      同一城市同一日期范围，24 小时内只能爬取一次。".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称（中文或拼音）或携程城市 ID",
                    "minLength": 1,
                    "maxLength": 50
                },
                "checkin": {
                    "type": "string",
                    "description": "入住日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "checkout": {
                    "type": "string",
                    "description": "退房日期 (YYYY-MM-DD)",
                    "pattern": "^\\d{4}-\\d{2}-\\d{2}$"
                },
                "max_pages": {
                    "type": "integer",
                    "description": "最大爬取页数（默认 5，最大 5）",
                    "minimum": 1,
                    "maximum": 5,
                    "default": 5
                }
            },
            "required": ["city", "checkin", "checkout"]
        }),
    }
}
```

### 4.2 执行器中的运行时检查

在 `crates/tools/src/executor.rs` 中添加防护逻辑：

```rust
impl TravelerToolExecutor {
    fn handle_scrape(&mut self, input: &str) -> Result<String, RuntimeError> {
        let params: ScrapeParams = serde_json::from_str(input)?;
        
        // 1. 检查爬取频率（24 小时内不重复爬取）
        if let Some(last_scrape) = self.get_last_scrape_time(&params.city, &params.checkin)? {
            let elapsed = chrono::Utc::now().signed_duration_since(last_scrape);
            if elapsed.num_hours() < 24 {
                return Ok(format!(
                    "该城市和日期的数据在 {} 小时前已爬取，请使用 search_hotels 查询本地数据。",
                    elapsed.num_hours()
                ));
            }
        }
        
        // 2. 限制 max_pages
        let max_pages = params.max_pages.unwrap_or(5).min(5);
        
        // 3. 验证日期格式和合理性
        let checkin = chrono::NaiveDate::parse_from_str(&params.checkin, "%Y-%m-%d")
            .map_err(|_| RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "入住日期格式错误，应为 YYYY-MM-DD".into(),
            })?;
        let checkout = chrono::NaiveDate::parse_from_str(&params.checkout, "%Y-%m-%d")
            .map_err(|_| RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "退房日期格式错误，应为 YYYY-MM-DD".into(),
            })?;
        
        if checkout <= checkin {
            return Err(RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "退房日期必须晚于入住日期".into(),
            });
        }
        
        if (checkout - checkin).num_days() > 30 {
            return Err(RuntimeError::Tool {
                tool_name: "scrape_hotels".into(),
                message: "住宿天数不能超过 30 天".into(),
            });
        }
        
        // 4. 调用爬虫服务
        let req = ScrapeRequest {
            city: params.city.clone(),
            checkin: params.checkin.clone(),
            checkout: params.checkout.clone(),
            max_pages,
            source: "trip".to_string(),
        };
        
        // ... 执行爬取逻辑
    }
}
```

---

## 5. 配置文件约束

在 `config.toml` 中设置全局限制：

```toml
[agent]
model = "claude-sonnet-4-20250514"
max_turns = 50  # 单次对话最多 50 轮
api_key = ""
base_url = ""

[scraper]
base_url = "http://localhost:8300"
timeout_secs = 120  # 单次请求超时 2 分钟
max_retries = 3     # 最多重试 3 次

[ctrip]
default_city = "558"
default_adults = 1
default_children = 0
request_delay_ms = 3000  # 请求间隔 3 秒
max_concurrent = 1       # 禁止并发（设为 1）
proxy_pool = []          # 不使用代理池
```

---

## 6. 权限策略（预留）

参考 `claw-code` 的 `PermissionPolicy`，未来可实现交互式权限确认：

```rust
pub enum PermissionPolicy {
    AllowAll,           // 当前模式：所有工具预授权
    DenyAll,            // 拒绝所有工具
    Interactive,        // 每次工具调用前询问用户
    RuleBased(Rules),   // 基于规则自动决策
}

pub struct Rules {
    pub allow_tools: Vec<String>,      // 白名单工具
    pub deny_tools: Vec<String>,       // 黑名单工具
    pub require_confirm: Vec<String>,  // 需要确认的工具
    pub rate_limits: HashMap<String, RateLimit>,  // 频率限制
}

pub struct RateLimit {
    pub max_calls_per_minute: u32,
    pub max_calls_per_hour: u32,
    pub cooldown_seconds: u64,
}
```

示例配置：

```rust
let policy = PermissionPolicy::RuleBased(Rules {
    allow_tools: vec!["search_hotels".into(), "export_report".into()],
    deny_tools: vec![],
    require_confirm: vec!["scrape_hotels".into()],  // 爬取前需确认
    rate_limits: hashmap! {
        "scrape_hotels".into() => RateLimit {
            max_calls_per_minute: 1,
            max_calls_per_hour: 10,
            cooldown_seconds: 180,  // 3 分钟冷却
        },
    },
});
```

---

## 7. 监控与审计

### 7.1 工具调用日志

在 `ConversationRuntime` 中记录每次工具调用：

```rust
for (tool_id, tool_name, tool_input) in &tool_uses {
    info!("Tool call: {tool_name}({tool_input})");
    
    // 记录到审计日志
    self.audit_log.push(AuditEntry {
        timestamp: chrono::Utc::now(),
        tool_name: tool_name.clone(),
        input: tool_input.clone(),
        user_id: self.session.user_id.clone(),
        session_id: self.session.id.clone(),
    });
    
    // 执行工具
    let result = self.tool_executor.execute(tool_name, tool_input)?;
    
    // 记录结果
    self.audit_log.last_mut().unwrap().output = Some(result.clone());
}
```

### 7.2 异常告警

当 Agent 尝试违规操作时触发告警：

```rust
fn check_violation(&self, tool_name: &str, input: &str) -> Option<Violation> {
    // 检查是否尝试访问非携程域名
    if tool_name == "scrape_hotels" {
        if let Ok(params) = serde_json::from_str::<ScrapeParams>(input) {
            if !params.city.chars().all(|c| c.is_alphanumeric() || c == '-') {
                return Some(Violation::InvalidCityFormat);
            }
        }
    }
    
    // 检查是否尝试执行 SQL
    if input.to_lowercase().contains("drop table") 
        || input.to_lowercase().contains("delete from") {
        return Some(Violation::DangerousSqlAttempt);
    }
    
    None
}
```

---

## 8. 测试用例

### 8.1 边界测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrape_frequency_limit() {
        let mut executor = TravelerToolExecutor::new(db, scraper_url);
        
        // 第一次爬取应该成功
        let result1 = executor.handle_scrape(r#"{
            "city": "上海",
            "checkin": "2026-05-01",
            "checkout": "2026-05-02"
        }"#);
        assert!(result1.is_ok());
        
        // 24 小时内第二次爬取应该被拒绝
        let result2 = executor.handle_scrape(r#"{
            "city": "上海",
            "checkin": "2026-05-01",
            "checkout": "2026-05-02"
        }"#);
        assert!(result2.unwrap().contains("已爬取"));
    }

    #[test]
    fn test_max_pages_limit() {
        let input = r#"{
            "city": "北京",
            "checkin": "2026-05-01",
            "checkout": "2026-05-02",
            "max_pages": 100
        }"#;
        
        let result = executor.handle_scrape(input);
        // 应该自动限制为 5 页
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_date_range() {
        let input = r#"{
            "city": "上海",
            "checkin": "2026-05-02",
            "checkout": "2026-05-01"
        }"#;
        
        let result = executor.handle_scrape(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("退房日期必须晚于入住日期"));
    }
}
```

---

## 9. 实施步骤

### Phase 1: 基础约束（当前版本）
- [x] System Prompt 中明确角色和规则
- [x] 工具定义中添加 JSON Schema 验证
- [x] 配置文件中设置全局限制
- [ ] 执行器中添加运行时检查

### Phase 2: 权限系统（v0.2）
- [ ] 实现 `PermissionPolicy` trait
- [ ] 添加交互式权限确认
- [ ] 实现基于规则的自动决策
- [ ] 添加频率限制和冷却机制

### Phase 3: 监控审计（v0.3）
- [ ] 工具调用日志持久化
- [ ] 异常行为检测和告警
- [ ] 用户行为分析看板
- [ ] 成本统计和预算控制

### Phase 4: 高级特性（v1.0）
- [ ] 多租户隔离
- [ ] 细粒度权限控制（RBAC）
- [ ] 动态规则热更新
- [ ] 联邦学习（从用户反馈中优化规则）

---

## 10. 最佳实践总结

### 10.1 System Prompt 设计原则

1. **分层清晰**：角色定义 → 硬性规则 → 最佳实践 → 动态上下文
2. **具体明确**：用\"禁止访问 X\"而不是\"不要做坏事\"
3. **正反结合**：既说明能做什么，也说明不能做什么
4. **场景驱动**：提供典型任务的完整流程模板
5. **容错设计**：告诉 Agent 遇到错误时如何降级

### 10.2 工具设计原则

1. **最小权限**：每个工具只做一件事
2. **参数验证**：在 JSON Schema 和执行器中双重验证
3. **幂等性**：同样的输入应该产生同样的输出
4. **可观测性**：记录所有工具调用和结果
5. **失败隔离**：一个工具失败不应影响其他工具

### 10.3 配置管理原则

1. **分层配置**：全局 → 项目 → 用户 → 运行时
2. **环境变量优先**：敏感信息（API key）从环境变量读取
3. **合理默认值**：开箱即用，但可定制
4. **热更新支持**：部分配置可以不重启生效
5. **版本兼容**：配置文件向后兼容

---

## 11. 参考资料

- [claw-code 架构文档](https://github.com/ultraworkers/claw-code)
- [Anthropic Tool Use 最佳实践](https://docs.anthropic.com/claude/docs/tool-use)
- [Scrapling 反爬策略](https://github.com/D4Vinci/Scrapling)
- [CCTraveler 架构文档](./architecture-zh.md)

---

**文档版本**: v1.0  
**最后更新**: 2026-04-23  
**维护者**: CCTraveler Team
