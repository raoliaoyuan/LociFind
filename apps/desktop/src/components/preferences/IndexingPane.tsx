import { useEffect, useRef, useState } from "react";
import { AppSettings } from "../../hooks/useAppSettings";
import {
  ExtractionFailure,
  IndexStatus,
  RootIndexOverview,
  formatIndexTime,
} from "./shared";

/**
 * BETA-33 cycle 5：单个索引 root 行。
 * `overview` = null 时统计显示"…"（尚未加载）。`onRemove` = null 时不显示移除按钮
 * （系统默认目录用户不能"移除"、只能通过"+ 添加目录"覆盖）。
 *
 * cycle 7-a：
 * - `isPending`：picker 加入但未保存的自定义 root，显示 `⏳ 待应用` 琥珀 badge。
 * - `flash`：picker 后 1.5s CSS flash 高亮 + scrollIntoView（消除"选了没反应"错觉）。
 */
function RootRow({
  path,
  isSystemDefault,
  overview,
  onRemove,
  isPending,
  flash,
  excludePatterns,
  onUpdateExcludes,
  onOpenDir,
  onRescan,
  rescanDisabled,
}: {
  path: string;
  isSystemDefault: boolean;
  overview: RootIndexOverview | null;
  onRemove: (() => void) | null;
  isPending?: boolean;
  flash?: boolean;
  /** cycle 7-b：该 root 的 per-root 子路径 exclude patterns（默认空）。 */
  excludePatterns?: string[];
  /** cycle 7-b：更新 patterns 回调；null = 只读（例如 fallback、无 root_excludes wiring）。 */
  onUpdateExcludes?: ((patterns: string[]) => void) | null;
  /** cycle 7-c：在系统文件管理器中打开该目录。 */
  onOpenDir?: () => void;
  /** cycle 7-c：单目录重扫；null = 不显示（如待应用的 pending root，排除配置尚未保存）。 */
  onRescan?: (() => void) | null;
  /** cycle 7-c：重扫按钮禁用（全局索引中）。 */
  rescanDisabled?: boolean;
}) {
  const stats = overview
    ? [
        `文档 ${overview.doc_count.toLocaleString()}`,
        `图片 ${overview.image_count.toLocaleString()}`,
        `音乐 ${overview.music_count.toLocaleString()}`,
      ].join(" · ")
    : "…";
  const lastIndexed = overview?.last_indexed_time
    ? formatIndexTime(overview.last_indexed_time)
    : null;
  const rowRef = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [patternDraft, setPatternDraft] = useState("");
  useEffect(() => {
    if (flash) {
      rowRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [flash]);
  const cls = [
    "prefs-root-row",
    isPending ? "pending" : "",
    flash ? "flash" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const patterns = excludePatterns ?? [];
  const excludeEditable = onUpdateExcludes != null;
  const addPattern = () => {
    const t = patternDraft.trim();
    if (!t || !onUpdateExcludes) return;
    if (!patterns.includes(t)) {
      onUpdateExcludes([...patterns, t]);
    }
    setPatternDraft("");
  };
  return (
    <>
      {/* 2026-07-06（cycle 9 真机反馈二轮）：三行卡片式布局——单行 flex 会把路径列
          挤到极窄逐字断行。行 1 完整路径（独占整宽）、行 2 索引内容统计、行 3 操作按钮。 */}
      <div className={cls} ref={rowRef}>
        <div className="prefs-root-line">
          <span
            className={`prefs-root-path${isSystemDefault ? " sys" : ""}`}
            title={path}
          >
            📂 {path}
          </span>
          {isSystemDefault && <span className="prefs-root-tag">系统默认</span>}
          {isPending && (
            <span className="prefs-root-tag pending" title="picker 加入但未保存">
              ⏳ 待应用
            </span>
          )}
        </div>
        <div className="prefs-root-line">
          <span
            className="prefs-root-stats"
            title="该目录下索引条数（文档 · 图片 · 音乐）"
          >
            {stats}
          </span>
          {lastIndexed && (
            <span
              className="prefs-root-time"
              title={`上次索引：${overview?.last_indexed_time ?? ""}`}
            >
              上次索引 {lastIndexed}
            </span>
          )}
        </div>
        <div className="prefs-root-line prefs-root-actions">
          {excludeEditable && (
            <button
              type="button"
              className={`prefs-btn small${patterns.length > 0 ? " has-excludes" : ""}`}
              onClick={() => setExpanded(!expanded)}
              title="配置该目录下的子路径排除（通配符）"
            >
              {expanded ? "▾" : "▸"} 子路径排除
              {patterns.length > 0 ? ` (${patterns.length})` : ""}
            </button>
          )}
          {onOpenDir && (
            <button
              type="button"
              className="prefs-btn small"
              onClick={onOpenDir}
              title="在系统文件管理器中打开该目录"
            >
              打开
            </button>
          )}
          {onRescan && (
            <button
              type="button"
              className="prefs-btn small"
              onClick={onRescan}
              disabled={rescanDisabled}
              title="只重扫该目录（排除规则仍生效，不影响其他目录）"
            >
              重扫
            </button>
          )}
          {onRemove && (
            <button type="button" className="prefs-btn small" onClick={onRemove}>
              移除
            </button>
          )}
        </div>
      </div>
      {excludeEditable && expanded && (
        <div className="prefs-root-excludes">
          <p className="prefs-hint">
            相对该目录的通配符：<code>**</code>=任意层，<code>*</code>=单段，
            <code>?</code>=单字符。示例：<code>临时/**</code>、
            <code>**/backup/**</code>、<code>*.old/*</code>。
          </p>
          {patterns.map((p, i) => (
            <div key={i} className="prefs-exclude-row">
              <code>{p}</code>
              <button
                type="button"
                className="prefs-btn small"
                onClick={() => {
                  if (!onUpdateExcludes) return;
                  onUpdateExcludes(patterns.filter((_, j) => j !== i));
                }}
              >
                移除
              </button>
            </div>
          ))}
          <div className="prefs-exclude-add-row">
            <input
              type="text"
              className="prefs-input"
              value={patternDraft}
              onChange={(e) => setPatternDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") addPattern();
              }}
              placeholder="如 临时/** 或 **/backup/**"
            />
            <button
              type="button"
              className="prefs-btn"
              onClick={addPattern}
              disabled={!patternDraft.trim()}
            >
              添加
            </button>
          </div>
        </div>
      )}
    </>
  );
}

export function IndexingPane({
  settings,
  setSettings,
  initialIndexRoots,
  effectiveRoots,
  indexOverview,
  indexStatus,
  indexStatusLine,
  extractionFailures,
  semanticLine,
  reindexing,
  reindexMsg,
  onReindex,
  onReindexRoot,
  onOpenRoot,
  onRequestRemoveRoot,
  onPickMessage,
  flashPath,
  onFlash,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  initialIndexRoots: string[];
  effectiveRoots: string[] | null;
  indexOverview: RootIndexOverview[] | null;
  indexStatus: IndexStatus | null;
  indexStatusLine: string;
  /** BETA-40：文件级提取失败留痕（null = 加载中）。 */
  extractionFailures: ExtractionFailure[] | null;
  semanticLine: string | null;
  reindexing: boolean;
  reindexMsg: string;
  onReindex: () => void;
  /** cycle 7-c：单目录重扫。 */
  onReindexRoot: (path: string) => void;
  /** cycle 7-c：文件管理器打开目录。 */
  onOpenRoot: (path: string) => void;
  /** cycle 7-c：移除目录（父组件弹二次确认、可选 purge）。 */
  onRequestRemoveRoot: (path: string) => void;
  onPickMessage: (m: string) => void;
  flashPath: string | null;
  onFlash: (path: string) => void;
}) {
  const [excludeDraft, setExcludeDraft] = useState("");
  // BETA-40：「未能索引的文件」清单折叠态（默认收起，仅显示条数）。
  const [failuresExpanded, setFailuresExpanded] = useState(false);

  const addExclude = () => {
    const t = excludeDraft.trim();
    if (!t) return;
    if (!settings.exclude_globs.includes(t)) {
      setSettings({
        ...settings,
        exclude_globs: [...settings.exclude_globs, t],
      });
    }
    setExcludeDraft("");
  };

  // 按 path 找对应统计（overview 里的顺序 = effectiveRoots 顺序、但按 path 匹配更稳）。
  const overviewOf = (path: string): RootIndexOverview | null =>
    indexOverview?.find((o) => o.path === path) ?? null;

  // 顶部总览合计（跨所有 root）。
  const totalDocs = indexOverview?.reduce((s, o) => s + o.doc_count, 0) ?? 0;
  const totalImages =
    indexOverview?.reduce((s, o) => s + o.image_count, 0) ?? 0;
  const totalMusic = indexOverview?.reduce((s, o) => s + o.music_count, 0) ?? 0;
  const grandTotal = totalDocs + totalImages + totalMusic;
  // cycle 7-a：数据源统一（Codex APPROVED 2 · 选 a）——概貌"上次索引"用 indexOverview.max()、
  // 与「本地索引」区文案一致；避免出现"顶部 Downloads-only 数字 vs 底部全库数字"两套口径。
  const latestTime = indexOverview
    ?.map((o) => o.last_indexed_time)
    .filter((t): t is string => !!t)
    .sort()
    .pop();

  // cycle 9：口径统一明示——概貌是「当前生效目录内」口径、「本地索引」行 last_summary 是
  // 「全库」口径，两者可合法不一致（「仅移除」目录保留的记录 / override 前旧默认目录的
  // 记录仍在库且仍可被搜索命中）。全库 > 概貌合计时显式提示差值来源，不放任两个数字
  // 各说各话。反向（概貌 > 全库，生效目录相互嵌套导致重复计数）不提示、属已知统计特性。
  const dbGrand = indexStatus?.db_totals
    ? indexStatus.db_totals[0] + indexStatus.db_totals[1] + indexStatus.db_totals[2]
    : null;
  const outsideRootsCount =
    dbGrand !== null && indexOverview !== null && dbGrand > grandTotal
      ? dbGrand - grandTotal
      : 0;

  // cycle 7-a：pending 集合——settings.index_roots 里但不在 initialIndexRoots 里 = picker 加入未保存。
  const pendingSet = new Set(
    settings.index_roots.filter((p) => !initialIndexRoots.includes(p)),
  );

  // cycle 7-b：查某 root 对应的 excludePatterns。后端按 normalize_root_key 归一化匹配、
  // 但前端保留 display 形式（跟 settings.index_roots 字符串一致）；简单按等值匹配。
  const excludesFor = (rootPath: string): string[] => {
    return (
      settings.root_excludes.find((re) => re.root === rootPath)?.patterns ?? []
    );
  };
  const updateExcludesFor = (rootPath: string, patterns: string[]) => {
    const others = settings.root_excludes.filter((re) => re.root !== rootPath);
    if (patterns.length === 0) {
      // 空 patterns → 从 root_excludes 里删（避免存空条目）
      setSettings({ ...settings, root_excludes: others });
    } else {
      setSettings({
        ...settings,
        root_excludes: [...others, { root: rootPath, patterns }],
      });
    }
  };
  // cycle 7-c：移除 root 走父组件的二次确认弹窗（onRequestRemoveRoot），
  // 确认后由父组件同步删 root_excludes 条目（不留孤儿）+ 可选 purge 索引记录。

  return (
    <div className="prefs-form">
      {/* BETA-33 cycle 5：顶部概貌卡片——总目录 / 分类分总 / 上次索引 */}
      <div className="prefs-overview-card">
        <div className="prefs-overview-title">索引概貌</div>
        {indexOverview === null ? (
          <p className="prefs-hint">加载中…</p>
        ) : indexOverview.length === 0 ? (
          <p className="prefs-hint err">
            ⚠️ 无生效索引目录（未添加 + 系统未检测到默认音乐/文档/图片目录）。
          </p>
        ) : (
          <div className="prefs-overview-stats">
            <div
              className="prefs-overview-cell"
              title="设置里生效的目录数（含系统默认追加）"
            >
              <div className="prefs-overview-num">{indexOverview.length}</div>
              <div className="prefs-overview-label">生效目录</div>
            </div>
            <div
              className="prefs-overview-cell"
              title="当前生效目录内的条数合计（全库口径见下方「本地索引」行）"
            >
              <div className="prefs-overview-num">{grandTotal.toLocaleString()}</div>
              <div className="prefs-overview-label">总条数</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalDocs.toLocaleString()}</div>
              <div className="prefs-overview-label">文档</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalImages.toLocaleString()}</div>
              <div className="prefs-overview-label">图片</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalMusic.toLocaleString()}</div>
              <div className="prefs-overview-label">音乐</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num prefs-overview-time">
                {latestTime ? formatIndexTime(latestTime) : "尚未"}
              </div>
              <div className="prefs-overview-label">上次索引</div>
            </div>
          </div>
        )}
        {/* cycle 9：全库 vs 概貌口径差显式提示（差值来源 + 清理路径），替代两个数字各说各话。 */}
        {outsideRootsCount > 0 && (
          <p className="prefs-hint" style={{ marginTop: "8px" }}>
            ℹ️ 库内另有 <strong>{outsideRootsCount.toLocaleString()}</strong>{" "}
            条记录在当前生效目录之外（来自已移除的目录或旧配置），搜索仍会命中它们。
            如需清理：移除目录时选「移除并清除索引记录」，或在隐私页清空索引后重建。
          </p>
        )}
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          索引目录（生效 {effectiveRoots?.length ?? 0} 个 = 自定义{" "}
          {settings.index_roots.length} +{" "}
          {effectiveRoots
            ? Math.max(0, effectiveRoots.length - settings.index_roots.length)
            : 0}{" "}
          系统默认）
        </label>
        {/* 2026-07-06 新语义：checkbox 常显——系统三夹纳入与否完全由它决定（默认不勾 =
            不索引系统目录）；旧「覆盖语义」banner 随之退役（勾选状态自解释）。 */}
        <label className="prefs-checkbox prefs-checkbox-strong">
          <input
            type="checkbox"
            checked={settings.include_system_defaults}
            onChange={(e) =>
              setSettings({
                ...settings,
                include_system_defaults: e.target.checked,
              })
            }
          />
          <strong>同时索引系统默认目录（音乐 / 文档 / 图片）</strong>
        </label>
        {/* cycle 6 v4：统一按 effectiveRoots 渲染，自定义项显示「移除」、系统默认项显示 tag。
            cycle 7-a：pending 集合传 RootRow 显示琥珀 badge；flashPath 命中的行加 CSS flash 高亮。 */}
        {effectiveRoots?.map((path, i) => {
          const isCustom = settings.index_roots.includes(path);
          const isPending = pendingSet.has(path);
          return (
            <RootRow
              key={`${isCustom ? "usr" : "sys"}-${i}`}
              path={path}
              isSystemDefault={!isCustom}
              overview={overviewOf(path)}
              isPending={isPending}
              flash={flashPath === path}
              excludePatterns={excludesFor(path)}
              onUpdateExcludes={(patterns) => updateExcludesFor(path, patterns)}
              onOpenDir={() => onOpenRoot(path)}
              // pending root 的排除配置尚未保存、重扫口径会与预期不符 → 不给重扫入口。
              onRescan={isPending ? null : () => onReindexRoot(path)}
              rescanDisabled={reindexing || (indexStatus?.indexing ?? false)}
              onRemove={isCustom ? () => onRequestRemoveRoot(path) : null}
            />
          );
        })}
        {effectiveRoots && effectiveRoots.length === 0 && (
          <p className="prefs-hint err">
            ⚠️
            尚未选择任何索引目录——默认不索引、搜索不会有本地索引结果。请「+
            添加目录」，或勾选上方系统默认目录。
          </p>
        )}
        <button
          type="button"
          className="prefs-btn"
          onClick={async () => {
            const { open } = await import("@tauri-apps/plugin-dialog");
            const picked = await open({ directory: true, multiple: false });
            if (typeof picked === "string") {
              if (settings.index_roots.includes(picked)) {
                // cycle 7-a：已在列表也 flash 一下让用户知道"没重复添加、但确实是这条"
                onFlash(picked);
                onPickMessage("该目录已在列表中");
              } else {
                setSettings({
                  ...settings,
                  index_roots: [...settings.index_roots, picked],
                });
                onFlash(picked);
                onPickMessage(
                  "已加入下方列表 · 未保存 —— 点「应用」或「确定」生效",
                );
              }
            }
          }}
        >
          + 添加目录
        </button>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          排除目录名（通配符，留空 = 默认排除 node_modules/.git 等）
        </label>
        {settings.exclude_globs.map((g, i) => (
          <div key={i} className="prefs-root-row">
            <span className="prefs-root-path">{g}</span>
            <button
              type="button"
              className="prefs-btn small"
              onClick={() =>
                setSettings({
                  ...settings,
                  exclude_globs: settings.exclude_globs.filter(
                    (_, j) => j !== i,
                  ),
                })
              }
            >
              移除
            </button>
          </div>
        ))}
        <div style={{ display: "flex", gap: "8px" }}>
          <input
            type="text"
            className="prefs-input"
            value={excludeDraft}
            onChange={(e) => setExcludeDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") addExclude();
            }}
            placeholder="如 node_modules 或 *cache*"
          />
          <button type="button" className="prefs-btn" onClick={addExclude}>
            添加
          </button>
        </div>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">本地索引</label>
        <p className="prefs-hint">
          建立音乐 metadata 与文档内容的本地索引；应用启动时会在后台自动索引。
        </p>
        <label className="prefs-label" htmlFor="auto-index-interval">
          自动增量索引
        </label>
        <p className="prefs-hint">
          定期检查新增与变动的文件（未变化的文件不会重新索引）。
        </p>
        <select
          id="auto-index-interval"
          className="prefs-input"
          value={settings.auto_index_interval_minutes}
          onChange={(e) =>
            setSettings({
              ...settings,
              auto_index_interval_minutes: Number(e.target.value),
            })
          }
        >
          <option value={0}>关闭</option>
          <option value={15}>15 分钟</option>
          <option value={30}>30 分钟</option>
          <option value={60}>60 分钟</option>
        </select>
        {/* BETA-39：图片语义索引 opt-in。默认关（防乱码 OCR 污染语义召回）；
            开启后图片文字走更严的质量门槛（0.75）入语义索引，需重新索引生效。 */}
        <label className="prefs-checkbox">
          <input
            type="checkbox"
            checked={settings.enable_image_semantics}
            onChange={(e) =>
              setSettings({
                ...settings,
                enable_image_semantics: e.target.checked,
              })
            }
          />
          <span>
            <strong>让图片文字参与语义搜索（实验性）</strong>
            <br />
            <span className="prefs-hint">
              默认关闭：图片 OCR 文字仅支持字面（关键词）匹配。开启后，通过更严格质量门槛的图片文字（如聊天截图、扫描笔记）也能被「按意思」搜到；乱码 OCR 会被自动挡下。
              <strong>需重新索引后生效。</strong>
            </span>
          </span>
        </label>
        {/* cycle 7-a：正在索引时显示 indeterminate 进度条（Codex OBJECT 3 · 不做百分比）
            + 阶段 chip + 当前目录 + 累计计数。文本行由 indexStatusLine 生成。 */}
        {indexStatus?.indexing && (
          <div className="prefs-progress-indeterminate" aria-hidden="true">
            <div className="prefs-progress-bar" />
          </div>
        )}
        <p className="prefs-status">{indexStatusLine}</p>
        {semanticLine && <p className="prefs-status">{semanticLine}</p>}
        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <button
            type="button"
            className="prefs-btn primary"
            onClick={onReindex}
            disabled={reindexing}
          >
            {reindexing ? "索引中…" : "立即索引"}
          </button>
          {reindexMsg && <span className="prefs-status">{reindexMsg}</span>}
        </div>
      </div>

      {/* BETA-40：文件级提取失败留痕——哪些文件没能进索引、为什么。成功重扫 /
          文件从磁盘删除后自动从清单消失。无失败时不渲染整节（不制造焦虑）。 */}
      {extractionFailures !== null && extractionFailures.length > 0 && (
        <div className="prefs-field">
          <label className="prefs-label">未能索引的文件</label>
          <p className="prefs-hint">
            以下文件在索引时提取失败（损坏 / 加密 / 缺依赖等），搜索不到它们的内容。
            修复原因后「立即索引」会自动重试；成功或文件删除后自动从此清单消失。
          </p>
          <button
            type="button"
            className="prefs-btn small"
            onClick={() => setFailuresExpanded((v) => !v)}
          >
            {failuresExpanded ? "▾" : "▸"} 共 {extractionFailures.length} 个文件
          </button>
          {failuresExpanded && (
            <div
              style={{
                maxHeight: "220px",
                overflowY: "auto",
                marginTop: "8px",
              }}
            >
              {extractionFailures.map((f, i) => (
                <div key={i} className="prefs-root-row" title={f.path}>
                  <span className="prefs-root-path">
                    {f.path.split(/[\\/]/).pop() ?? f.path}
                    <span className="prefs-hint">
                      {" — "}
                      {f.reason}
                      {f.failed_time
                        ? `（${formatIndexTime(f.failed_time)}）`
                        : ""}
                    </span>
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
