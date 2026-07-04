//! 索引进度回调 — daemon 走 tracing log、桌面 app 默认走 no-op、
//! 测试代码可注入 spy 收集进度事件。
//!
//! BETA-32 C1a：为 headless MCP daemon 提供"边索引边汇报进度"通道。
//! 桌面 app 调旧 `index_dirs` / `index_dirs_excluding`（默认 [`NoopProgress`]、行为零变更），
//! daemon 调新 `_with_progress` 变体注入自定义 [`IndexProgress`]（典型走 tracing log）。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// BETA-33 cycle 7-a：索引阶段枚举。
/// `reindex_with_progress_inner` 每进入一个 phase 前调 `IndexProgress::on_phase`，
/// 桌面 UI 用它显示 chip：「🎵 扫描音乐」/「📄 扫描文档」/「🖼 扫描图片」等，
/// 避免用户看到"已扫描 0 · 已入库 0"卡住不动误判死机（例如 Everything 全盘发现阶段没有 per-file progress）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexPhase {
    /// Everything 全盘发现（`AudioDiscovery::discover_audio`），无 per-file 进度、秒级完成。
    MusicDiscovery,
    /// Everything 发现失败回退 walkdir 扫音乐目录，有 per-file 进度。
    MusicScan,
    /// 文档 phase（`DocumentIndex::index_dirs_excluding_with_progress`）。
    Doc,
    /// 图片 OCR phase（`DocumentIndex::index_image_dirs_excluding_with_progress`）。
    Image,
}

/// 索引进度回调 trait。线程安全，可被 indexer 内部跨线程调用。
pub trait IndexProgress: Send + Sync {
    /// 单文件索引完成（成功或跳过）。`indexed = true` 表示真插入/更新；
    /// `false` 表示 mtime 命中跳过或提取失败被计 failed。
    fn on_file(&self, path: &Path, mime: &str, indexed: bool);
    /// 整批索引完成（一次 `_with_progress` 调用尾部触发一次）。
    fn on_batch_done(&self, scanned: u64, indexed: u64);
    /// BETA-33 cycle 7-a：新阶段开始的通知。默认 no-op（旧实现零改动）。
    /// 桌面 `StatusProgressBridge` 覆写此方法把 phase 写回 `IndexStatus.current_phase`。
    fn on_phase(&self, _phase: IndexPhase) {}
}

/// 默认 no-op 实现。桌面 app 走这个、行为与改造前 100% 等价。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProgress;

impl IndexProgress for NoopProgress {
    fn on_file(&self, _: &Path, _: &str, _: bool) {}
    fn on_batch_done(&self, _: u64, _: u64) {}
}

/// 测试 spy：用 atomic 计文件 / 批次次数 + 记最近一次 [`IndexProgress::on_batch_done`] 的参数，
/// 防 indexer 把 `(scanned, indexed)` 传成 `(0, 0)` 或顺序写反等契约回归被测试静默放过。
#[derive(Debug, Default)]
pub struct SpyProgress {
    /// 累计 [`IndexProgress::on_file`] 调用次数。
    pub files: AtomicU64,
    /// 累计 [`IndexProgress::on_batch_done`] 调用次数。
    pub batches: AtomicU64,
    /// 最近一次 [`IndexProgress::on_batch_done`] 收到的 `scanned` 参数。
    pub last_scanned: AtomicU64,
    /// 最近一次 [`IndexProgress::on_batch_done`] 收到的 `indexed` 参数。
    pub last_indexed: AtomicU64,
    /// BETA-33 cycle 7-a：累计 [`IndexProgress::on_phase`] 调用次数（跨 phase 总和）。
    pub phase_calls: AtomicU64,
    /// BETA-33 cycle 7-a：最近一次 phase 的 discriminant（`u8`：0=MusicDiscovery / 1=MusicScan / 2=Doc / 3=Image）。
    /// 只是给单测用（enum 无 atomic 方便的表达）；生产 bridge 保 `Option<IndexPhase>`。
    pub last_phase_discriminant: AtomicU64,
}

impl IndexProgress for SpyProgress {
    fn on_file(&self, _: &Path, _: &str, _: bool) {
        self.files.fetch_add(1, Ordering::Relaxed);
    }
    fn on_batch_done(&self, scanned: u64, indexed: u64) {
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.last_scanned.store(scanned, Ordering::Relaxed);
        self.last_indexed.store(indexed, Ordering::Relaxed);
    }
    fn on_phase(&self, phase: IndexPhase) {
        self.phase_calls.fetch_add(1, Ordering::Relaxed);
        let d: u64 = match phase {
            IndexPhase::MusicDiscovery => 0,
            IndexPhase::MusicScan => 1,
            IndexPhase::Doc => 2,
            IndexPhase::Image => 3,
        };
        self.last_phase_discriminant.store(d, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn noop_progress_compiles_and_no_panics() {
        let p = NoopProgress;
        p.on_file(&PathBuf::from("/x"), "text/plain", true);
        p.on_batch_done(10, 10);
    }

    #[test]
    fn spy_progress_counts() {
        let s = SpyProgress::default();
        s.on_file(&PathBuf::from("/a"), "text/plain", true);
        s.on_file(&PathBuf::from("/b"), "text/plain", false);
        s.on_batch_done(2, 1);
        assert_eq!(s.files.load(Ordering::Relaxed), 2);
        assert_eq!(s.batches.load(Ordering::Relaxed), 1);
        assert_eq!(s.last_scanned.load(Ordering::Relaxed), 2);
        assert_eq!(s.last_indexed.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn spy_progress_distinguishes_scanned_and_indexed_argument_order() {
        // 防 indexer 把 `(scanned, indexed)` 顺序写反 — last_scanned / last_indexed 分别 store。
        let s = SpyProgress::default();
        s.on_batch_done(7, 3);
        assert_eq!(s.last_scanned.load(Ordering::Relaxed), 7);
        assert_eq!(s.last_indexed.load(Ordering::Relaxed), 3);
    }

    /// BETA-33 cycle 7-a：验证 `on_phase` 默认实现不 panic + `NoopProgress` 收到不动状态。
    #[test]
    fn noop_progress_on_phase_default_impl_is_no_op() {
        let p = NoopProgress;
        p.on_phase(IndexPhase::MusicDiscovery);
        p.on_phase(IndexPhase::Doc);
        // 无 assert：不 panic 即验证默认实现兜底。
    }

    /// BETA-33 cycle 7-a：SpyProgress 记录 phase 调用 + 最后 phase discriminant 正确。
    #[test]
    fn spy_progress_records_phase_calls() {
        let s = SpyProgress::default();
        s.on_phase(IndexPhase::MusicDiscovery);
        assert_eq!(s.phase_calls.load(Ordering::Relaxed), 1);
        assert_eq!(s.last_phase_discriminant.load(Ordering::Relaxed), 0);
        s.on_phase(IndexPhase::Doc);
        s.on_phase(IndexPhase::Image);
        assert_eq!(s.phase_calls.load(Ordering::Relaxed), 3);
        assert_eq!(s.last_phase_discriminant.load(Ordering::Relaxed), 3);
    }
}
