import type { AppData } from "../state/useAppData";
import { api, errorMessage } from "../lib/api";
import { formatBytes, formatDuration, timeAgo } from "../lib/format";
import { Icon } from "../components/Icon";
import { EmptyState, Panel, Stat } from "../components/ui";

export function RunningPage({ data }: { data: AppData }) {
  const { running, busyIds } = data;
  const totalMemory = running.reduce((total, item) => total + item.memoryBytes, 0);
  const totalProcesses = running.reduce((total, item) => total + item.processCount, 0);
  const totalCpu = running.reduce((total, item) => total + item.cpuPercent, 0);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Running browsers</h1>
          <p className="page-subtitle">
            One isolated browser per profile. Stopping a profile asks Chromium to shut down cleanly so
            cookies, storage and history are flushed before the process exits.
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn" onClick={() => void data.refreshRunning()}>
            <Icon name="refresh" size={14} /> Refresh
          </button>
          <button
            className="btn btn-danger"
            disabled={running.length === 0}
            onClick={() => void data.stopAll()}
          >
            <Icon name="stop" size={14} /> Stop all
          </button>
        </div>
      </div>

      <div className="page-body">
        <div className="stat-row" style={{ marginTop: 16 }}>
          <Stat label="Active profiles" value={running.length} note="Independent browser processes" />
          <Stat label="Memory" value={formatBytes(totalMemory)} note="All Chromium helper processes" />
          <Stat label="Processes" value={totalProcesses} note="Browser + renderers + GPU" />
          <Stat label="CPU" value={`${totalCpu.toFixed(0)}%`} note="Sampled every 3 seconds" />
        </div>

        {running.length === 0 ? (
          <EmptyState
            icon="activity"
            title="Nothing is running"
            description="Launch a profile from the Profiles page. Each one opens in its own window with its own storage and network identity."
          />
        ) : (
          <div className="table-wrap" style={{ marginTop: 0 }}>
            <table>
              <thead>
                <tr>
                  <th>Profile</th>
                  <th>PID</th>
                  <th>Memory</th>
                  <th>CPU</th>
                  <th>Proxy</th>
                  <th>Debug port</th>
                  <th>Uptime</th>
                  <th style={{ width: 110 }} />
                </tr>
              </thead>
              <tbody>
                {running.map((instance) => {
                  const busy = busyIds.includes(instance.profileId);
                  return (
                    <tr key={instance.profileId}>
                      <td>
                        <div className="row gap-4">
                          <span className="status-dot running" />
                          <div className="col" style={{ gap: 0 }}>
                            <span style={{ fontWeight: 550 }}>{instance.profileName}</span>
                            <span className="text-faint" style={{ fontSize: 11 }}>
                              started {timeAgo(instance.startedAt)}
                            </span>
                          </div>
                        </div>
                      </td>
                      <td className="mono text-dim">
                        {instance.pid}
                        <span className="text-faint"> · {instance.processCount} proc</span>
                      </td>
                      <td className="num text-dim">{formatBytes(instance.memoryBytes)}</td>
                      <td className="num text-dim">{instance.cpuPercent.toFixed(0)}%</td>
                      <td className="text-dim truncate">{instance.proxyLabel ?? "Direct"}</td>
                      <td className="mono text-dim">
                        {instance.cdpPort}
                        {instance.relayPort ? (
                          <span className="text-faint"> / relay {instance.relayPort}</span>
                        ) : null}
                      </td>
                      <td className="num text-dim">{formatDuration(instance.uptimeSecs)}</td>
                      <td>
                        <div className="row-actions">
                          <button
                            className="icon-button"
                            aria-label="Open data folder"
                            onClick={() =>
                              api
                                .openPath(instance.browserDir, false)
                                .catch((error) => data.toast("error", errorMessage(error)))
                            }
                          >
                            <Icon name="folder" size={14} />
                          </button>
                          <button
                            className="icon-button"
                            aria-label="Stop profile"
                            disabled={busy}
                            onClick={() => void data.stop(instance.profileId)}
                          >
                            {busy ? <span className="spinner" /> : <Icon name="stop" size={14} />}
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        <Panel title="Resource limits" description="Manage how many profiles may run at once.">
          <div className="row wrap gap-12">
            <div className="grow">
              <div className="text-dim" style={{ fontSize: 12, lineHeight: 1.6 }}>
                Currently {running.length} profile(s) running, {formatBytes(totalMemory)} of memory in
                use. The concurrency limit lives in Settings and is unlimited by default. There is no
                hard cap on how many profiles you can create.
              </div>
            </div>
            <div className="col" style={{ alignItems: "flex-end", gap: 2 }}>
              <span className="text-faint" style={{ fontSize: 11 }}>
                Limit
              </span>
              <strong className="num">
                {data.settings?.maxConcurrent === 0 || data.settings === null
                  ? "Unlimited"
                  : data.settings.maxConcurrent}
              </strong>
            </div>
          </div>
        </Panel>
      </div>
    </div>
  );
}
