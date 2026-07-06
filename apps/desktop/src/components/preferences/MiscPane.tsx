/**
 * BETA-47：「杂项」面板。目前收纳不属于其他分类的功能入口；
 * 后续零散设置（如 tracing 开关等）优先落这里、不再挤「常规」。
 */
export function MiscPane({
  onNavigate,
}: {
  /** 跳转应用内页面（走关闭守卫，见 shell）。 */
  onNavigate: (path: string) => void;
}) {
  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">我的同义词</label>
        <p className="prefs-hint">
          自定义同义词词典：让「财报 = 财务报表 = financial report」这类你自己的叫法
          也能互相召回。支持导入 / 导出。
        </p>
        <button
          type="button"
          className="prefs-btn"
          onClick={() => onNavigate("/synonyms")}
        >
          打开「我的同义词」…
        </button>
      </div>
    </div>
  );
}
