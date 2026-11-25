import { useEffect, useState } from "react";
import type { AppData } from "../state/useAppData";
import type { Fingerprint, HistoryEntry, Profile } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import {
  colorFor,
  formatBytes,
  formatDate,
  initials,
  normalizeUrl,
  timeAgo,
} from "../lib/format";
import { Icon } from "../components/Icon";
import { ChipsInput, Field, Switch, Tabs } from "../components/ui";

type TabId = "general" | "identity" | "privacy" | "advanced";

const swatches = ["#4f8cff", "#7c6bff", "#39c0ff", "#3ddc97", "#f5b544", "#ff7a59", "#ff6090", "#5ce1e6"];

export function ProfileEditor({
  data,
  profile,
  onClose,
}: {
  data: AppData;
  profile: Profile;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<Profile>(profile);
  const [tab, setTab] = useState<TabId>("general");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const presets = data.presets;
  const running = data.runningIds.has(profile.id);
  const storageBytes = data.profiles.find((item) => item.id === profile.id)?.storageBytes ?? 0;

  useEffect(() => {
    setDraft(profile);
    setError(null);
  }, [profile]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose, saving]);

  useEffect(() => {
    if (tab !== "advanced") return;
    api
      .history(profile.id, 15)
      .then(setHistory)
      .catch(() => setHistory([]));
  }, [tab, profile.id]);

  const patch = (values: Partial<Profile>) => setDraft((current) => ({ ...current, ...values }));
  const patchFingerprint = (values: Partial<Fingerprint>) =>
    setDraft((current) => ({
      ...current,
      fingerprint: { ...current.fingerprint, ...values },
    }));

  const save = async () => {
    setSaving(true);
    setError(null);
    const payload: Profile = {
      ...draft,
      name: draft.name.trim(),
      startUrls: draft.startUrls.map(normalizeUrl).filter(Boolean),
    };
    if (!payload.name) {
      setError("A profile name is required.");
      setSaving(false);
      return;
    }
    const saved = await data.saveProfile(payload);
    setSaving(false);
    if (saved) onClose();
    else setError("The profile could not be saved. See the notification for the exact reason.");
  };

  const openFolder = async () => {
    try {
      const dir = await api.profileDataDir(profile.id);
      await api.openPath(dir, false);
    } catch (cause) {
      data.toast("error", errorMessage(cause));
    }
  };

  const timezoneOptions =
    presets && !presets.timezones.includes(draft.fingerprint.timezone)
      ? [draft.fingerprint.timezone, ...presets.timezones]
      : presets?.timezones ?? [draft.fingerprint.timezone];

  return (
    <>
      <div className="drawer-backdrop" onClick={() => !saving && onClose()} />
      <aside className="drawer" role="dialog" aria-label="Profile editor">
        <header className="drawer-header">
          <div
            className="avatar"
            style={{
              background: `linear-gradient(150deg, ${draft.color}, ${draft.color}99)`,
            }}
          >
            {initials(draft.name || "?")}
          </div>
          <div className="grow">
            <div style={{ fontWeight: 600, fontSize: 14 }}>{draft.name || "New profile"}</div>
            <div className="row gap-4 text-faint" style={{ fontSize: 11.5 }}>
              <span className={`status-dot ${running ? "running" : "idle"}`} />
              {running ? "Running now" : `Last launch ${timeAgo(draft.lastLaunchedAt)}`}
              <span>·</span>
              {formatBytes(storageBytes)}
            </div>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close editor">
            <Icon name="x" size={16} />
          </button>
        </header>

        <div className="drawer-body">
          <Tabs
            value={tab}
            onChange={setTab}
            tabs={[
              { id: "general", label: "General" },
              { id: "identity", label: "Identity" },
              { id: "privacy", label: "Privacy" },
              { id: "advanced", label: "Advanced" },
            ]}
          />

          {error ? (
            <div className="panel" style={{ borderColor: "rgba(255,96,114,.4)", marginBottom: 14 }}>
              <div className="row" style={{ color: "var(--danger)" }}>
                <Icon name="alert" size={15} />
                {error}
              </div>
            </div>
          ) : null}

          {tab === "general" ? (
            <>
              <Field label="Profile name">
                <input
                  className="input"
                  value={draft.name}
                  maxLength={80}
                  onChange={(event) => patch({ name: event.target.value })}
                  placeholder="e.g. Instagram — EU store"
                />
              </Field>

              <div className="grid-2">
                <Field label="Proxy" hint="The browser only ever talks to a loopback relay, never to the proxy directly.">
                  <select
                    className="select"
                    value={draft.proxyId ?? ""}
                    onChange={(event) => patch({ proxyId: event.target.value || null })}
                  >
                    <option value="">Direct connection</option>
                    {data.proxies.map((proxy) => (
                      <option key={proxy.id} value={proxy.id}>
                        {proxy.label} · {proxy.protocol}://{proxy.host}:{proxy.port}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Folder" hint="Free-form grouping used by the folder filter.">
                  <input
                    className="input"
                    value={draft.folder}
                    onChange={(event) => patch({ folder: event.target.value })}
                    placeholder="e.g. Clients / EU"
                  />
                </Field>
              </div>

              <Field label="Accent colour">
                <div className="row wrap" style={{ gap: 8 }}>
                  {swatches.map((color) => (
                    <button
                      key={color}
                      type="button"
                      aria-label={`Use ${color}`}
                      onClick={() => patch({ color })}
                      style={{
                        width: 26,
                        height: 26,
                        borderRadius: 8,
                        cursor: "pointer",
                        background: color,
                        border:
                          draft.color === color
                            ? "2px solid #fff"
                            : "1px solid rgba(255,255,255,.18)",
                        boxShadow: draft.color === color ? "0 0 0 3px rgba(79,140,255,.25)" : "none",
                      }}
                    />
                  ))}
                  <button
                    type="button"
                    className="btn btn-sm btn-ghost"
                    onClick={() => patch({ color: colorFor(draft.name + Date.now()) })}
                  >
                    <Icon name="refresh" size={12} /> Shuffle
                  </button>
                </div>
              </Field>

              <Field label="Tags" hint="Press Enter to add. Tags power the filter chips in the toolbar.">
                <ChipsInput
                  values={draft.tags}
                  onChange={(tags) => patch({ tags })}
                  placeholder="e.g. instagram, warmup, eu"
                />
              </Field>

              <Field label="Start pages" hint="Opened in order when the profile launches.">
                <ChipsInput
                  values={draft.startUrls}
                  onChange={(startUrls) => patch({ startUrls })}
                  placeholder="https://www.instagram.com"
                />
              </Field>

              <Field label="Notes">
                <textarea
                  className="textarea"
                  value={draft.notes}
                  onChange={(event) => patch({ notes: event.target.value })}
                  placeholder="Account details, warm-up state, anything you want to remember."
                />
              </Field>

              <Switch
                label="Archived"
                description="Archived profiles are hidden from the Active filter and cannot be launched until restored. Their data is kept."
                checked={draft.status === "archived"}
                onChange={(value) => patch({ status: value ? "archived" : "active" })}
              />
            </>
          ) : null}

          {tab === "identity" ? (
            <>
              <div className="grid-2">
                <Field
                  label="Operating system"
                  hint="Sets the platform hint and the suggested user agent. The browser binary itself is unchanged."
                >
                  <select
                    className="select"
                    value={draft.fingerprint.os}
                    onChange={(event) => {
                      const os = event.target.value;
                      const platform = os === "macos" ? "MacIntel" : os === "linux" ? "Linux x86_64" : "Win32";
                      patchFingerprint({ os, platform });
                    }}
                  >
                    <option value="windows">Windows</option>
                    <option value="macos">macOS</option>
                    <option value="linux">Linux</option>
                  </select>
                </Field>
                <Field label="Platform hint" hint="Reported by navigator.platform.">
                  <input
                    className="input"
                    value={draft.fingerprint.platform}
                    onChange={(event) => patchFingerprint({ platform: event.target.value })}
                  />
                </Field>
              </div>

              <Field
                label="User agent"
                hint="Leave empty to use the real user agent of the installed browser. A mismatch between the user agent and the platform hint is itself detectable, so keep them consistent."
              >
                <select
                  className="select"
                  value={
                    presets?.userAgents.some((preset) => preset.value === draft.fingerprint.userAgent)
                      ? draft.fingerprint.userAgent
                      : "__custom"
                  }
                  onChange={(event) => {
                    const value = event.target.value;
                    if (value === "__custom") {
                      patchFingerprint({ userAgent: "" });
                      return;
                    }
                    const preset = presets?.userAgents.find((item) => item.value === value);
                    patchFingerprint({
                      userAgent: value,
                      os: preset?.os ?? draft.fingerprint.os,
                      platform:
                        preset?.os === "macos"
                          ? "MacIntel"
                          : preset?.os === "linux"
                          ? "Linux x86_64"
                          : "Win32",
                    });
                  }}
                >
                  <option value="">Browser default (recommended)</option>
                  {presets?.userAgents.map((preset) => (
                    <option key={preset.os} value={preset.value}>
                      {preset.label}
                    </option>
                  ))}
                  <option value="__custom">Custom…</option>
                </select>
              </Field>

              {draft.fingerprint.userAgent ? (
                <Field label="Custom user agent string">
                  <input
                    className="input mono"
                    value={draft.fingerprint.userAgent}
                    onChange={(event) => patchFingerprint({ userAgent: event.target.value })}
                  />
                </Field>
              ) : null}

              <div className="grid-2">
                <Field label="Locale" hint="Applied to navigator.language and Intl.">
                  <select
                    className="select"
                    value={draft.fingerprint.locale}
                    onChange={(event) => patchFingerprint({ locale: event.target.value })}
                  >
                    {(presets?.locales ?? [draft.fingerprint.locale]).map((locale) => (
                      <option key={locale} value={locale}>
                        {locale}
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Timezone" hint="Applied through the Chrome DevTools Protocol.">
                  <select
                    className="select"
                    value={draft.fingerprint.timezone}
                    onChange={(event) => patchFingerprint({ timezone: event.target.value })}
                  >
                    {timezoneOptions.map((timezone) => (
                      <option key={timezone} value={timezone}>
                        {timezone}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>

              <Field label="Languages" hint="First entry becomes navigator.language; the rest form Accept-Language.">
                <ChipsInput
                  values={draft.fingerprint.languages}
                  onChange={(languages) => patchFingerprint({ languages })}
                  placeholder="en-US"
                />
              </Field>

              <Field label="Screen preset">
                <select
                  className="select"
                  value={`${draft.fingerprint.screen.width}x${draft.fingerprint.screen.height}`}
                  onChange={(event) => {
                    const preset = presets?.screens.find(
                      (item) => `${item.width}x${item.height}` === event.target.value
                    );
                    if (!preset) return;
                    patchFingerprint({
                      screen: {
                        ...draft.fingerprint.screen,
                        width: preset.width,
                        height: preset.height,
                        screenWidth: preset.width,
                        screenHeight: preset.height,
                        deviceScaleFactor: preset.dpr,
                      },
                    });
                  }}
                >
                  {presets?.screens.map((preset) => (
                    <option key={preset.label} value={`${preset.width}x${preset.height}`}>
                      {preset.label}
                    </option>
                  ))}
                </select>
              </Field>

              <div className="grid-3">
                <Field label="Window width">
                  <input
                    className="input num"
                    type="number"
                    min={640}
                    value={draft.fingerprint.screen.width}
                    onChange={(event) =>
                      patchFingerprint({
                        screen: { ...draft.fingerprint.screen, width: Number(event.target.value) },
                      })
                    }
                  />
                </Field>
                <Field label="Window height">
                  <input
                    className="input num"
                    type="number"
                    min={480}
                    value={draft.fingerprint.screen.height}
                    onChange={(event) =>
                      patchFingerprint({
                        screen: { ...draft.fingerprint.screen, height: Number(event.target.value) },
                      })
                    }
                  />
                </Field>
                <Field label="Pixel ratio">
                  <input
                    className="input num"
                    type="number"
                    step={0.25}
                    min={0.5}
                    value={draft.fingerprint.screen.deviceScaleFactor}
                    onChange={(event) =>
                      patchFingerprint({
                        screen: {
                          ...draft.fingerprint.screen,
                          deviceScaleFactor: Number(event.target.value),
                        },
                      })
                    }
                  />
                </Field>
                <Field label="Screen width">
                  <input
                    className="input num"
                    type="number"
                    value={draft.fingerprint.screen.screenWidth}
                    onChange={(event) =>
                      patchFingerprint({
                        screen: { ...draft.fingerprint.screen, screenWidth: Number(event.target.value) },
                      })
                    }
                  />
                </Field>
                <Field label="Screen height">
                  <input
                    className="input num"
                    type="number"
                    value={draft.fingerprint.screen.screenHeight}
                    onChange={(event) =>
                      patchFingerprint({
                        screen: { ...draft.fingerprint.screen, screenHeight: Number(event.target.value) },
                      })
                    }
                  />
                </Field>
                <Field label="CPUs">
                  <input
                    className="input num"
                    type="number"
                    min={1}
                    max={64}
                    value={draft.fingerprint.hardwareConcurrency}
                    onChange={(event) =>
                      patchFingerprint({ hardwareConcurrency: Number(event.target.value) })
                    }
                  />
                </Field>
              </div>

              <div className="grid-2">
                <Field label="Device memory (GB)">
                  <select
                    className="select"
                    value={draft.fingerprint.deviceMemoryGb}
                    onChange={(event) =>
                      patchFingerprint({ deviceMemoryGb: Number(event.target.value) })
                    }
                  >
                    {[2, 4, 8, 16, 32].map((value) => (
                      <option key={value} value={value}>
                        {value} GB
                      </option>
                    ))}
                  </select>
                </Field>
                <Field label="Colour scheme">
                  <select
                    className="select"
                    value={draft.fingerprint.colorScheme}
                    onChange={(event) => patchFingerprint({ colorScheme: event.target.value })}
                  >
                    {(presets?.colorSchemes ?? ["dark", "light", "system"]).map((scheme) => (
                      <option key={scheme} value={scheme}>
                        {scheme}
                      </option>
                    ))}
                  </select>
                </Field>
              </div>

              <Switch
                label="Mobile viewport"
                description="Reports a mobile device metric to pages. Useful only for testing responsive layouts."
                checked={draft.fingerprint.screen.mobile}
                onChange={(value) =>
                  patchFingerprint({ screen: { ...draft.fingerprint.screen, mobile: value } })
                }
              />
            </>
          ) : null}

          {tab === "privacy" ? (
            <>
              <Field
                label="WebRTC policy"
                hint="Chrome exposes your local network address to WebRTC by default. “Proxy only” keeps non-proxied UDP candidates out; “Block” also removes the RTCPeerConnection API for this profile."
              >
                <select
                  className="select"
                  value={draft.fingerprint.webrtc}
                  onChange={(event) => patchFingerprint({ webrtc: event.target.value })}
                >
                  <option value="default">Browser default</option>
                  <option value="proxy">Proxy only (recommended)</option>
                  <option value="disabled">Block WebRTC</option>
                </select>
              </Field>

              <Switch
                label="Canvas readback noise"
                description="Adds deterministic, seed-based pixel noise to canvas readbacks (toDataURL, getImageData). The same profile always produces the same noise, so nothing flickers between page loads."
                checked={draft.fingerprint.canvasNoise}
                onChange={(value) => patchFingerprint({ canvasNoise: value })}
              />
              <Switch
                label="WebGL readback noise"
                description="Perturbs gl.readPixels output and reports a generic GPU for the debug renderer extension."
                checked={draft.fingerprint.webglNoise}
                onChange={(value) => patchFingerprint({ webglNoise: value })}
              />
              <Switch
                label="Do Not Track header"
                description="Sends DNT: 1 with every request the profile makes."
                checked={draft.fingerprint.doNotTrack}
                onChange={(value) => patchFingerprint({ doNotTrack: value })}
              />

              <div className="panel" style={{ marginTop: 16, background: "rgba(79,140,255,.05)" }}>
                <div className="row" style={{ alignItems: "flex-start", gap: 10 }}>
                  <Icon name="shield" size={16} style={{ color: "var(--accent)", marginTop: 2 }} />
                  <div className="text-dim" style={{ fontSize: 12, lineHeight: 1.6 }}>
                    These controls are real browser settings, applied by a small extension that Prism
                    generates for this profile and a documented CDP session. Prism does not patch the
                    browser binary, does not hide automation, and does not attempt to defeat any
                    platform's security or anti-abuse systems. Use separate accounts you are entitled
                    to use.
                  </div>
                </div>
              </div>
            </>
          ) : null}

          {tab === "advanced" ? (
            <>
              <Field
                label="Noise seed"
                hint="Deterministic per profile. Changing it changes the canvas/WebGL signature."
                right={
                  <button
                    type="button"
                    className="btn btn-sm btn-ghost"
                    onClick={() => patchFingerprint({ seed: Math.floor(Math.random() * 2 ** 48) })}
                  >
                    <Icon name="refresh" size={12} /> Regenerate
                  </button>
                }
              >
                <input
                  className="input mono"
                  type="number"
                  value={draft.fingerprint.seed}
                  onChange={(event) => patchFingerprint({ seed: Number(event.target.value) })}
                />
              </Field>

              <Switch
                label="Headless mode"
                description="Runs without a visible browser window. Useful for checks, not for interactive work on social platforms."
                checked={draft.fingerprint.headless}
                onChange={(value) => patchFingerprint({ headless: value })}
              />

              <div className="panel" style={{ marginTop: 16 }}>
                <div className="row between">
                  <div className="col" style={{ gap: 2 }}>
                    <strong style={{ fontSize: 12.5 }}>Profile data</strong>
                    <span className="text-faint mono" style={{ fontSize: 11 }}>
                      profiles/{profile.id}
                    </span>
                  </div>
                  <button className="btn btn-sm" onClick={() => void openFolder()}>
                    <Icon name="folder" size={13} /> Open folder
                  </button>
                </div>
                <div className="grid-2 mt-16">
                  <div>
                    <div className="stat-label">Created</div>
                    <div className="text-dim">{formatDate(draft.createdAt)}</div>
                  </div>
                  <div>
                    <div className="stat-label">Last updated</div>
                    <div className="text-dim">{formatDate(draft.updatedAt)}</div>
                  </div>
                  <div>
                    <div className="stat-label">Launches</div>
                    <div className="text-dim num">{draft.launchCount}</div>
                  </div>
                  <div>
                    <div className="stat-label">Disk usage</div>
                    <div className="text-dim num">{formatBytes(storageBytes)}</div>
                  </div>
                </div>
              </div>

              <div className="panel" style={{ marginTop: 14 }}>
                <div className="row between" style={{ marginBottom: 10 }}>
                  <strong style={{ fontSize: 12.5 }}>Launch history</strong>
                  <button
                    className="btn btn-sm btn-ghost"
                    onClick={() =>
                      api
                        .history(profile.id, 15)
                        .then(setHistory)
                        .catch((cause) => data.toast("error", errorMessage(cause)))
                    }
                  >
                    <Icon name="refresh" size={12} /> Refresh
                  </button>
                </div>
                {history.length === 0 ? (
                  <p className="text-faint" style={{ fontSize: 12, margin: 0 }}>
                    No launches recorded yet.
                  </p>
                ) : (
                  <table>
                    <thead>
                      <tr>
                        <th>Started</th>
                        <th>Ended</th>
                        <th>Exit</th>
                      </tr>
                    </thead>
                    <tbody>
                      {history.map((entry) => (
                        <tr key={entry.id}>
                          <td className="text-dim">{formatDate(entry.startedAt)}</td>
                          <td className="text-dim">
                            {entry.endedAt ? formatDate(entry.endedAt) : "running"}
                          </td>
                          <td className={entry.exitCode && entry.exitCode !== 0 ? "text-warn" : "text-dim"}>
                            {entry.error ?? entry.exitCode ?? "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </>
          ) : null}
        </div>

        <footer className="drawer-footer">
          <button
            className="btn btn-ghost"
            disabled={running || saving || draft.status === "archived"}
            onClick={async () => {
              const saved = await data.launch(draft.id);
              if (saved) onClose();
            }}
          >
            <Icon name="play" size={13} /> Launch
          </button>
          <span className="grow" />
          <button className="btn" onClick={onClose} disabled={saving}>
            Cancel
          </button>
          <button className="btn btn-primary" onClick={() => void save()} disabled={saving}>
            {saving ? <span className="spinner" /> : <Icon name="check" size={14} />}
            Save changes
          </button>
        </footer>
      </aside>
    </>
  );
}
