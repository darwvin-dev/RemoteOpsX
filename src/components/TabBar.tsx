import { useStore } from "../store";

/** Center tab strip. SSH/RDP/VNC/Logs/Runbook/SFTP tabs live here. */
export function TabBar() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const setActiveTab = useStore((s) => s.setActiveTab);
  const closeTab = useStore((s) => s.closeTab);

  if (tabs.length === 0) return null;

  return (
    <div className="tabbar" role="tablist" aria-label="Open sessions">
      {tabs.map((t) => (
        <div key={t.id} className={`tab-shell${activeTabId === t.id ? " active" : ""}`}>
          <button type="button" className="tab" onClick={() => setActiveTab(t.id)} role="tab"
            aria-selected={activeTabId === t.id} tabIndex={activeTabId === t.id ? 0 : -1}>
            <span className="tab-kind">{t.kind}</span>
            <span className="tab-title">{t.title}</span>
          </button>
          <button type="button" className="tab-close" aria-label={`Close ${t.title}`} onClick={() => closeTab(t.id)}>✕</button>
        </div>
      ))}
    </div>
  );
}
