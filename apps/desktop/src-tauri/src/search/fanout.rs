//! BETA-04/18/19 fan-out 多源搜索与跨范畴均衡分支（从 search.rs 拆出，逻辑零改动）。

use std::sync::Arc;
use std::time::Instant;

use locifind_harness::context::{ContextMemory, RefineMergeError};
use locifind_harness::{
    run_fanout_merge_rrf, run_fanout_merge_with_fallback, IntentRouter, MergedResult,
    SearchableTool,
};
use locifind_intent_parser::fallback::IntentSource;
use locifind_search_backend::{CancellationToken, SearchIntent};
use tauri::ipc::Channel;

use super::{
    describe_intent, emit_synonym_events, result_to_json, signals_to_labels, ResolvedQuery,
    SearchDeps, SearchEvent,
};

/// fanout backends 是否含语义召回臂（决定走加权 RRF 还是原 fallback 合并）。
fn fanout_has_semantic(backends: &[std::sync::Arc<dyn SearchableTool>]) -> bool {
    backends.iter().any(|t| {
        t.capability().backend_kind == Some(locifind_search_backend::BackendKind::SemanticIndex)
    })
}

/// BETA-04 fan-out 多源搜索分支：同时查 `backends`（系统搜索 + 本地索引），结果经
/// Result Normalizer 合并去重后流式投递。与 fallback 链路并列——纯文件名 / 单后端仍走 fallback。
/// `SearchResultJson.sources` 携带各结果的全部命中来源（供 UI 显示「via …」）。
pub(crate) async fn run_fanout_search(
    backends: Vec<Arc<dyn SearchableTool>>,
    query: ResolvedQuery<'_>,
    on_event: Channel<SearchEvent>,
    deps: &SearchDeps,
    start: Instant,
) -> Result<(), String> {
    let ResolvedQuery {
        effective,
        expanded,
        source,
        signals,
        raw_query,
    } = query;
    // 多源 fan-out：tracer 仍按首个后端记录（ToolCallEvent/ToolResultEvent 的 tool_id 是单 id
    // 语义）；但前端 Started 事件展示**全部并列源**，避免「via search.local」误导——实际并列查了
    // 如 local + windows + everything 多个后端。
    let backend_ids: Vec<String> = backends.iter().map(|b| b.id().to_owned()).collect();
    let first_id = backend_ids[0].clone();
    let sources_label = backend_ids.join(" + ");
    let tool_start = Instant::now();

    deps.tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: first_id.clone(),
        tool_kind: locifind_harness::ToolKind::Search,
        intent_variant: locifind_harness::SupportedIntent::from_intent(&effective),
    });
    let _ = on_event.send(SearchEvent::Started {
        intent_summary: describe_intent(&effective),
        fallback_used: matches!(source, IntentSource::Model),
        signals: signals_to_labels(signals),
        tool_id: sources_label.clone(),
        intent_json: crate::search::intent_to_json(&effective),
    });
    emit_synonym_events(&expanded, &deps.tracer);

    let cancel = CancellationToken::new();
    // BETA-15B-1：含语义臂时走加权 RRF 融合（语义臂 + FTS 内容臂跨列表按 path 累加加权倒数排名），
    // 不接文件名兜底（语义/FTS 内容后端覆盖语义召回，无需 Everything 文件名补召回）。
    // 不含语义臂 → 沿用原 fan-out + 文件名兜底路径（字节级零行为变化）。
    let has_semantic = fanout_has_semantic(&backends);
    // 收齐合并结果（fan-out 本就先收齐各源再合并，无流式损失）→ Ranker 排序 → 按序发出。
    let mut merged: Vec<MergedResult> = Vec::new();
    let outcome = if has_semantic {
        let merged_ref = &mut merged;
        let mut on_result = |m: MergedResult| merged_ref.push(m);
        let semantic_weight = deps.semantic_weight();
        run_fanout_merge_rrf(
            &backends,
            &expanded,
            cancel,
            &mut on_result,
            semantic_weight,
            raw_query,
        )
        .await
    } else {
        // 文件名兜底候选（Everything）：内容 fan-out（仅 content-capable）零结果时按文件名补召回，
        // 闭合「文件在系统索引/本地索引未覆盖的位置、但文件名含关键词」的盲区。
        // macOS 无纯文件名后端 → 空 → 兜底永不触发（零行为变化）。
        let filename_fallback =
            IntentRouter::new(deps.registry()).route_filename_fallback(&expanded);
        let merged_ref = &mut merged;
        let on_event_ref = &on_event;
        let tracer_ref = &deps.tracer;
        let from_id = backends[0].id().to_owned();
        let to_id = filename_fallback.first().map(|t| t.id().to_owned());
        let mut on_result = |m: MergedResult| merged_ref.push(m);
        let mut on_fallback = move || {
            // 内容源全空 → 切到 Everything 文件名兜底：发 BackendSwitched 供 UI 提示 + trace 记一条。
            if let Some(to) = to_id.as_deref() {
                tracer_ref.on_error(&locifind_harness::ToolErrorEvent {
                    tool_id: from_id.clone(),
                    duration: tool_start.elapsed(),
                    error_type: "fanout_filename_fallback".to_owned(),
                });
                let _ = on_event_ref.send(SearchEvent::BackendSwitched {
                    from: from_id.clone(),
                    to: to.to_owned(),
                    reason: "empty".to_owned(),
                });
            }
        };
        run_fanout_merge_with_fallback(
            &backends,
            &filename_fallback,
            &expanded,
            cancel,
            &mut on_result,
            &mut on_fallback,
        )
        .await
    };

    deps.tracer
        .on_tool_result(&locifind_harness::ToolResultEvent {
            tool_id: first_id,
            duration: tool_start.elapsed(),
            result_count: outcome.total,
        });

    if outcome.total == 0 {
        // BETA-33 cycle 9：语义臂错误不冒充全链错误——报错优先取非语义臂错误；仅语义臂
        // 出错（如路由后模型加载竞态失败）时按「未找到结果」空态呈现：其余臂已正常查完、
        // 语义能力的真实状态在顶栏 / 设置页 EmbedStatus 另有如实展示。
        let semantic_ids: Vec<&str> = backends
            .iter()
            .filter(|t| {
                t.capability().backend_kind
                    == Some(locifind_search_backend::BackendKind::SemanticIndex)
            })
            .map(|t| t.id())
            .collect();
        let message = outcome
            .errors
            .iter()
            .find(|(id, _)| !semantic_ids.contains(&id.as_str()))
            .map(|(_, m)| m.clone())
            .unwrap_or_else(|| "未找到结果".to_owned());
        let _ = on_event.send(SearchEvent::Error { message });
        return Ok(());
    }

    // BETA-05：相关性启发式排序（默认）/ 显式 sort 跨源生效，写入 score。
    let ranked = locifind_ranker::rank(
        merged,
        &locifind_ranker::RankContext::from_expanded(&expanded),
    );

    let mut recorded: Vec<locifind_search_backend::SearchResult> = Vec::with_capacity(ranked.len());
    let mut send_failed = false;
    for m in ranked {
        let sources: Vec<String> = m
            .sources
            .iter()
            .map(|s| format!("{s:?}").to_lowercase())
            .collect();
        let sem_cos = m.semantic_cosine;
        let json = result_to_json(&m.result, sources, sem_cos);
        recorded.push(m.result);
        if !send_failed && on_event.send(SearchEvent::Result { item: json }).is_err() {
            send_failed = true;
        }
    }
    let _ = send_failed;

    {
        let mut guard = deps.context.lock().unwrap_or_else(|e| e.into_inner());
        guard.record(expanded.base, recorded);
    }
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        path = "fanout",
        total = outcome.total,
        elapsed_ms,
        "search 出口"
    );
    let _ = on_event.send(SearchEvent::Complete {
        total: outcome.total,
        elapsed_ms,
    });
    Ok(())
}

/// BETA-19：`FileSearch` 是否含 ≥2 个**不同** `file_type`（跨范畴多类型 → 需均衡展示）。
/// 返回去重保序的类型列表；单类型 / 非 `FileSearch` / 无 file_type → `None`（不触发，走原路径）。
pub(crate) fn multi_file_types(
    intent: &SearchIntent,
) -> Option<Vec<locifind_search_backend::FileType>> {
    let SearchIntent::FileSearch(fs) = intent else {
        return None;
    };
    let types = fs.file_type.as_ref()?;
    let mut uniq: Vec<locifind_search_backend::FileType> = Vec::new();
    for t in types {
        if !uniq.contains(t) {
            uniq.push(*t);
        }
    }
    (uniq.len() >= 2).then_some(uniq)
}

/// BETA-19：把多类型 expanded 收窄为**单类型**子查询——`file_type` 置为单值，`extensions`
/// 并集按该类型切回子集（保留用户显式收窄，如「png 和 mp4」只查 png/mp4）；交集空 → `None`
/// 让后端按 `file_type` 派生。`keyword_groups` 原样保留（同义词与类型无关）。
pub(crate) fn single_type_expanded(
    expanded: &locifind_search_backend::ExpandedSearchIntent,
    file_type: locifind_search_backend::FileType,
) -> locifind_search_backend::ExpandedSearchIntent {
    let mut sub = expanded.clone();
    if let SearchIntent::FileSearch(fs) = &mut sub.base {
        fs.file_type = Some(vec![file_type]);
        if let Some(exts) = &fs.extensions {
            let allowed = locifind_search_backend::extensions_for_file_type(file_type);
            let narrowed: Vec<String> = exts
                .iter()
                .filter(|e| {
                    let lo = e.to_lowercase();
                    allowed.iter().any(|a| *a == lo)
                })
                .cloned()
                .collect();
            fs.extensions = if narrowed.is_empty() {
                None
            } else {
                Some(narrowed)
            };
        }
    }
    sub
}

/// BETA-19 均衡分支的逐类型执行计划：(单类型子查询, content 后端, 文件名兜底后端)。
pub(crate) type TypePlan = (
    locifind_search_backend::ExpandedSearchIntent,
    Vec<Arc<dyn SearchableTool>>,
    Vec<Arc<dyn SearchableTool>>,
);

/// BETA-19 跨范畴均衡分支：对每个 `file_type` 各跑一遍自己的路由+执行（各得一份配额），
/// 桶内按 [`locifind_ranker::rank`] 排序，再 round-robin 交错——保证少数派类型在结果前列可见。
/// 复用现有 fan-out 机制（每个单类型子查询用它自己的 `route_search_fanout` + `route_filename_fallback`）。
pub(crate) async fn run_balanced_multitype_search(
    types: Vec<locifind_search_backend::FileType>,
    query: ResolvedQuery<'_>,
    on_event: Channel<SearchEvent>,
    deps: &SearchDeps,
    start: Instant,
) -> Result<(), String> {
    let ResolvedQuery {
        effective,
        expanded,
        source,
        signals,
        raw_query: _,
    } = query;

    // 1) 逐类型路由：单类型子查询 → (sub, content 后端, 文件名兜底)。无后端的类型跳过。
    let router = IntentRouter::new(deps.registry());
    let mut plans: Vec<TypePlan> = Vec::with_capacity(types.len());
    for t in &types {
        let sub = single_type_expanded(&expanded, *t);
        if let Ok(backends) = router.route_search_fanout(&sub) {
            let fallback = router.route_filename_fallback(&sub);
            plans.push((sub, backends, fallback));
        }
    }
    if plans.is_empty() {
        let _ = on_event.send(SearchEvent::Error {
            message: "未找到结果".to_owned(),
        });
        return Ok(());
    }

    // 跨范畴均衡同样并列多后端：tracer 记首个；前端展示全部并列源（去重，按 id 序）。
    let backend_ids: Vec<String> = {
        use std::collections::BTreeSet;
        plans
            .iter()
            .flat_map(|p| p.1.iter().map(|t| t.id().to_owned()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let first_id = plans[0].1[0].id().to_owned();
    let sources_label = backend_ids.join(" + ");
    let tool_start = Instant::now();
    deps.tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: first_id.clone(),
        tool_kind: locifind_harness::ToolKind::Search,
        intent_variant: locifind_harness::SupportedIntent::from_intent(&effective),
    });
    let _ = on_event.send(SearchEvent::Started {
        intent_summary: describe_intent(&effective),
        fallback_used: matches!(source, IntentSource::Model),
        signals: signals_to_labels(signals),
        tool_id: sources_label.clone(),
        intent_json: crate::search::intent_to_json(&effective),
    });
    emit_synonym_events(&expanded, &deps.tracer);

    // 2) 逐类型收桶 + 桶内排序。各类型独立 cancel 派生自同一根。
    let cancel = CancellationToken::new();
    let mut buckets: Vec<Vec<MergedResult>> = Vec::with_capacity(plans.len());
    for (sub, backends, fallback) in &plans {
        let mut bucket: Vec<MergedResult> = Vec::new();
        {
            let mut on_result = |m: MergedResult| bucket.push(m);
            // 均衡分支不对每类型单独发 BackendSwitched（噪声大）；兜底仍按需触发补召回。
            let mut on_fallback = || {};
            let _ = run_fanout_merge_with_fallback(
                backends,
                fallback,
                sub,
                cancel.clone(),
                &mut on_result,
                &mut on_fallback,
            )
            .await;
        }
        let ranked =
            locifind_ranker::rank(bucket, &locifind_ranker::RankContext::from_expanded(sub));
        buckets.push(ranked);
    }

    // 3) round-robin 交错 + 显式 limit 截断（总数 ≤ L 语义；默认不截断）。
    let mut results = locifind_ranker::interleave(buckets);
    if let SearchIntent::FileSearch(fs) = &expanded.base {
        if let Some(limit) = fs.limit {
            results.truncate(limit as usize);
        }
    }
    let total = results.len();

    deps.tracer
        .on_tool_result(&locifind_harness::ToolResultEvent {
            tool_id: first_id,
            duration: tool_start.elapsed(),
            result_count: total,
        });

    if total == 0 {
        let _ = on_event.send(SearchEvent::Error {
            message: "未找到结果".to_owned(),
        });
        return Ok(());
    }

    // 4) 流式投递 + 记上下文（记原多类型 intent 供 refine）。
    let mut recorded: Vec<locifind_search_backend::SearchResult> = Vec::with_capacity(total);
    let mut send_failed = false;
    for m in results {
        let sources: Vec<String> = m
            .sources
            .iter()
            .map(|s| format!("{s:?}").to_lowercase())
            .collect();
        let sem_cos = m.semantic_cosine;
        let json = result_to_json(&m.result, sources, sem_cos);
        recorded.push(m.result);
        if !send_failed && on_event.send(SearchEvent::Result { item: json }).is_err() {
            send_failed = true;
        }
    }
    let _ = send_failed;

    {
        let mut guard = deps.context.lock().unwrap_or_else(|e| e.into_inner());
        guard.record(expanded.base, recorded);
    }
    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        path = "balanced-multitype",
        total,
        elapsed_ms,
        "search 出口"
    );
    let _ = on_event.send(SearchEvent::Complete { total, elapsed_ms });
    Ok(())
}

/// 若 intent 是 `Refine`，按 [`ContextMemory`] 合并上一轮基准；否则原样返回。
///
/// `conflicts`（clear 与 delta 同名字段冲突，已按 clear 为准合并）走 `eprintln`
/// 记录，不阻断。合并失败（无上一轮 / 基准非法 / 字段不适用）返回
/// [`RefineMergeError`]，由调用方转 [`SearchEvent::Error`]。
pub(crate) fn apply_refine_if_needed(
    intent: SearchIntent,
    ctx: &ContextMemory,
) -> Result<SearchIntent, RefineMergeError> {
    match intent {
        SearchIntent::Refine(refine) => {
            let outcome = ctx.apply_refine(&refine)?;
            if !outcome.conflicts.is_empty() {
                eprintln!(
                    "search: refine 字段冲突(以 clear 为准): {:?}",
                    outcome.conflicts
                );
            }
            Ok(outcome.intent)
        }
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use locifind_harness::{SearchTool, SupportedIntent};
    use locifind_search_backend::{
        BackendKind, BackendSearchFuture, BackendStream, ImplementationStatus, SearchBackend,
        SearchError, SearchResult,
    };

    /// 仅用于路由判定：`kind()` 可设的空结果后端（capability().backend_kind 取自它）。
    #[derive(Debug)]
    struct KindBackend(BackendKind);
    impl SearchBackend for KindBackend {
        fn kind(&self) -> BackendKind {
            self.0
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn is_available(&self) -> bool {
            true
        }
        fn search<'a>(
            &'a self,
            _intent: &'a SearchIntent,
            _cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            Box::pin(async move {
                let items: Vec<Result<SearchResult, SearchError>> = Vec::new();
                Ok(Box::pin(futures::stream::iter(items)) as BackendStream)
            })
        }
    }

    fn tool(kind: BackendKind) -> Arc<dyn SearchableTool> {
        Arc::new(SearchTool::new(
            "search.fake",
            "Fake",
            KindBackend(kind),
            vec![SupportedIntent::FileSearch],
            "kind-only fake backend",
        ))
    }

    /// 仅本地索引臂 → false；含语义臂 → true（决定加权 RRF vs 原 fallback 合并的路由谓词）。
    #[test]
    fn fanout_has_semantic_detects_semantic_arm() {
        let local_only = vec![tool(BackendKind::NativeIndex)];
        assert!(
            !fanout_has_semantic(&local_only),
            "仅 NativeIndex 本地索引臂应判为无语义"
        );

        let with_semantic = vec![
            tool(BackendKind::NativeIndex),
            tool(BackendKind::SemanticIndex),
        ];
        assert!(
            fanout_has_semantic(&with_semantic),
            "含 SemanticIndex 臂应判为有语义"
        );

        assert!(
            !fanout_has_semantic(&[]),
            "空 backends 应判为无语义（兜底安全）"
        );
    }
}
