//! locifindd binary 入口。
//!
//! BETA-32 T10 接全套：CLI parse → tracing init → preflight fail-fast →
//! `ServerCtx` 构造（打开 indexer DB + 加载 embedder + 首次全量索引）→
//! axum Router 装配 → [`lifecycle::serve`] 阻塞直到信号。
//!
//! BETA-36：启动形态二选一——legacy 单根（`--root` + `--token` 合成 default
//! collection + 全权 admin token）或 collection 模式（`--config <TOML>`）；
//! per-collection 独立 index.db（物理信息墙），布局见
//! `locifind_server::config::collection_db_path`。
//!
//! 显式 allow `print_stdout` / `print_stderr`：daemon binary 启动/收尾阶段
//! 必须直接写 stdout/stderr（在 tracing subscriber 初始化前后）。同款做法
//! 见 apps/locifind-cli。

#![forbid(unsafe_code)]
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod cli;
mod lifecycle;
mod preflight;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use parking_lot::{Mutex, RwLock};
use secrecy::SecretString;
use tracing::{info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::{
    default_ocr_engine, DocumentIndex, GlobSet, IndexError, IndexStats, MusicIndex, NoopProgress,
    OcrEngine, PopplerPdfRasterizer,
};
use locifind_model_runtime::{ModelDaemon, ModelLoadParams};
use locifind_server::app::build_app;
use locifind_server::collections::{parse_config_toml, DaemonConfigFile};
use locifind_server::config::{
    collection_db_path, CollectionRuntime, CollectionState, IndexingProbe, ServerConfig,
};
use locifind_server::ServerCtx;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_tracing(&cli.log_level, &cli.log_format)?;

    // ---- BETA-36：解析访问模型（--config TOML 与 --root/--token 互斥）----
    let access = resolve_access_config(&cli)?;

    // ---- preflight fail-fast ----
    for c in &access.collections {
        for root in &c.roots {
            preflight::check_root(root)
                .with_context(|| format!("collection {} 的 root 检查失败", c.id))?;
        }
    }
    preflight::check_data_dir(&cli.data_dir)?;
    preflight::check_model(&cli.model_path)?;
    // reindex 中断残留按 collection db 目录逐一检查（legacy default = data_dir 平铺）。
    for c in &access.collections {
        let db = collection_db_path(&cli.data_dir, &c.id);
        if let Some(dir) = db.parent() {
            if dir.exists() {
                preflight::check_rebuild_leftover(dir, cli.allow_rebuild_schema)?;
            }
        }
    }
    // bind 端口检查：留 lifecycle::serve 真 bind 时报错（TOCTOU 风险下不重复 try）。

    let log_level = parse_log_level(&cli.log_level);

    let config = ServerConfig {
        bind_addr: cli.bind,
        data_dir: cli.data_dir,
        model_path: cli.model_path,
        log_level,
        semantic_weight: cli
            .semantic_weight
            .unwrap_or(locifind_server::tools::search::DEFAULT_SEMANTIC_WEIGHT),
        embed_images: !cli.disable_image_semantics,
        access,
    };

    // ---- ctx 构造（打开 db + 加载模型 + 首次全量索引）----
    let ctx = Arc::new(build_runtime_ctx(config).await?);

    // ---- 装配 Router + 跑 server（阻塞到信号）----
    let app = build_app(ctx.clone());
    lifecycle::serve(ctx, app).await
}

/// 解析访问模型：`--config` TOML（collection 模式）或 `--root`+`--token`（legacy）。
///
/// 互斥把守：两者都给 / 都不给 / legacy 缺一半 → 启动错误。
fn resolve_access_config(cli: &Cli) -> Result<DaemonConfigFile> {
    match (&cli.config, &cli.root, &cli.token) {
        (Some(path), None, None) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("读取配置文件失败：{}", path.display()))?;
            let cfg = parse_config_toml(&text)
                .with_context(|| format!("配置文件非法：{}", path.display()))?;
            info!(
                collections = cfg.collections.len(),
                tokens = cfg.tokens.len(),
                "collection 模式启动（TOML 配置）"
            );
            Ok(cfg)
        }
        (None, Some(root), Some(token)) => {
            preflight::check_token(token)?;
            info!(root = %root.display(), "legacy 单根模式启动（合成 default collection + 全权 token）");
            Ok(DaemonConfigFile::legacy_single_root(
                root.clone(),
                SecretString::from(token.clone()),
            ))
        }
        (Some(_), _, _) => Err(anyhow!(
            "--config 与 --root/--token 互斥：collection 模式下 token 在 TOML [[tokens]] 里声明"
        )),
        _ => Err(anyhow!(
            "启动参数不完整：要么给 --config <TOML>（collection 模式），要么同时给 --root 与 --token（legacy 单根）"
        )),
    }
}

/// 初始化 tracing subscriber：env-filter（fallback `info`）+ text/json 二选一。
fn init_tracing(level: &str, format: &str) -> Result<()> {
    let filter = EnvFilter::try_new(level).unwrap_or_else(|e| {
        eprintln!("[locifindd] 警告：--log-level={level} 无法解析（{e}），回退 info 级别");
        EnvFilter::new("info")
    });
    match format {
        "json" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init()
            .map_err(|e| anyhow!("tracing init 失败：{e}"))?,
        "text" => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init()
            .map_err(|e| anyhow!("tracing init 失败：{e}"))?,
        other => return Err(anyhow!("不支持的 log_format（应为 text 或 json）：{other}")),
    }
    Ok(())
}

/// 解析 log level 字符串到 [`LevelFilter`]。
fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_ascii_lowercase().as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "warn" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => LevelFilter::INFO,
    }
}

/// 构造运行时 [`ServerCtx`]：逐 collection 打开独立 index.db（music + documents
/// 两套 schema 共存单文件——dual-db 单文件共识，ultra-review C-1）、首次全量索引
/// 其 roots、装配候选链缓存与 [`CollectionState`]。
///
/// 索引层是 sync API（rusqlite），放进 [`tokio::task::spawn_blocking`] 跑、
/// 避免阻塞 tokio runtime worker。
async fn build_runtime_ctx(config: ServerConfig) -> Result<ServerCtx> {
    if !config.data_dir.exists() {
        std::fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("创建 data_dir 失败：{}", config.data_dir.display()))?;
    }

    info!(model_path = %config.model_path.display(), "加载 embedder 模型");
    let embedder = load_embedder(&config.model_path)?;

    // reviewer M-6：默认 daemon binary 不开 llama-cpp feature → ModelDaemon 走
    // StubLoader、`embed()` 返 Err。真启动跑一次 ping probe 发 warn、让 ops
    // 立刻知道运行在 FTS-only 降级模式。
    // BETA-40 收尾：probe 结果同时决定 ① 索引期是否跑 embed pass（写
    // document_vectors）② 候选链是否装语义臂——此前二者都缺席，daemon 实为 FTS-only。
    let semantic_ready = embedder.embed("ping").is_ok();
    if !semantic_ready {
        warn!(
            "embedder 不支持 embed()（默认 stub backend）；语义召回已禁用、\
             daemon 退化为 FTS-only。生产请用 --features semantic-recall（或\
             同款 llama-cpp 系列 feature）编译"
        );
    }

    let ocr_engine = probe_ocr_dependencies();

    // reviewer I-2：首次全量索引期间 axum 尚未 bind、/health 不响应；ops 需调大
    // supervisor 启动超时（README 已注明）。
    warn!(
        collections = config.access.collections.len(),
        "首次全量索引开始（逐 collection）；大目录可能耗时数分钟、期间 /health 不响应。\
         部署时请适当调高 launchd ThrottleInterval / systemd TimeoutStartSec"
    );

    let mut collections: BTreeMap<String, CollectionRuntime> = BTreeMap::new();
    for meta in config.access.collections.clone() {
        let db_path = collection_db_path(&config.data_dir, &meta.id);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "创建 collection {} 的索引目录失败：{}",
                    meta.id,
                    parent.display()
                )
            })?;
        }

        info!(collection = %meta.id, db = %db_path.display(), "打开索引数据库（music + documents 共用单文件）");
        let music_index = MusicIndex::open(&db_path)
            .with_context(|| format!("打开 MusicIndex 失败：{}", db_path.display()))?;
        let document_index = DocumentIndex::open(&db_path)
            .with_context(|| format!("打开 DocumentIndex 失败：{}", db_path.display()))?;

        // 首次全量索引：spawn_blocking 包 sync indexer；多 root 一次传入。
        let (music_index, document_index) = run_initial_collection_index(
            &meta,
            music_index,
            document_index,
            ocr_engine.clone(),
            embedder.clone(),
            semantic_ready,
            config.embed_images,
        )
        .await?;

        let music_count = music_index.count().context("MusicIndex.count() 失败")?;
        let document_count = document_index
            .count()
            .context("DocumentIndex.count() 失败")?;
        let doc_count = music_count.saturating_add(document_count);

        let state = CollectionState {
            indexed_at: Some(chrono::Utc::now()),
            doc_count,
            reindex_in_flight: false,
        };

        // 语义臂随 embedder probe 结果装配：ready → hybrid（FTS + 语义 RRF 融合）；
        // 否则 FTS-only（与旧行为一致）。
        let search_candidates = Arc::new(
            locifind_server::tools::search::build_local_search_candidates(
                db_path.clone(),
                semantic_ready.then(|| embedder.clone()),
            ),
        );

        let rt = CollectionRuntime {
            meta,
            db_path,
            music_index: Arc::new(Mutex::new(music_index)),
            document_index: Arc::new(Mutex::new(document_index)),
            search_candidates,
            state: Arc::new(RwLock::new(state)),
        };
        collections.insert(rt.meta.id.clone(), rt);
    }

    let audit = Arc::new(locifind_server::audit::AuditSink::new(
        &config.data_dir,
        config.access.audit.log_query,
    ));

    let indexing_probe: IndexingProbe = {
        let states: Vec<_> = collections.values().map(|rt| rt.state.clone()).collect();
        Arc::new(move || states.iter().any(|state| state.read().reindex_in_flight))
    };

    Ok(ServerCtx {
        config,
        embedder,
        collections,
        audit,
        indexing_probe,
    })
}

/// BETA-40 收尾：OCR / PDF 渲染依赖启动期探测留日志（此前静默缺失——图片不入索引、
/// 扫描 PDF 计 failed 都无从察觉）。OCR 引擎探测一次、全 collection 复用。
fn probe_ocr_dependencies() -> Option<Arc<dyn OcrEngine>> {
    let ocr_engine: Option<Arc<dyn OcrEngine>> = default_ocr_engine().map(Arc::from);
    if let Some(engine) = &ocr_engine {
        info!(
            engine = engine.name(),
            "OCR 引擎可用（图片 / 扫描 PDF 文字识别）"
        );
    } else {
        warn!(
            "无可用 OCR 引擎（Windows.Media.Ocr / Tesseract）——JPG/PNG 图片不入索引、\
             无文本层的扫描 PDF 将计 failed 并留痕 index_failures 表"
        );
    }
    if !PopplerPdfRasterizer::detect() {
        warn!(
            "未检测到 pdftoppm（poppler-utils）——扫描版 PDF 无法渲染页、\
             将计 failed 并留痕 index_failures 表"
        );
    }
    ocr_engine
}

/// 单 collection 首次全量索引：music + document + **图片 OCR** 三轮增量 + **语义向量
/// pass**（BETA-40 收尾——此前只有前两轮：JPG/PNG 不入索引、`document_vectors` 恒空）。
/// `embed_images` 由 `ServerConfig` 注入（daemon 默认 true，企业场景图片证据检索；
/// BETA-39 双层质量门槛沿用）。embed 前先按开关跑 `purge_short_body_vectors`——
/// 镜像桌面启动期语义：关 → 清全部图片向量回到一刀切态、开 → 仅清不过门槛的。
/// 完成后打统计日志（含 `index_failures` 留痕条数），归还两个 index 句柄。
#[allow(clippy::too_many_arguments)]
async fn run_initial_collection_index(
    meta: &locifind_server::collections::CollectionConfig,
    music_index: MusicIndex,
    document_index: DocumentIndex,
    ocr: Option<Arc<dyn OcrEngine>>,
    embedder: Arc<dyn TextEmbedder>,
    semantic_ready: bool,
    embed_images: bool,
) -> Result<(MusicIndex, DocumentIndex)> {
    let roots = meta.roots.clone();
    let (music_index, document_index, music_stats, document_stats, image_stats, embed_stats) =
        tokio::task::spawn_blocking(move || -> Result<_, IndexError> {
            let m = music_index.index_dirs_with_progress(&roots, &NoopProgress)?;
            let d = document_index.index_dirs_with_progress(&roots, &NoopProgress)?;
            let i = if let Some(engine) = &ocr {
                document_index.index_image_dirs_excluding_with_progress(
                    &roots,
                    engine.as_ref(),
                    &GlobSet::empty(),
                    &NoopProgress,
                )?
            } else {
                IndexStats::default()
            };
            let e = if semantic_ready {
                document_index.purge_short_body_vectors(embed_images)?;
                document_index.embed_pending(
                    &roots,
                    embedder.as_ref(),
                    embed_images,
                    &mut |_, _| {},
                )?
            } else {
                (0, 0, 0)
            };
            Ok((music_index, document_index, m, d, i, e))
        })
        .await
        .context("indexer 任务 panic 或被取消")??;

    let extraction_failures = document_index.extraction_failure_count().unwrap_or(0);
    info!(
        collection = %meta.id,
        music_scanned = music_stats.scanned,
        music_added = music_stats.added,
        document_scanned = document_stats.scanned,
        document_added = document_stats.added,
        image_scanned = image_stats.scanned,
        image_added = image_stats.added,
        image_failed = image_stats.failed,
        embedded = embed_stats.0,
        embed_reused = embed_stats.1,
        embed_failed = embed_stats.2,
        extraction_failures,
        "collection 首次全量索引完成"
    );
    Ok((music_index, document_index))
}

/// 加载 embedder：调 [`ModelDaemon::load_blocking`]（model-runtime 自动按
/// feature 选 stub / llama-cpp），包成 [`DaemonEmbedder`] 暴露给 indexer。
fn load_embedder(model_path: &Path) -> Result<Arc<dyn TextEmbedder>> {
    let params = ModelLoadParams {
        gpu_layers: 99,
        context_size: 2048,
    };
    let daemon = ModelDaemon::load_blocking(model_path, params)
        .map_err(|e| anyhow!("加载 embedder 模型失败：{e}"))?;
    let model_id = derive_model_id(model_path);
    Ok(Arc::new(DaemonEmbedder {
        daemon: Arc::new(daemon),
        model_id,
    }))
}

/// 从 GGUF 文件名派生 `model_id`（写入 `document_vectors.embed_model`）。
fn derive_model_id(model_path: &Path) -> String {
    model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map_or_else(|| "unknown-embedder".to_owned(), str::to_owned)
}

/// [`ModelDaemon`] → [`TextEmbedder`] 适配器。
struct DaemonEmbedder {
    daemon: Arc<ModelDaemon>,
    model_id: String,
}

impl std::fmt::Debug for DaemonEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonEmbedder")
            .field("model_id", &self.model_id)
            .finish_non_exhaustive()
    }
}

impl TextEmbedder for DaemonEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError> {
        // reviewer I-1：用 Io variant + `<embedder>` 占位 path（详 BETA-32 注）。
        self.daemon.embed(text).map_err(|e| IndexError::Io {
            path: "<embedder>".to_owned(),
            detail: e.to_string(),
        })
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn base_cli() -> Cli {
        Cli::parse_from([
            "locifindd",
            "--data-dir",
            "/tmp/d",
            "--model-path",
            "/tmp/m.gguf",
        ])
    }

    #[test]
    fn resolve_access_rejects_missing_both_modes() {
        let cli = base_cli();
        assert!(
            resolve_access_config(&cli).is_err(),
            "无 --config 也无 --root/--token 应报错"
        );
    }

    #[test]
    fn resolve_access_rejects_mixed_modes() {
        let mut cli = base_cli();
        cli.config = Some("/tmp/c.toml".into());
        cli.root = Some("/tmp/r".into());
        cli.token = Some("t".repeat(32));
        assert!(
            resolve_access_config(&cli).is_err(),
            "--config 与 --root/--token 互斥"
        );
    }

    #[test]
    fn resolve_access_legacy_synthesizes_default() {
        let mut cli = base_cli();
        cli.root = Some("/tmp/r".into());
        cli.token = Some("t".repeat(32));
        let cfg = resolve_access_config(&cli).unwrap();
        assert_eq!(cfg.collections.len(), 1);
        assert_eq!(cfg.collections[0].id, "default");
        assert!(cfg.tokens[0].admin);
    }

    #[test]
    fn resolve_access_legacy_short_token_rejected() {
        let mut cli = base_cli();
        cli.root = Some("/tmp/r".into());
        cli.token = Some("short".into());
        assert!(resolve_access_config(&cli).is_err());
    }
}
