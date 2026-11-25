import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "./Icon";

/** Custom window chrome: the whole bar drags the window, the buttons do not. */
export function TitleBar({
  running,
  browser,
}: {
  running: number;
  browser: string;
}) {
  const appWindow = getCurrentWindow();
  return (
    <header className="titlebar">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true" />
        Prism
        <span className="brand-sub">Profiles</span>
      </div>
      <div className="titlebar-spacer" />
      <div className="titlebar-status">
        {running > 0 ? (
          <>
            <span className="status-dot running" />
            {running} running
          </>
        ) : (
          <>
            <span className="status-dot idle" />
            idle
          </>
        )}
        <span style={{ opacity: 0.4 }}>·</span>
        <span className="truncate" style={{ maxWidth: 220 }} title={browser}>
          {browser}
        </span>
      </div>
      <div className="window-controls">
        <button onClick={() => void appWindow.minimize()} aria-label="Minimize">
          <Icon name="minimize" size={13} />
        </button>
        <button onClick={() => void appWindow.toggleMaximize()} aria-label="Maximize">
          <Icon name="maximize" size={12} />
        </button>
        <button className="close" onClick={() => void appWindow.close()} aria-label="Close">
          <Icon name="close" size={13} />
        </button>
      </div>
    </header>
  );
}
