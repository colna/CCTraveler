use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod doctor;
mod init;
mod oneshot;
mod repl;

#[derive(Parser)]
#[command(
    name = "cctraveler",
    version,
    about = "AI Travel Planner — 终端中的旅行 Agent"
)]
struct Cli {
    /// 显式指定配置文件路径（默认按层级搜索 ~/.cctraveler / ./.cctraveler / ./config.toml）
    #[arg(long)]
    config: Option<PathBuf>,

    /// 一次性模式：直接发送 prompt，拿到回答后退出
    #[arg(short = 'p', long, value_name = "PROMPT")]
    prompt: Option<String>,

    /// 覆盖模型
    #[arg(long)]
    model: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 显式进入 AI 对话（等价于不传子命令）
    Chat,
    /// 初始化配置向导，写入 ~/.cctraveler/config.toml
    Init,
    /// 诊断环境（API key / scraper / sqlite / 网络）
    Doctor,
    /// 抓取携程酒店列表
    Scrape {
        #[arg(long)]
        city: String,
        #[arg(long)]
        checkin: String,
        #[arg(long)]
        checkout: String,
        #[arg(long, default_value = "5")]
        max_pages: u32,
    },
    /// 在本地数据库中检索酒店
    Search {
        #[arg(long)]
        city: Option<String>,
        #[arg(long)]
        max_price: Option<f64>,
        #[arg(long)]
        min_star: Option<u8>,
        #[arg(long)]
        min_rating: Option<f64>,
        #[arg(long)]
        sort_by: Option<String>,
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// 导出抓取的数据（csv / json）
    Export {
        #[arg(long)]
        format: String,
        #[arg(long)]
        city: Option<String>,
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // init / doctor 不需要先加载配置（init 用来生成、doctor 容忍缺失）
    match &cli.command {
        Some(Commands::Init) => return init::run(),
        Some(Commands::Doctor) => return doctor::run(cli.config.as_deref()).await,
        _ => {}
    }

    let mut config = runtime::RuntimeConfig::load_layered(cli.config.as_deref())?;

    // CLI 覆盖：模型
    if let Some(m) = &cli.model {
        config.agent.model = m.clone();
    }

    // 准备 db 目录
    let db_path = PathBuf::from(&config.storage.db_path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 调度
    match (cli.command, cli.prompt) {
        (Some(Commands::Chat), _) | (None, None) => repl::run(&config, &db_path)?,
        (None, Some(p)) => oneshot::run(&config, &db_path, Some(p))?,
        (Some(Commands::Scrape {
            city,
            checkin,
            checkout,
            max_pages,
        }), _) => commands::scrape(&config, &db_path, city, checkin, checkout, max_pages).await?,
        (Some(Commands::Search {
            city,
            max_price,
            min_star,
            min_rating,
            sort_by,
            limit,
        }), _) => commands::search(&db_path, city, max_price, min_star, min_rating, sort_by, limit)?,
        (Some(Commands::Export {
            format,
            city,
            output,
        }), _) => commands::export(&db_path, format, city, output)?,
        (Some(Commands::Init | Commands::Doctor), _) => unreachable!(),
    }

    Ok(())
}
