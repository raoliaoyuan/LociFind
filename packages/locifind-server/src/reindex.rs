//! `/admin/reindex` 后台逻辑：per-collection `IN_FLIGHT` guard + 真增量 reindex。
//!
//! BETA-32 T7 落了全局 guard + stub；BETA-36 把粒度拆到 collection（`?collection=<id>`
//! 指名重建单个集合、省略时顺序跑全部**非只读**集合；`read_only=true` 的集合被显式
//! 指名 → [`ReindexError::ReadOnly`]（409，冻结语义冲突））；**BETA-36 follow-up
//! （2026-07-03）接真 indexer**：
//!
//! - **增量而非 atomic swap 全量重建**（对 BETA-32 spec §5.3 的实现修订）：
//!   复用 `index_dirs_with_progress`（mtime skip + 磁盘已删记录回收），与桌面
//!   `perform_reindex` 同款语义。放弃 rename swap 的原因：① Windows 上 rename 被
//!   `CollectionRuntime` 持有的 rusqlite 打开句柄挡住（需先换出连接、时序复杂）；
//!   ② 增量已覆盖"新增 / 修改 / 删除"全部日常场景。schema 变更级的全量重建仍走
//!   daemon 重启 + `--allow-rebuild-schema`（preflight 残留检查因此保留）。
//! - 完成后写回 per-collection `state.doc_count` / `indexed_at`。
//!
//! **PRIVACY CONTRACT**：`Internal(String)` 可能含本机绝对路径；按 spec §6.2 隐私
//! 硬规则、**不允许放 HTTP response body**，仅可进 tracing log。handler 只返
//! status code 不带 error body 是合规的。

use std::sync::Arc;
use std::time::Instant;

use anyhow::Context;
use thiserror::Error;

use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::{GlobSet, IndexStats, NoopProgress};

use crate::admin::ReindexResp;
use crate::config::{CollectionRuntime, ServerCtx};

/// reindex 触发期间可能产生的错误。
#[derive(Debug, Error)]
pub enum ReindexError {
    /// 指名的 collection 不存在（handler 映射 404）。
    #[error("collection 不存在：{0}")]
    UnknownCollection(String),
    /// 指名的 collection 是只读态冷冻归档（handler 映射 409）。
    #[error("collection 为只读态（冷冻归档），拒绝 reindex：{0}")]
    ReadOnly(String),
    /// 该 collection 已有 reindex 在进行中（`IN_FLIGHT` guard 命中），handler 映射 409。
    #[error("已有 reindex 在进行中：{0}")]
    InFlight(String),
    /// 内部错误（indexer 失败 / atomic swap 失败等），handler 映射 500。
    #[error("索引失败：{0}")]
    Internal(String),
}

/// 触发一次 reindex。
///
/// - `collection = Some(id)`：只重建该集合；不存在 → [`ReindexError::UnknownCollection`]、
///   只读 → [`ReindexError::ReadOnly`]、已在跑 → [`ReindexError::InFlight`]。
/// - `collection = None`：顺序重建全部**非只读**集合（只读集合静默跳过——冷冻归档
///   不参与全量重建是预期语义）；任一集合在跑 → InFlight（保持整体互斥简单性）。
///
/// # Errors
///
/// 见 [`ReindexError`] 各 variant。
pub async fn trigger_reindex(
    ctx: Arc<ServerCtx>,
    collection: Option<&str>,
) -> Result<ReindexResp, ReindexError> {
    let targets: Vec<&CollectionRuntime> = match collection {
        Some(id) => {
            let rt = ctx
                .collection(id)
                .ok_or_else(|| ReindexError::UnknownCollection(id.to_string()))?;
            if rt.meta.read_only {
                return Err(ReindexError::ReadOnly(id.to_string()));
            }
            vec![rt]
        }
        None => ctx
            .collections
            .values()
            .filter(|rt| !rt.meta.read_only)
            .collect(),
    };

    // 抢全部目标的 in_flight 标志（任一已在跑 → InFlight，不部分执行）。
    for rt in &targets {
        if rt.state.read().reindex_in_flight {
            return Err(ReindexError::InFlight(rt.meta.id.clone()));
        }
    }
    let mut guards = Vec::with_capacity(targets.len());
    for rt in &targets {
        rt.state.write().reindex_in_flight = true;
        // guard 接管 flag 复位职责（提前 return / panic / await 取消都能清干净）。
        guards.push(ReindexGuard {
            state: rt.state.clone(),
        });
    }

    let started = Instant::now();
    let mut total_doc_count: u64 = 0;
    let mut reindexed: Vec<String> = Vec::with_capacity(targets.len());
    for rt in &targets {
        // 真增量 reindex；错误链先落 tracing（可能含路径）、HTTP 侧只透 status code。
        let n = run_collection_reindex(rt, ctx.embedder.clone(), ctx.config.embed_images)
            .await
            .map_err(|e| {
                tracing::error!(collection = %rt.meta.id, error = ?e, "collection reindex 失败");
                ReindexError::Internal(e.to_string())
            })?;
        // 写回 per-collection 状态（list_collections 的 doc_count / indexed_at 数据源）。
        {
            let mut st = rt.state.write();
            st.doc_count = n;
            st.indexed_at = Some(chrono::Utc::now());
        }
        total_doc_count = total_doc_count.saturating_add(n);
        reindexed.push(rt.meta.id.clone());
    }

    Ok(ReindexResp {
        status: "completed",
        collections: reindexed,
        doc_count: total_doc_count,
        duration_ms: started.elapsed().as_millis(),
    })
}

/// RAII guard：drop 时把对应 collection 的 `reindex_in_flight` 复位为 false。
struct ReindexGuard {
    state: Arc<parking_lot::RwLock<crate::config::CollectionState>>,
}

impl Drop for ReindexGuard {
    fn drop(&mut self) {
        self.state.write().reindex_in_flight = false;
    }
}

/// 单 collection 真增量 reindex：`music` + `document` + **图片 OCR** 三轮增量
/// （mtime skip + 回收）+ **语义向量 pass**（BETA-40 收尾——此前 daemon 只有前两轮：
/// JPG/PNG 不入索引、`document_vectors` 恒空 → 语义臂名不副实），返回该集合当前
/// 索引总数（music + documents，含图片）。
///
/// - **图片轮**：per-call 现场探测 OCR 引擎——admin 装好依赖后无需重启 daemon、
///   下次 reindex 即生效（镜像桌面 onboarding「自动重检」精神）；不可用 → warn 跳过。
/// - **embed pass**：embedder ping 通过才跑（stub 构建自动跳过）；`embed_images`
///   由 `ServerConfig` 注入（daemon 默认 true——企业场景图片证据检索 + 2 字 CJK
///   词语义臂唯一兜底；BETA-39 双层质量门槛沿用，`--disable-image-semantics` 关闭）。
///
/// indexer 是 sync（rusqlite + rayon），放 [`tokio::task::spawn_blocking`] 跑；
/// `Mutex` 长持有到本轮写完——期间 MCP search 走 `LocalIndexBackend`（独立连接、
/// 只读）不受阻，`list_collections` 读 state 也不经过这两把锁。
async fn run_collection_reindex(
    rt: &CollectionRuntime,
    embedder: Arc<dyn TextEmbedder>,
    embed_images: bool,
) -> anyhow::Result<u64> {
    let music = rt.music_index.clone();
    let document = rt.document_index.clone();
    let roots = rt.meta.roots.clone();
    let id = rt.meta.id.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
        let music = music.lock();
        let m_stats = music.index_dirs_with_progress(&roots, &NoopProgress)?;
        let music_count = music.count()?;
        drop(music);
        let document = document.lock();
        let d_stats = document.index_dirs_with_progress(&roots, &NoopProgress)?;
        let i_stats = if let Some(ocr) = locifind_indexer::default_ocr_engine() {
            document.index_image_dirs_excluding_with_progress(
                &roots,
                ocr.as_ref(),
                &GlobSet::empty(),
                &NoopProgress,
            )?
        } else {
            tracing::warn!(
                collection = %id,
                "无可用 OCR 引擎（Windows.Media.Ocr / Tesseract），本轮跳过图片索引"
            );
            IndexStats::default()
        };
        let (embed_new, embed_reused, embed_failed) = if embedder.embed("ping").is_ok() {
            document.embed_pending(&roots, embedder.as_ref(), embed_images, &mut |_, _| {})?
        } else {
            (0, 0, 0)
        };
        let document_count = document.count()?;
        let extraction_failures = document.extraction_failure_count().unwrap_or(0);
        drop(document);
        tracing::info!(
            collection = %id,
            music_scanned = m_stats.scanned,
            document_scanned = d_stats.scanned,
            document_added = d_stats.added,
            document_updated = d_stats.updated,
            document_removed = d_stats.removed,
            document_failed = d_stats.failed,
            image_scanned = i_stats.scanned,
            image_added = i_stats.added,
            image_failed = i_stats.failed,
            embedded = embed_new,
            embed_reused,
            embed_failed,
            extraction_failures,
            "collection 增量 reindex 完成"
        );
        Ok(music_count.saturating_add(document_count))
    })
    .await
    .context("reindex 任务 panic 或被取消")?
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::test_support::{build_test_ctx_inmem, build_test_ctx_multi_inmem};

    /// 第二次并发触发应直接拿到 `InFlight` 错误。
    #[tokio::test]
    async fn concurrent_reindex_second_returns_in_flight() {
        let ctx = build_test_ctx_inmem();
        // 模拟"已有 reindex 在跑"（default collection）。
        ctx.collections["default"].state.write().reindex_in_flight = true;
        let err = trigger_reindex(ctx, None).await.unwrap_err();
        assert!(
            matches!(err, ReindexError::InFlight(_)),
            "并发触发应返 InFlight，实得：{err:?}"
        );
    }

    /// guard drop 后应自动复位 flag。
    #[tokio::test]
    async fn guard_clears_flag_on_drop() {
        let ctx = build_test_ctx_inmem();
        let state = ctx.collections["default"].state.clone();
        {
            let _g = ReindexGuard {
                state: state.clone(),
            };
            state.write().reindex_in_flight = true;
        }
        assert!(
            !state.read().reindex_in_flight,
            "guard drop 后 reindex_in_flight 应被复位为 false"
        );
    }

    /// happy path：成功 reindex 后 guard 自动 drop（flag 清）+ state 写回。
    ///
    /// inmem ctx 的 root 路径不存在：WalkDir 空扫（0 文件）、in-memory db 计数 0——
    /// 覆盖"真跑但空结果"路径；带 corpus 的真盘路径由 e2e 覆盖。
    #[tokio::test]
    async fn trigger_reindex_clears_flag_and_writes_back_state() {
        let ctx = build_test_ctx_inmem();
        let resp = trigger_reindex(ctx.clone(), None).await.unwrap();
        assert_eq!(resp.status, "completed");
        assert_eq!(resp.collections, vec!["default"]);
        assert_eq!(resp.doc_count, 0, "root 不存在 → 空扫 0 doc");
        assert!(!ctx.collections["default"].state.read().reindex_in_flight);
        assert!(
            ctx.collections["default"].state.read().indexed_at.is_some(),
            "真实 reindex 完成后应写回 indexed_at"
        );
        assert_eq!(ctx.collections["default"].state.read().doc_count, 0);
    }

    /// 指名不存在的 collection → `UnknownCollection`。
    #[tokio::test]
    async fn unknown_collection_rejected() {
        let ctx = build_test_ctx_inmem();
        let err = trigger_reindex(ctx, Some("nonexistent")).await.unwrap_err();
        assert!(matches!(err, ReindexError::UnknownCollection(_)));
    }

    /// 指名只读 collection → ReadOnly；省略时只读集合被静默跳过。
    #[tokio::test]
    async fn read_only_collection_rejected_when_named_skipped_when_all() {
        let ctx = build_test_ctx_multi_inmem();
        // test_support multi builder：case-a 只读、case-b 读写。
        let err = trigger_reindex(ctx.clone(), Some("case-a"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ReindexError::ReadOnly(_)),
            "指名只读集合应返 ReadOnly，实得：{err:?}"
        );

        let resp = trigger_reindex(ctx, None).await.unwrap();
        assert_eq!(
            resp.collections,
            vec!["case-b"],
            "省略 collection 时只读集合应被跳过"
        );
    }

    /// 指名读写 collection → 只重建它。
    #[tokio::test]
    async fn named_collection_reindexes_only_that_one() {
        let ctx = build_test_ctx_multi_inmem();
        let resp = trigger_reindex(ctx, Some("case-b")).await.unwrap();
        assert_eq!(resp.collections, vec!["case-b"]);
    }
}
