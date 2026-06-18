import { useStore } from "../store";

/** Center tab strip. SSH/RDP/VNC/Logs/Runbook/SFTP tabs live here. */
export function TabBar() {
  const tabs = useStore((s) => s.tabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const setActiveTab = useStore((s) => s.setActiveTab);
  const closeTab = useStore((s) => s.closeTab);

  if (tabs.length === 0) return <div className="tabbar" />;

  return (
    <div className="tabbar">
      {tabs.map((t) => (
        <div
          key={t.id}
          className={`tab${activeTabId === t.id ? " active" : ""}`}
          onClick={() => setActiveTab(t.id)}
        >
          <span className="tab-kind">{t.kind}</span>
          <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{t.title}</span>
          <span
            className="tab-close"
            onClick={(e) => { e.stopPropagation(); closeTab(t.id); }}
          >
            ✕
          </span>
        </div>
      ))}
    </div>
  );
}
