//! Server 配置 + 共享上下文。
//!
//! BETA-32 T4：`ServerConfig` 承载启动参数（由 CLI / TOML / env 合并得出），
//! `ServerCtx` 是运行时依赖容器（注入到所有 tools / handlers）。
//!
//! BETA-36 重构：单根单钥升级为 **collection 模型**——`ServerConfig.access` 承载
//! `[[collections]]`/`[[tokens]]`/`[audit]`（TOML 或 legacy 合成，见
//! [`crate::collections`]）；`ServerCtx.collections` 每个归档集合一份
//! [`CollectionRuntime`]（独立 index.db + 候选链缓存 + 运行时状态——物理信息墙）。

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::level_filters::LevelFilter;

use parking_lot::{Mutex, RwLock};

use locifind_harness::SearchableTool;
use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::{DocumentIndex, MusicIndex};

use crate::auth::AuthCtx;
use crate::collections::{CollectionConfig, DaemonConfigFile, LEGACY_COLLECTION_ID};

/// 启动参数 — 由 CLI / TOML / env 合并填充。
#[derive(Debug)]
pub struct ServerConfig {
    /// HTTP 监听地址（含端口）。
    pub bind_addr: SocketAddr,
    /// 索引 / 缓存 / audit 等 server 数据目录。
    pub data_dir: PathBuf,
    /// 模型文件路径（embedding gguf 等）。
    pub model_path: PathBuf,
    /// 日志级别过滤。
    pub log_level: LevelFilter,
    /// hybrid RRF 融合中语义臂的权重（daemon 无 settings.json，由 CLI
    /// `--semantic-weight` 注入；默认镜像桌面
    /// `locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT`）。企业评测
    /// （BETA-40 收尾）用它 A/B 不同权重下 FTS 精确命中 vs 语义召回的排位。
    pub semantic_weight: f64,
    /// OCR 图片文本是否入语义索引（BETA-39 双层质量门槛沿用）。**daemon 默认
    /// true**——企业冷归档场景检索者不熟悉语料、图片证据（凭证/截图/现场照片）
    /// 是三场景共同需求，且 2 字 CJK 词 FTS 结构性不可达、语义臂是图片内容唯一
    /// 兜底（评估：docs/reviews/beta-40-enterprise-eval-2026-07-04.md §5）。
    /// 桌面端维持 opt-in 默认关，两侧策略独立。CLI `--disable-image-semantics`
    /// 关闭；启动期 purge 按本开关镜像桌面语义（关 → 清全部图片向量）。
    pub embed_images: bool,
    /// 已校验的 collections / tokens / audit 配置（TOML 解析或 legacy 合成）。
    pub access: DaemonConfigFile,
}

/// 一个 collection 的运行时状态（BETA-36：原全局 `RuntimeState` 按 collection 拆分）。
#[derive(Debug, Default)]
pub struct CollectionState {
    /// 最近一次 indexing 完成的时间。
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 当前索引内文档总数。
    pub doc_count: u64,
    /// reindex 是否在跑（用作互斥锁，防并发触发）。
    pub reindex_in_flight: bool,
}

/// 一个 collection 的运行时容器：元信息 + 独立 index.db + 候选链缓存 + 状态。
///
/// `music_index` / `document_index` 用 `Mutex` 包：内部含裸 `rusqlite::Connection`
/// （`!Sync`），加 `parking_lot::Mutex` 是让容器 `Sync` 成立的最小代价（rusqlite
/// 单连接自带 `SQLite` mutex、并发 query 本就要排队，业务语义无损）。
pub struct CollectionRuntime {
    /// 配置元信息（显示名 / 归档主体 / roots / 只读态 / 审计标签）。
    pub meta: CollectionConfig,
    /// 本 collection 的 index.db 路径（[`collection_db_path`] 布局）。
    pub db_path: PathBuf,
    /// 音乐索引句柄。
    pub music_index: Arc<Mutex<MusicIndex>>,
    /// 文档索引句柄。
    pub document_index: Arc<Mutex<DocumentIndex>>,
    /// search 候选链缓存：启动时构造一次、每次 invoke `Arc::clone` 复用
    /// （BETA-32 T6 #6 节奏不变，粒度从全局改 per-collection）。
    pub search_candidates: Arc<Vec<Arc<dyn SearchableTool>>>,
    /// `运行时状态（indexed_at` / `doc_count` / reindex 互斥）。
    pub state: Arc<RwLock<CollectionState>>,
}

impl std::fmt::Debug for CollectionRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectionRuntime")
            .field("meta", &self.meta)
            .field("db_path", &self.db_path)
            .field(
                "search_candidates",
                &format!("Arc<Vec<{}-tool(s)>>", self.search_candidates.len()),
            )
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// collection 的 index.db 布局：legacy `default` 沿用 `<data_dir>/index.db`
/// （现有部署零迁移），其余走 `<data_dir>/collections/<id>/index.db`。
///
/// id 字符集由 [`crate::collections::is_valid_collection_id`] 在配置校验时把守，
/// 拼路径无注入面。
#[must_use]
pub fn collection_db_path(data_dir: &Path, collection_id: &str) -> PathBuf {
    if collection_id == LEGACY_COLLECTION_ID {
        data_dir.join("index.db")
    } else {
        data_dir
            .join("collections")
            .join(collection_id)
            .join("index.db")
    }
}

/// 运行时依赖容器 — 注入到所有 tools / handlers。
///
/// 通过 `Arc<ServerCtx>` 在 axum State 中传递；含 `Arc<dyn TextEmbedder>` trait object，
/// 因此手动实现 `Debug`。
pub struct ServerCtx {
    /// 启动配置（含 access 权限模型）。
    pub config: ServerConfig,
    /// embedder（语义臂；stub 时 FTS-only 降级）。
    pub embedder: Arc<dyn TextEmbedder>,
    /// 全部 collection 运行时，key = collection id（`BTreeMap` 保证遍历顺序稳定）。
    pub collections: BTreeMap<String, CollectionRuntime>,
    /// 检索留痕 sink（`<data_dir>/audit.jsonl`，BETA-36 验收 ③④）。
    pub audit: Arc<crate::audit::AuditSink>,
}

impl std::fmt::Debug for ServerCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerCtx")
            .field("config", &self.config)
            .field("embedder_model_id", &self.embedder.model_id())
            .field("collections", &self.collections)
            .field("audit", &self.audit)
            .finish()
    }
}

impl ServerCtx {
    /// 从 `ServerCtx` 派生出窄一点的 [`AuthCtx`]，
    /// 用于装配 bearer 中间件 layer（避免给 middleware 注入完整索引）。
    #[must_use]
    pub fn auth_ctx(&self) -> Arc<AuthCtx> {
        Arc::new(AuthCtx::from_config_file(&self.config.access))
    }

    /// 按 id 查 collection 运行时。
    #[must_use]
    pub fn collection(&self, id: &str) -> Option<&CollectionRuntime> {
        self.collections.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_legacy_default_stays_flat() {
        let p = collection_db_path(Path::new("/data"), LEGACY_COLLECTION_ID);
        assert_eq!(p, Path::new("/data").join("index.db"));
    }

    #[test]
    fn db_path_named_collection_nested() {
        let p = collection_db_path(Path::new("/data"), "case-a");
        assert_eq!(
            p,
            Path::new("/data")
                .join("collections")
                .join("case-a")
                .join("index.db")
        );
    }
}
