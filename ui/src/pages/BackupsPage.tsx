import { useEffect, useState } from "react";
import type { AppData } from "../state/useAppData";
import type { BackupReport, HistoryEntry, ImportReport } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import { BACKUP_FILTER, CONFIG_FILTER, pickOpenPath, pickSavePath, stampedName } from "../lib/files";
import { formatBytes, formatDate } from "../lib/format";
import { Icon } from "../components/Icon";
import { EmptyState, Field, Panel, Stat } from "../components/ui";

export function BackupsPage({ data }: { data: AppData }) {
  const [exportPass, setExportPass] = useState("");
  const [importPass, setImportPass] = useState("");
  const [backupPass, setBackupPass] = useState("");
  const [restorePass, setRestorePass] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [lastExport, setLastExport] = useState<BackupReport | null>(null);
  const [lastImport, setLastImport] = useState<ImportReport | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);

  const loadHistory = () => {
    api
      .history(null, 40)
      .then(setHistory)
      .catch((error) => data.toast("error", errorMessage(error)));
  };

  useEffect(loadHistory, []);

  const run = async (label: string, action: () => Promise<void>) => {
    setBusy(label);
    try {
      await action();
    } catch (error) {
      data.toast("error", errorMessage(error));
    } finally {
      setBusy(null);
      void data.refreshInfo();
    }
  };

  const exportConfig = () =>
    run("export", async () => {
      const path = await pickSavePath(stampedName("prism-config", "prism"), [CONFIG_FILTER]);
      if (!path) return;
      const report = await api.exportConfig(path, [], exportPass.trim() || null);
      setLastExport(report);
      data.toast("ok", `Exported ${report.profiles} profile(s) to ${report.path}`);
    });

  const createBackup = () =>
    run("backup", async () => {
      const path = await pickSavePath(stampedName("prism-backup", "prism"), [BACKUP_FILTER]);
      if (!path) return;
      const report = await api.createBackup(path, backupPass.trim() || null);
      setLastExport(report);
      data.toast("ok", `Backup written: ${report.profiles} profile(s), ${formatBytes(report.bytes)}`);
    });

  const importConfigFile = () =>
    run("import", async () => {
      const path = await pickOpenPath([CONFIG_FILTER, BACKUP_FILTER]);
      if (!path) return;
      const report = await api.importConfig(path, importPass.trim() || null);
      setLastImport(report);
      await data.refreshAll();
      data.toast(
        report.errors.length ? "warn" : "ok",
        `Imported ${report.imported}, skipped ${report.skipped}${
          report.errors.length ? `, ${report.errors.length} problem(s)` : ""
        }`
      );
    });

  const restoreFile = () =>
    run("restore", async () => {
      const path = await pickOpenPath([BACKUP_FILTER, CONFIG_FILTER]);
      if (!path) return;
      const report = await api.restoreBackup(path, restorePass.trim() || null);
      setLastImport(report);
      await data.refreshAll();
      data.toast(
        report.errors.length ? "warn" : "ok",
        `Restored ${report.imported} profile(s) with their browser data`
      );
    });

  const openPath = (path: string) => {
    api.openPath(path, false).catch((error) => data.toast("error", errorMessage(error)));
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Backup &amp; import</h1>
          <p className="page-subtitle">
            Move profile configuration between machines, or take a complete backup that includes
            every profile's cookies and storage. Backups can be sealed with a passphrase (Argon2id +
            AES-256-GCM).
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn" onClick={() => void data.refreshAll()}>
            <Icon name="refresh" size={14} /> Refresh
          </button>
        </div>
      </div>

      <div className="page-body">
        <div className="stat-row" style={{ marginTop: 16 }}>
          <Stat label="Profiles" value={data.profiles.length} note="In this workspace" />
          <Stat label="Proxies" value={data.proxies.length} note="Exported with configuration" />
          <Stat
            label="Profile data"
            value={formatBytes(data.info?.storageBytes ?? 0)}
            note="Cookies, cache, storage"
          />
          <Stat label="Version" value={`v${data.info?.version ?? "1.0.0"}`} note="Prism" />
        </div>

        <div className="grid-2" style={{ alignItems: "start" }}>
          <Panel
            title="Export configuration"
            description="Profiles, proxies and settings as a single file. No browser data is included, so the file stays small."
          >
            <Field label="Passphrase (optional)" hint="Leave empty for plain JSON. A passphrase encrypts the export.">
              <input
                className="input"
                type="password"
                value={exportPass}
                onChange={(event) => setExportPass(event.target.value)}
                placeholder="Optional"
                autoComplete="new-password"
              />
            </Field>
            <button className="btn btn-primary" onClick={() => void exportConfig()} disabled={busy !== null || !data.profiles.length}>
              {busy === "export" ? <span className="spinner" /> : <Icon name="download" size={14} />}
              Export configuration
            </button>
            {lastExport ? (
              <p className="field-hint mt-8" style={{ wordBreak: "break-all" }}>
                Last export: {lastExport.profiles} profile(s), {lastExport.proxies} prox(y/ies),{" "}
                {formatBytes(lastExport.bytes)} → {lastExport.path}
              </p>
            ) : null}
          </Panel>

          <Panel
            title="Full backup"
            description="A ZIP with the configuration plus every profile's browser directory — cookies, local storage, IndexedDB, history and sessions."
          >
            <Field label="Passphrase (optional)" hint="Strongly recommended: browser data contains live session cookies.">
              <input
                className="input"
                type="password"
                value={backupPass}
                onChange={(event) => setBackupPass(event.target.value)}
                placeholder="Optional"
                autoComplete="new-password"
              />
            </Field>
            <button className="btn" onClick={() => void createBackup()} disabled={busy !== null || !data.profiles.length}>
              {busy === "backup" ? <span className="spinner" /> : <Icon name="archive" size={14} />}
              Create full backup
            </button>
            <p className="field-hint mt-8">
              Large profiles take a while: the archive is compressed and, if you set a passphrase,
              encrypted in 64 KiB chunks as it streams to disk.
            </p>
          </Panel>
        </div>

        <div className="grid-2" style={{ alignItems: "start" }}>
          <Panel
            title="Import configuration"
            description="Adds profiles and proxies from a configuration file. Existing profiles are never overwritten — a duplicate id is imported under a new id."
          >
            <Field label="Passphrase (only if the file is encrypted)">
              <input
                className="input"
                type="password"
                value={importPass}
                onChange={(event) => setImportPass(event.target.value)}
                placeholder="Optional"
                autoComplete="off"
              />
            </Field>
            <button className="btn" onClick={() => void importConfigFile()} disabled={busy !== null}>
              {busy === "import" ? <span className="spinner" /> : <Icon name="upload" size={14} />}
              Choose file &amp; import
            </button>
          </Panel>

          <Panel
            title="Restore a full backup"
            description="Restores profiles together with their browser data. Close every running profile first — Prism refuses to restore while browsers are open."
          >
            <Field label="Passphrase (only if the backup is encrypted)">
              <input
                className="input"
                type="password"
                value={restorePass}
                onChange={(event) => setRestorePass(event.target.value)}
                placeholder="Optional"
                autoComplete="off"
              />
            </Field>
            <button className="btn" onClick={() => void restoreFile()} disabled={busy !== null}>
              {busy === "restore" ? <span className="spinner" /> : <Icon name="refresh" size={14} />}
              Choose backup &amp; restore
            </button>
            {lastImport ? (
              <p className="field-hint mt-8">
                Last import: {lastImport.imported} imported, {lastImport.skipped} skipped
                {lastImport.errors.length ? ` · ${lastImport.errors.length} problem(s)` : ""}
              </p>
            ) : null}
          </Panel>
        </div>

        <Panel title="Application data" description="Where Prism keeps everything on this machine.">
          <div className="col" style={{ gap: 10 }}>
            {[
              { label: "Data directory", value: data.info?.dataDir },
              { label: "Database", value: data.info?.dbPath },
              { label: "Logs", value: data.info?.logsDir },
            ].map((row) => (
              <div className="row between" key={row.label}>
                <div className="col" style={{ gap: 1, minWidth: 0 }}>
                  <span className="text-faint" style={{ fontSize: 11 }}>
                    {row.label}
                  </span>
                  <span className="mono truncate" style={{ maxWidth: 520 }}>
                    {row.value ?? "—"}
                  </span>
                </div>
                <button
                  className="btn btn-sm"
                  disabled={!row.value}
                  onClick={() => row.value && openPath(row.value)}
                >
                  <Icon name="folder" size={13} /> Open
                </button>
              </div>
            ))}
          </div>
        </Panel>

        <Panel
          title="Launch history"
          description="Every launch is recorded with its exit status. Rows left open are closed automatically if Prism is terminated."
          actions={
            <div className="row gap-4">
              <button className="btn btn-sm btn-ghost" onClick={loadHistory}>
                <Icon name="refresh" size={12} /> Refresh
              </button>
              <button
                className="btn btn-sm btn-ghost"
                onClick={async () => {
                  try {
                    await api.clearHistory();
                    setHistory([]);
                    data.toast("ok", "History cleared");
                  } catch (error) {
                    data.toast("error", errorMessage(error));
                  }
                }}
              >
                <Icon name="trash" size={12} /> Clear
              </button>
            </div>
          }
        >
          {history.length === 0 ? (
            <EmptyState
              icon="clock"
              title="No launches recorded"
              description="Launch a profile and its session will appear here with start time, end time and exit code."
            />
          ) : (
            <div className="table-wrap" style={{ marginTop: 4 }}>
              <table>
                <thead>
                  <tr>
                    <th>Profile</th>
                    <th>Started</th>
                    <th>Ended</th>
                    <th>Exit code</th>
                    <th>Note</th>
                  </tr>
                </thead>
                <tbody>
                  {history.map((entry) => (
                    <tr key={entry.id}>
                      <td style={{ fontWeight: 550 }}>{entry.profileName}</td>
                      <td className="text-dim">{formatDate(entry.startedAt)}</td>
                      <td className="text-dim">{entry.endedAt ? formatDate(entry.endedAt) : "running"}</td>
                      <td className={entry.exitCode && entry.exitCode !== 0 ? "text-warn" : "text-dim"}>
                        {entry.exitCode ?? "—"}
                      </td>
                      <td className="text-dim truncate" title={entry.error ?? ""}>
                        {entry.error ?? "—"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Panel>
      </div>
    </div>
  );
}
