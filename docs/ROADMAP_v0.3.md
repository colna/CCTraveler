# CCTraveler v0.3 技术方案

版本：v0.3  
日期：2026-04-23  
状态：规划中

## 1. 概述

v0.2 已实现交通查询的基础框架（使用 Mock 数据），v0.3 的目标是**接入真实数据源**并**完善用户体验**，使 CCTraveler 成为真正可用的全链路旅行规划助手。

### 核心目标
1. 接入真实 12306 和机票数据
2. 导入完整城市地理数据
3. 实现数据缓存和持久化
4. 优化 Agent 交互体验
5. 添加价格监控功能

## 2. 技术架构优化

### 2.1 数据层架构

```
┌─────────────────────────────────────────────────────────┐
│                    Agent Layer                          │
│  (自然语言理解 + 工具调用 + 多轮对话)                      │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                    Tool Layer                           │
│  search_trains | search_flights | compare_routes       │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                  Cache Layer (NEW)                      │
│  - Redis: 热数据缓存 (TTL: 1小时)                        │
│  - SQLite: 历史数据持久化                                │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│                  Scraper Layer                          │
│  12306 | Ctrip | Qunar | Fliggy                        │
└─────────────────────────────────────────────────────────┘
```

### 2.2 缓存策略

**三级缓存架构**：

1. **内存缓存**（进程内）
   - 城市代码映射表
   - 车站/机场代码表
   - TTL: 永久（启动时加载）

2. **Redis 缓存**（可选）
   - 火车票/机票查询结果
   - TTL: 1 小时
   - Key 格式: `train:{from}:{to}:{date}` / `flight:{from}:{to}:{date}`

3. **SQLite 持久化**
   - 历史价格数据（用于趋势分析）
   - 用户查询记录
   - 永久保存

### 2.3 数据更新策略

```rust
async fn search_trains(params: SearchParams) -> Result<Vec<Train>> {
    // 1. 检查内存缓存
    if let Some(cached) = MEMORY_CACHE.get(&params) {
        if !cached.is_expired() {
            return Ok(cached.data);
        }
    }

    // 2. 检查 Redis 缓存
    if let Some(cached) = redis.get(&params).await? {
        MEMORY_CACHE.set(params, cached);
        return Ok(cached);
    }

    // 3. 检查数据库（1小时内的数据）
    if let Some(db_data) = db.query_recent(params, Duration::hours(1)).await? {
        redis.set(&params, &db_data, Duration::hours(1)).await?;
        return Ok(db_data);
    }

    // 4. 调用爬虫服务
    let fresh_data = scraper.fetch(params).await?;
    
    // 5. 写入数据库和缓存
    db.insert(&fresh_data).await?;
    redis.set(&params, &fresh_data, Duration::hours(1)).await?;
    
    Ok(fresh_data)
}
```

## 3. 数据源接入方案

### 3.1 12306 火车票爬取

**技术方案**：

1. **反爬绕过**
   - 使用 `undetected-chromedriver` 绕过 Selenium 检测
   - 随机 User-Agent 和浏览器指纹
   - 模拟真实用户行为（鼠标轨迹、随机延迟）

2. **验证码处理**
   - 方案 A：使用 OCR 识别（准确率 ~70%）
   - 方案 B：接入打码平台（成本 ~0.01元/次）
   - 方案 C：人工介入（开发阶段）

3. **车站代码映射**
   - 从 12306 官网获取完整车站列表
   - 建立 `城市名 → 车站代码` 映射表
   - 支持模糊匹配（如"北京" → ["BJP", "BXP", "VNP"]）

**实现步骤**：

```python
# services/scraper/src/train/fetcher_12306.py

import undetected_chromedriver as uc
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait

async def fetch_trains_12306(from_city, to_city, travel_date):
    # 1. 初始化 undetected-chromedriver
    options = uc.ChromeOptions()
    options.add_argument('--headless')
    driver = uc.Chrome(options=options)
    
    # 2. 构建查询 URL
    from_code = get_station_code(from_city)
    to_code = get_station_code(to_city)
    url = f"https://kyfw.12306.cn/otn/leftTicket/query?..."
    
    # 3. 访问页面并等待加载
    driver.get(url)
    wait = WebDriverWait(driver, 10)
    wait.until(EC.presence_of_element_located((By.ID, "queryLeftTable")))
    
    # 4. 解析车次列表
    trains = []
    rows = driver.find_elements(By.CSS_SELECTOR, "#queryLeftTable tbody tr")
    for row in rows:
        train = parse_train_row(row)
        trains.append(train)
    
    driver.quit()
    return trains
```

### 3.2 机票数据聚合

**多源策略**：

| 数据源 | 优先级 | 覆盖范围 | 价格准确度 |
|--------|--------|----------|-----------|
| 携程 | P0 | 全面 | 高 |
| 去哪儿 | P1 | 全面 | 中 |
| 飞猪 | P2 | 部分 | 中 |

**聚合逻辑**：

```rust
async fn search_flights_aggregated(params: SearchParams) -> Result<Vec<Flight>> {
    // 并发查询多个数据源
    let (ctrip, qunar, fliggy) = tokio::join!(
        scraper.fetch_ctrip(params),
        scraper.fetch_qunar(params),
        scraper.fetch_fliggy(params),
    );
    
    // 合并去重（按航班号）
    let mut flights = HashMap::new();
    for flight in ctrip?.into_iter().chain(qunar?).chain(fliggy?) {
        flights.entry(flight.flight_id.clone())
            .and_modify(|f| {
                // 保留最低价格
                if flight.lowest_price < f.lowest_price {
                    *f = flight.clone();
                }
            })
            .or_insert(flight);
    }
    
    Ok(flights.into_values().collect())
}
```

### 3.3 城市地理数据导入

**数据来源**：

1. **城市列表**（2173 个城市）
   - 来源：国家统计局 + 高德地图 API
   - 字段：城市名、省份、经纬度、人口、面积、等级

2. **区域划分**
   - 来源：各城市政府官网 + 百度地图
   - 字段：区域名、所属城市、特点、标签

3. **景点数据**
   - 来源：携程 + 马蜂窝 + 大众点评
   - 字段：景点名、类别、评分、门票、游玩时长

**导入脚本**：

```bash
# scripts/import_geo_data.sh

# 1. 下载城市数据
curl -o data/cities.json https://example.com/cities.json

# 2. 导入数据库
cargo run --bin import_cities -- --file data/cities.json

# 3. 验证导入
sqlite3 data/cctraveler.db "SELECT COUNT(*) FROM cities;"
```

## 4. 功能增强

### 4.1 价格监控功能

**需求**：用户可以订阅特定路线的价格变化，当价格低于阈值时收到通知。

**技术方案**：

```rust
// crates/tools/src/monitor.rs

pub struct PriceMonitor {
    db: Database,
    notifier: Notifier,
}

impl PriceMonitor {
    pub async fn add_subscription(&self, sub: Subscription) -> Result<()> {
        // 1. 保存订阅到数据库
        self.db.insert_subscription(&sub).await?;
        
        // 2. 启动定时任务（每小时检查一次）
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::hours(1)).await;
                self.check_price_changes().await;
            }
        });
        
        Ok(())
    }
    
    async fn check_price_changes(&self) {
        let subs = self.db.get_active_subscriptions().await?;
        for sub in subs {
            let current_price = self.fetch_current_price(&sub).await?;
            if current_price < sub.threshold {
                self.notifier.send(&sub.user_id, &format!(
                    "价格提醒：{} → {} 的{}已降至 ¥{}",
                    sub.from_city, sub.to_city, sub.transport_type, current_price
                )).await?;
            }
        }
    }
}
```

**数据库表**：

```sql
CREATE TABLE price_subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    from_city TEXT NOT NULL,
    to_city TEXT NOT NULL,
    transport_type TEXT NOT NULL, -- 'train' or 'flight'
    threshold REAL NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
```

### 4.2 知识维基（用户偏好记忆）

**需求**：记住用户的预算范围、偏好星级、常去城市等信息。

**实现方案**：

```rust
// crates/storage/src/wiki.rs

pub struct WikiManager {
    db: Database,
}

impl WikiManager {
    pub async fn remember(&self, topic: &str, key: &str, value: &str) -> Result<()> {
        self.db.upsert_wiki_entry(topic, key, value).await
    }
    
    pub async fn recall(&self, topic: &str, key: &str) -> Result<Option<String>> {
        self.db.get_wiki_entry(topic, key).await
    }
}

// 使用示例
wiki.remember("user_history", "budget_range", "500-1000").await?;
wiki.remember("user_history", "preferred_star", "4-5").await?;
wiki.remember("user_history", "frequent_cities", "[\"北京\",\"上海\",\"深圳\"]").await?;
```

**Agent 集成**：

```rust
// 在 search_hotels 工具中自动应用用户偏好
let budget = wiki.recall("user_history", "budget_range").await?;
let preferred_star = wiki.recall("user_history", "preferred_star").await?;

if let Some(budget) = budget {
    let (min, max) = parse_budget_range(&budget);
    filters.min_price = Some(min);
    filters.max_price = Some(max);
}
```

### 4.3 完整行程规划

**需求**：根据用户的出发地、目的地、时间、预算，生成完整的行程方案（交通+住宿+景点）。

**工具定义**：

```rust
fn plan_trip_spec() -> ToolSpec {
    ToolSpec {
        name: "plan_trip".to_string(),
        description: "生成完整的旅行行程方案，包括交通、住宿、景点推荐。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "from_city": { "type": "string" },
                "to_city": { "type": "string" },
                "start_date": { "type": "string" },
                "end_date": { "type": "string" },
                "budget": { "type": "number" },
                "travelers": { "type": "integer" },
                "preferences": {
                    "type": "object",
                    "properties": {
                        "transport_priority": { "enum": ["time", "cost", "comfort"] },
                        "hotel_star": { "type": "integer" },
                        "interests": { "type": "array", "items": { "type": "string" } }
                    }
                }
            },
            "required": ["from_city", "to_city", "start_date", "end_date", "budget"]
        }),
    }
}
```

**实现逻辑**：

```rust
pub async fn handle_plan_trip(params: PlanTripParams) -> Result<TripPlan> {
    // 1. 计算天数
    let days = (params.end_date - params.start_date).num_days();
    
    // 2. 查询交通方案
    let routes = compare_routes(&params).await?;
    let best_route = routes.iter()
        .min_by_key(|r| match params.preferences.transport_priority {
            "time" => r.total_minutes,
            "cost" => r.total_cost as i32,
            "comfort" => -(r.comfort_score as i32),
        })
        .unwrap();
    
    // 3. 计算剩余预算
    let remaining_budget = params.budget - best_route.total_cost;
    let hotel_budget = remaining_budget * 0.6; // 60% 用于住宿
    let food_budget = remaining_budget * 0.3;  // 30% 用于餐饮
    let activity_budget = remaining_budget * 0.1; // 10% 用于门票
    
    // 4. 查询酒店
    let hotels = search_hotels(SearchHotelsParams {
        city: params.to_city.clone(),
        checkin: params.start_date,
        checkout: params.end_date,
        max_price: Some(hotel_budget / days as f64),
        min_star: params.preferences.hotel_star,
        ..Default::default()
    }).await?;
    
    // 5. 查询景点
    let attractions = query_city_attractions(&params.to_city).await?;
    let recommended_attractions = attractions.iter()
        .filter(|a| {
            params.preferences.interests.is_empty() ||
            params.preferences.interests.contains(&a.category)
        })
        .filter(|a| a.ticket_price.unwrap_or(0.0) <= activity_budget / days as f64)
        .take(days as usize * 2) // 每天推荐 2 个景点
        .collect::<Vec<_>>();
    
    // 6. 生成行程
    Ok(TripPlan {
        transport: best_route.clone(),
        hotel: hotels.first().cloned(),
        daily_plans: generate_daily_plans(days, &recommended_attractions),
        budget_breakdown: BudgetBreakdown {
            transport: best_route.total_cost,
            hotel: hotel_budget,
            food: food_budget,
            activities: activity_budget,
            total: params.budget,
        },
    })
}
```

## 5. 性能优化

### 5.1 并发查询优化

**问题**：当前 `compare_routes` 串行调用火车票和机票查询，耗时较长。

**优化方案**：

```rust
pub async fn handle_compare_routes(params: CompareRoutesParams) -> Result<String> {
    // 并发查询火车票和机票
    let (trains, flights) = tokio::join!(
        search_trains_internal(&params),
        search_flights_internal(&params),
    );
    
    let trains = trains?;
    let flights = flights?;
    
    // 生成对比结果
    generate_comparison(trains, flights, &params)
}
```

**预期效果**：响应时间从 2-3 秒降至 1-1.5 秒。

### 5.2 数据库查询优化

**索引优化**：

```sql
-- 火车票查询索引
CREATE INDEX idx_trains_route_date ON trains(from_city, to_city);
CREATE INDEX idx_train_prices_date ON train_prices(travel_date, scraped_at);

-- 机票查询索引
CREATE INDEX idx_flights_route_date ON flights(from_city, to_city);
CREATE INDEX idx_flight_prices_date ON flight_prices(travel_date, scraped_at);

-- 城市查询索引
CREATE INDEX idx_cities_name ON cities(name);
CREATE INDEX idx_attractions_city ON attractions(city_id, category);
```

### 5.3 爬虫服务优化

**连接池**：

```python
# services/scraper/src/utils/pool.py

from selenium import webdriver
from queue import Queue

class DriverPool:
    def __init__(self, size=5):
        self.pool = Queue(maxsize=size)
        for _ in range(size):
            driver = self._create_driver()
            self.pool.put(driver)
    
    def get(self):
        return self.pool.get()
    
    def put(self, driver):
        self.pool.put(driver)
    
    def _create_driver(self):
        options = uc.ChromeOptions()
        options.add_argument('--headless')
        return uc.Chrome(options=options)
```

**预期效果**：避免每次查询都创建新的浏览器实例，提升 50% 性能。

## 6. 实施计划

### Phase 1: 数据导入（1 周）

- [ ] 导入 2173 个城市数据
- [ ] 导入车站代码映射表（~3000 个车站）
- [ ] 导入机场代码映射表（~200 个机场）
- [ ] 导入主要城市的区域和景点数据

### Phase 2: 12306 接入（2 周）

- [ ] 实现 undetected-chromedriver 反爬
- [ ] 处理验证码（接入打码平台）
- [ ] 完善车次解析逻辑
- [ ] 测试稳定性（成功率 >95%）

### Phase 3: 机票接入（2 周）

- [ ] 实现携程机票爬取
- [ ] 实现去哪儿机票爬取
- [ ] 实现多源数据聚合
- [ ] 测试价格准确性

### Phase 4: 缓存层（1 周）

- [ ] 实现 Redis 缓存（可选）
- [ ] 实现数据库查询逻辑
- [ ] 实现三级缓存策略
- [ ] 性能测试

### Phase 5: 功能增强（2 周）

- [ ] 实现价格监控功能
- [ ] 实现知识维基
- [ ] 实现完整行程规划工具
- [ ] 优化 Agent 交互体验

### Phase 6: 测试和优化（1 周）

- [ ] 端到端测试
- [ ] 性能优化
- [ ] 文档完善
- [ ] 发布 v0.3

**总计：9 周**

## 7. 风险评估

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 12306 反爬升级 | 高 | 中 | 准备多套反爬方案，建立监控告警 |
| 验证码识别失败 | 中 | 高 | 接入打码平台作为备选方案 |
| 数据源不稳定 | 中 | 中 | 多源聚合，降级到缓存数据 |
| 性能瓶颈 | 中 | 低 | 提前做性能测试，优化热点路径 |
| API 限流 | 低 | 中 | 实现请求限流和重试机制 |

## 8. 成功指标

### 功能指标
- ✅ 12306 爬取成功率 >95%
- ✅ 机票数据覆盖率 >90%
- ✅ 城市地理数据完整度 100%（2173 个城市）

### 性能指标
- ✅ 火车票查询响应时间 <2 秒
- ✅ 机票查询响应时间 <2 秒
- ✅ 方案对比响应时间 <3 秒
- ✅ 缓存命中率 >60%

### 用户体验指标
- ✅ Agent 理解准确率 >90%
- ✅ 推荐方案满意度 >80%
- ✅ 完整行程规划成功率 >85%

## 9. 技术债务

### 当前技术债
1. ⚠️ Mock 数据需要替换为真实数据
2. ⚠️ 缺少数据库查询逻辑
3. ⚠️ 缺少错误处理和重试机制
4. ⚠️ 缺少日志和监控

### 优化建议
1. 引入 OpenTelemetry 进行分布式追踪
2. 添加 Prometheus 指标监控
3. 实现优雅降级机制
4. 完善单元测试和集成测试

## 10. 附录

### 10.1 相关文档
- [v0.2 测试报告](./TEST_REPORT_v0.2.md)
- [数据库设计文档](./DATABASE_DESIGN.md)
- [API 文档](./API_REFERENCE.md)

### 10.2 参考资料
- [12306 反爬技术分析](https://example.com/12306-anti-scraping)
- [undetected-chromedriver 文档](https://github.com/ultrafunkamsterdam/undetected-chromedriver)
- [携程机票爬取实践](https://example.com/ctrip-flight-scraping)

---

**文档版本**：v1.0  
**最后更新**：2026-04-23  
**负责人**：开发团队
