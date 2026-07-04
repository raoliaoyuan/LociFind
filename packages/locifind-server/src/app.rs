//! `axum::Router` 工厂 —— 把 MCP transport 与 admin REST 装配成一个 Router。
//!
//! BETA-32 T8：daemon binary（T9 / T10）与未来嵌入式都通过 [`build_app`] 拿到同一份
//! axum app；本 mod 只负责拼装路由 + 套 bearer middleware：
//!
//! - `GET /health` —— 无鉴权（spec §5.1）
//! - `POST /admin/reindex` —— bearer 鉴权
//! - `POST /mcp` —— bearer 鉴权（rmcp 1.8 streamable-HTTP transport，`nest_service` 进来）
//!
//! **streamable-HTTP 装配**：rmcp 1.8 用 `StreamableHttpService::new(factory, session_mgr, config)`
//! 返一个实现了 `tower::Service` 的服务（不是 `axum::Router`），通过
//! `axum::Router::new().nest_service("/mcp", service)` 挂载。`factory` 必须返一个
//! 实现了 `ServerHandler` 的实例，每个会话调用一次；BETA-32 daemon 是有状态长生命
//! 周期 ctx，把 [`LocifindMcpHandler`] 包 `Arc` 后 clone 给 factory 闭包，工厂每次
//! 返一份 `Arc<LocifindMcpHandler>`（rmcp 1.8 为 `Arc<T: ServerHandler>` 提供了
//! blanket `Service` 适配，见 `handler/server.rs::impl_server_handler_for_wrapper!`）。

use std::sync::Arc;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

use crate::admin::{admin_audit, admin_audit_report, admin_reindex, health};
use crate::auth::require_bearer;
use crate::config::ServerCtx;
use crate::mcp::LocifindMcpHandler;

/// 组装 axum `Router`。
///
/// 公共子树：`/health` 不鉴权；
/// 保护子树：`/admin/*` 与 `/mcp` 套 bearer middleware。
pub fn build_app(ctx: Arc<ServerCtx>) -> Router {
    let auth_ctx = ctx.auth_ctx();

    // ---- /mcp：rmcp 1.8 streamable-HTTP transport ----
    let mcp_handler = Arc::new(LocifindMcpHandler::new(ctx.clone()));
    let session_mgr = Arc::new(LocalSessionManager::default());
    // rmcp 1.8 streamable-HTTP 配置（参考 `~/.cargo/registry/.../rmcp-1.8.*/src/transport/
    // streamable_http_server/tower.rs:1055-1280`）：
    // - `stateful_mode=false`：BETA-32 follow-up ⑤ 切 stateless（2026-06-29）。原 stateful
    //   保 session state 节省 client 重复 initialize / tools/list；但 stateless 模式下
    //   响应走纯 JSON framing 简化 e2e client、且 LLM client（Claude Code / Codex / Cline）
    //   普遍能 cache initialize / tools/list 跨 request，stateless 实际客户体验损失小。
    //   trade-off：每个连接 client 需自带 protocol-version header；session-id header 不再发。
    // - `json_response=true`：stateless 模式下生效（tower.rs：stateless 分支用 JSON framing、
    //   stateful 分支强制 SSE）。`Content-Type: application/json` 响应、单 request 单 body、
    //   无 SSE event stream framing。
    // - `sse_keep_alive=None`：显式禁用 SSE keep-alive ping（stateless 模式纯 JSON 也无 SSE）。
    let mcp_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None);
    let mcp_service: StreamableHttpService<Arc<LocifindMcpHandler>, LocalSessionManager> =
        StreamableHttpService::new(
            {
                let handler = mcp_handler.clone();
                move || Ok(handler.clone())
            },
            session_mgr,
            mcp_config,
        );

    let mcp_router: Router<Arc<ServerCtx>> = Router::new()
        .nest_service("/mcp", mcp_service)
        .route_layer(from_fn_with_state(auth_ctx.clone(), require_bearer));

    // ---- /admin/*：bearer 保护（handler 内另有 admin 标志门 → 403）----
    let admin_router: Router<Arc<ServerCtx>> = Router::new()
        .route("/admin/reindex", post(admin_reindex))
        .route("/admin/audit", get(admin_audit))
        .route("/admin/audit/report", get(admin_audit_report))
        .route_layer(from_fn_with_state(auth_ctx, require_bearer));

    // ---- /health：无鉴权 ----
    let public: Router<Arc<ServerCtx>> = Router::new().route("/health", get(health));

    Router::new()
        .merge(public)
        .merge(admin_router)
        .merge(mcp_router)
        .with_state(ctx)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::test_support::build_test_ctx_inmem;
    use axum::body::Body;
    use axum::http::{header::AUTHORIZATION, Request, StatusCode};
    use tower::ServiceExt;

    /// `/health` 不需要 Authorization header（spec §5.1 探活端点）。
    #[tokio::test]
    async fn health_no_auth_required() {
        let ctx = build_test_ctx_inmem();
        let app = build_app(ctx);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// `/admin/reindex` 无 Authorization → 401。
    #[tokio::test]
    async fn admin_reindex_requires_auth() {
        let ctx = build_test_ctx_inmem();
        let app = build_app(ctx);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/reindex")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// `/admin/reindex` 带正确 token → 200（stub `run_full_reindex` 返 `doc_count=0`）。
    #[tokio::test]
    async fn admin_reindex_with_correct_token_succeeds() {
        // build_test_ctx_inmem 用 `SecretString::from("test-token")`，对应 Bearer test-token。
        let ctx = build_test_ctx_inmem();
        let app = build_app(ctx);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/reindex")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// `/mcp` 无 Authorization → 401。spec §6.2：MCP endpoint 全程鉴权；
    /// regression test 锁住 `route_layer` 透过 `nest_service` 套用的行为
    /// （reviewer Important #2）。
    #[tokio::test]
    async fn mcp_endpoint_requires_auth() {
        let ctx = build_test_ctx_inmem();
        let app = build_app(ctx);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
