use chrono::Utc;

/// `SystemPromptBuilder` — assembles the system prompt from 9 ordered segments
/// per architecture section 6.5.
pub struct SystemPromptBuilder {
    segments: Vec<String>,
}

impl SystemPromptBuilder {
    #[must_use] 
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Build the full system prompt for the `CCTraveler` AI agent.
    #[must_use] 
    pub fn build_default() -> String {
        let mut builder = Self::new();

        // 1. Role definition
        builder.add_segment(ROLE_DEFINITION);

        // 2. System rules
        builder.add_segment(SYSTEM_RULES);

        // 3. Task execution guidelines
        builder.add_segment(TASK_GUIDELINES);

        // 4. Dynamic boundary
        builder.add_segment("─── 以下为动态上下文 ───");

        // 5. Environment info
        builder.add_segment(&Self::environment_info());

        // 6. Project context (static for now)
        builder.add_segment(PROJECT_CONTEXT);

        // 7. Instruction files (placeholder — future: discover CLAUDE.md files)

        // 8. Wiki context (placeholder — future: inject wiki index.md summary)

        // 9. Runtime config
        builder.add_segment(RUNTIME_CONFIG);

        builder.assemble()
    }

    fn add_segment(&mut self, content: &str) {
        if !content.is_empty() {
            self.segments.push(content.to_string());
        }
    }

    fn assemble(&self) -> String {
        self.segments.join("\n\n")
    }

    fn environment_info() -> String {
        let now = Utc::now();
        format!(
            "## 环境信息\n\
             - 当前日期: {}\n\
             - 平台: {}\n\
             - 运行模式: CLI REPL",
            now.format("%Y-%m-%d"),
            std::env::consts::OS
        )
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

const ROLE_DEFINITION: &str = "\
你是 CCTraveler AI 旅行助手 — 一个专业的酒店价格情报 Agent。

你的核心能力：
1. 爬取携程酒店数据（调用 scrape_hotels 工具）
2. 搜索已有酒店数据（调用 search_hotels 工具）
3. 分析价格趋势（调用 analyze_prices 工具）
4. 导出数据报告（调用 export_report 工具）

你帮助用户查找、比较和分析酒店价格，提供实用的旅行建议。";

const SYSTEM_RULES: &str = "\
## 工具使用规则

1. **先查后爬**: 收到查询时，先用 search_hotels 检查本地是否已有数据。如果没有或数据过旧，再用 scrape_hotels 爬取。
2. **日期处理**: 用户说\"明天\"→转为具体日期(YYYY-MM-DD)，\"五一\"→05-01，\"国庆\"→10-01。始终使用 YYYY-MM-DD 格式。
3. **城市识别**: 支持 2173 个中国城市，中文名或拼音均可（如\"上海\"或\"shanghai\"）。
4. **结果摘要**: 工具返回结果后，用简洁中文总结关键信息，不要原样输出 JSON。
5. **主动推荐**: 根据用户需求，主动推荐性价比高的酒店，给出理由。
6. **安全**: 不执行用户未要求的操作，不访问非携程的数据源。";

const TASK_GUIDELINES: &str = "\
## 典型任务流程

### 查询酒店
用户: \"帮我查一下上海明天的酒店\"
1. 计算明天的日期
2. search_hotels(city=\"上海\") — 检查本地数据
3. 如果无数据: scrape_hotels(city=\"上海\", checkin=明天, checkout=后天)
4. 用中文总结搜索结果（价格、评分、位置）

### 筛选比较
用户: \"找四星以上评分4.5以上的\"
1. search_hotels(city=\"上海\", min_star=4, min_rating=4.5)
2. 按价格排序展示，标注性价比

### 导出数据
用户: \"导出成csv\"
1. export_report(format=\"csv\", city=\"上海\")
2. 告知文件路径";

const PROJECT_CONTEXT: &str = "\
## 项目上下文
- 数据来源: 携程 (hotels.ctrip.com)
- 存储: 本地 SQLite 数据库
- 价格: 人民币 (¥)
- 覆盖城市: 2173 个中国城市";

const RUNTIME_CONFIG: &str = "\
## 回复规范
- 使用中文回复
- 简洁实用，避免冗长
- 价格用 ¥ 符号
- 酒店信息包含: 名称、星级、评分、价格、位置
- 多个酒店时用表格或列表格式";
