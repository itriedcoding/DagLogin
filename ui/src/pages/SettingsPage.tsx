import { useEffect, useState } from "react";
import type { AppData } from "../state/useAppData";
import type { Settings } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import { pickOpenPath } from "../lib/files";
import { formatBytes } from "../lib/format";
import { Icon } from "../components/Icon";
import { ChipsInput, Field, Panel, Stat, Switch } from "../components/ui";

export function SettingsPage({ data }: { data: AppData }) {
  const [draft, setDraft] = useState<Settings | null>(data.settings);
  const [saving, setSaving] = useState(false);
  const [detecting, setDetecting] = useState(false);

  useEffect(() => {
    setDraft(data.settings);
  }, [data.settings]);

  if (!draft) {
    return (
      <div className="page">
        <div className="page-header">
          <h1 className="page-title">Settings</h1>
        </div>
        <div className="page-body">
          <div className="skeleton" style={{ height: 140 }} />
        </div>
      </div>
    );
  }

  const dirty = JSON.stringify(draft) !== JSON.stringify(data.settings);
  const update = (values: Partial<Settings>) => setDraft({ ...draft, ...values });
  const browser = data.info?.browser;

  const save = async () => {
    setSaving(true);
    await data.saveSettings(draft);
    setSaving(false);
  };

  const chooseBrowser = async () => {
    const path = await pickOpenPath([{ name: "Browser executable", extensions: ["exe"] }]);
    if (!path) return;
    setDetecting(true);
    try {
      const info = await api.detectBrowser(path);
      if (!info.detected) {
        data.toast("error", "That file does not look like a Chromium-based browser executable.");
      } else {
        update({ browserPath: path });
        data.toast("ok", `Detected ${info.name}${info.version ? ` ${info.version}` : ""}`);
      }
    } catch (error) {
      data.toast("error", errorMessage(error));
    } finally {
      setDetecting(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">
            Browser integration, resource limits and defaults for new profiles. Changes apply to
            profiles launched from now on.
          </p>
        </div>
        <div className="page-header-actions">
          {dirty ? <span className="text-warn" style={{ fontSize: 12 }}>Unsaved changes</span> : null}
          <button className="btn" onClick={() => setDraft(data.settings)} disabled={!dirty || saving}>
            Reset
          </button>
          <button className="btn btn-primary" onClick={() => void save()} disabled={!dirty || saving}>
            {saving ? <span className="spinner" /> : <Icon name="check" size={14} />}
            Save settings
          </button>
        </div>
      </div>

      <div className="page-body">
        <Panel
          title="Browser engine"
          description="Prism drives an installed Chromium-family browser. Each profile gets its own user-data directory, which is what provides real isolation."
          actions={
            <button className="btn btn-sm" onClick={() => void chooseBrowser()} disabled={detecting}>
              {detecting ? <span className="spinner" /> : <Icon name="folder" size={13} />}
              Choose executable
            </button>
          }
        >
          <div className="row wrap gap-12" style={{ marginBottom: 12 }}>
            <span className={`badge ${browser?.detected ? "ok" : "danger"}`}>
              {browser?.detected ? "detected" : "not found"}
            </span>
            <strong style={{ fontSize: 13 }}>{browser?.name ?? "Unknown"}</strong>
            {browser?.version ? <span className="text-dim mono">{browser.version}</span> : null}
          </div>
          <Field label="Executable path" hint="Leave empty to use automatic detection on every launch.">
            <input
              className="input mono"
              value={draft.browserPath}
              onChange={(event) => update({ browserPath: event.target.value })}
              placeholder="Auto-detect (Chrome, Edge, Brave, Chromium)"
            />
          </Field>
          {browser && browser.candidates.length > 0 ? (
            <div className="col" style={{ gap: 4 }}>
              <span className="text-faint" style={{ fontSize: 11 }}>
                Found on this machine
              </span>
              {browser.candidates.slice(0, 6).map((candidate) => (
                <button
                  key={candidate}
                  className="btn btn-sm btn-ghost"
                  style={{ justifyContent: "flex-start" }}
                  onClick={() => update({ browserPath: candidate })}
                >
                  <Icon name="globe" size={12} />
                  <span className="mono truncate" style={{ maxWidth: 620 }}>
                    {candidate}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <p className="field-hint">
              No Chromium browser was found in the usual locations. Install Google Chrome or Microsoft
              Edge, or pick the executable manually.
            </p>
          )}
        </Panel>

        <Panel title="Performance" description="Keep many profiles open without exhausting the machine.">
          <div className="grid-2">
            <Field
              label="Maximum profiles running at once"
              hint="0 means unlimited. Prism refuses new launches above the limit instead of silently degrading."
            >
              <input
                className="input num"
                type="number"
                min={0}
                max={200}
                value={draft.maxConcurrent}
                onChange={(event) => update({ maxConcurrent: Number(event.target.value) })}
              />
            </Field>
            <Field
              label="Proxy test endpoint"
              hint="A small HTTPS endpoint that reports the caller's IP. Used by “Test connection”."
            >
              <input
                className="input mono"
                value={draft.proxyTestUrl}
                onChange={(event) => update({ proxyTestUrl: event.target.value })}
              />
            </Field>
          </div>
          <Switch
            label="Hardware acceleration"
            description="Disable to force software rendering when a GPU driver makes many browser windows unstable."
            checked={draft.hardwareAcceleration}
            onChange={(value) => update({ hardwareAcceleration: value })}
          />
          <Switch
            label="Stop browsers when Prism closes"
            description="Gracefully closes every running profile so cookies and storage are flushed. Disable only if you want browsers to outlive the app."
            checked={draft.closeBrowsersOnExit}
            onChange={(value) => update({ closeBrowsersOnExit: value })}
          />
        </Panel>

        <Panel title="Privacy defaults" description="Applied to every profile and also configurable per profile.">
          <Switch
            label="Apply fingerprint overrides through the DevTools Protocol"
            description="Sets timezone, locale, screen metrics, media preferences and the user agent in each page. Without it, only command-line settings (language, user agent, window size) are applied."
            checked={draft.cdpOverrides}
            onChange={(value) => update({ cdpOverrides: value })}
          />
          <Switch
            label="Reduce animations"
            description="Turns off transitions and pulse effects across the interface."
            checked={draft.reduceAnimations}
            onChange={(value) => {
              update({ reduceAnimations: value });
              document.documentElement.dataset.reduceMotion = String(value);
            }}
          />
        </Panel>

        <Panel title="New profile defaults">
          <Field label="Start pages" hint="Opened when a newly created profile launches. Press Enter to add.">
            <ChipsInput
              values={draft.defaultStartUrls}
              onChange={(defaultStartUrls) => update({ defaultStartUrls })}
              placeholder="https://www.google.com"
            />
          </Field>
          <Switch
            label="Confirm before deleting a profile"
            description="Deletion permanently removes the profile's cookies, storage and cache."
            checked={draft.confirmOnDelete}
            onChange={(value) => update({ confirmOnDelete: value })}
          />
        </Panel>

        <Panel title="Diagnostics">
          <div className="stat-row" style={{ marginBottom: 8 }}>
            <Stat label="Data directory" value={formatBytes(data.info?.storageBytes ?? 0)} note={data.info?.dataDir} />
            <Stat label="Profiles" value={data.info?.profileCount ?? 0} note="Stored locally" />
            <Stat label="Proxies" value={data.info?.proxyCount ?? 0} note="Credentials encrypted" />
            <Stat label="Platform" value={data.info?.os ?? "—"} note={`Prism v${data.info?.version ?? "1.0.0"}`} />
          </div>
          <Field label="Log level" hint="Applies after restart. Logs are rotated daily.">
            <select
              className="select"
              value={draft.logLevel}
              onChange={(event) => update({ logLevel: event.target.value })}
            >
              <option value="error">Errors only</option>
              <option value="warn">Warnings</option>
              <option value="info">Info</option>
              <option value="debug">Debug</option>
            </select>
          </Field>
          <div className="row wrap gap-12 mt-8">
            <button
              className="btn btn-sm"
              onClick={() =>
                data.info &&
                api.openPath(data.info.dataDir, false).catch((error) => data.toast("error", errorMessage(error)))
              }
            >
              <Icon name="folder" size={13} /> Open data folder
            </button>
            <button
              className="btn btn-sm"
              onClick={() =>
                data.info &&
                api.openPath(data.info.logsDir, false).catch((error) => data.toast("error", errorMessage(error)))
              }
            >
              <Icon name="terminal" size={13} /> Open logs
            </button>
            <button
              className="btn btn-sm btn-ghost"
              onClick={() =>
                api
                  .openPath("https://github.com/itriedcoding/DagLogin", false)
                  .catch((error) => data.toast("error", errorMessage(error)))
              }
            >
              <Icon name="info" size={13} /> Project page
            </button>
          </div>
        </Panel>
      </div>
    </div>
  );
}
