import { formatBytes } from "../lib/format";
import { Icon, type IconName } from "./Icon";

export type NavId = "profiles" | "proxies" | "running" | "data" | "settings";

const items: { id: NavId; label: string; icon: IconName }[] = [
  { id: "profiles", label: "Profiles", icon: "profiles" },
  { id: "proxies", label: "Proxies", icon: "proxy" },
  { id: "running", label: "Running", icon: "activity" },
  { id: "data", label: "Backup & import", icon: "archive" },
];

export function Sidebar({
  active,
  onNavigate,
  profileCount,
  runningCount,
  proxyCount,
  storageBytes,
  version,
  maxConcurrent,
}: {
  active: NavId;
  onNavigate: (id: NavId) => void;
  profileCount: number;
  runningCount: number;
  proxyCount: number;
  storageBytes: number;
  version: string;
  maxConcurrent: number;
}) {
  const capacityLabel = maxConcurrent > 0 ? `${runningCount}/${maxConcurrent}` : `${runningCount}`;
  const capacity = maxConcurrent > 0 ? Math.min(100, (runningCount / maxConcurrent) * 100) : Math.min(100, runningCount * 8);

  return (
    <nav className="sidebar" aria-label="Main">
      <div className="sidebar-section">Workspace</div>
      {items.map((item) => {
        const badge =
          item.id === "profiles"
            ? profileCount
            : item.id === "proxies"
            ? proxyCount
            : item.id === "running"
            ? runningCount
            : undefined;
        return (
          <button
            key={item.id}
            className={`nav-item ${active === item.id ? "active" : ""}`}
            onClick={() => onNavigate(item.id)}
            title={item.label}
            aria-current={active === item.id ? "page" : undefined}
          >
            <Icon name={item.icon} size={17} />
            <span className="nav-label">{item.label}</span>
            {badge !== undefined ? <span className="nav-badge">{badge}</span> : null}
          </button>
        );
      })}

      <div className="sidebar-section">Preferences</div>
      <button
        className={`nav-item ${active === "settings" ? "active" : ""}`}
        onClick={() => onNavigate("settings")}
        title="Settings"
        aria-current={active === "settings" ? "page" : undefined}
      >
        <Icon name="settings" size={17} />
        <span className="nav-label">Settings</span>
      </button>

      <div className="sidebar-footer">
        <div className="row between">
          <strong className="footer-copy">Browser slots</strong>
          <span className="num" style={{ fontSize: 11.5 }}>
            {capacityLabel}
          </span>
        </div>
        <div className="meter">
          <span style={{ width: `${capacity}%` }} />
        </div>
        <div className="footer-copy row between" style={{ fontSize: 11 }}>
          <span>Profile data</span>
          <span className="num">{formatBytes(storageBytes)}</span>
        </div>
        <div className="footer-copy" style={{ fontSize: 10.5, opacity: 0.65 }}>
          Prism v{version}
        </div>
      </div>
    </nav>
  );
}
