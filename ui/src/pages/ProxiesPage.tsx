import { useState } from "react";
import type { AppData } from "../state/useAppData";
import type { Proxy, ProxyInput, ProxyProtocol, ProxyTest } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import { pickOpenPath, pickSavePath, stampedName } from "../lib/files";
import { formatDate, timeAgo } from "../lib/format";
import { Icon } from "../components/Icon";
import { EmptyState, Field, Modal, Panel, Stat } from "../components/ui";

const emptyDraft: ProxyInput = {
  id: null,
  label: "",
  protocol: "socks5",
  host: "",
  port: 1080,
  username: "",
  password: "",
};

export function ProxiesPage({ data }: { data: AppData }) {
  const [editing, setEditing] = useState<ProxyInput | null>(null);
  const [importing, setImporting] = useState(false);
  const [importText, setImportText] = useState("");
  const [pending, setPending] = useState<string[]>([]);
  const [testingAll, setTestingAll] = useState(false);

  const profilesFor = (proxyId: string) => data.profiles.filter((p) => p.proxyId === proxyId).length;

  const test = async (id: string) => {
    setPending((current) => [...current, id]);
    try {
      const result = await api.testProxy(id);
      await data.refreshProxies();
      data.toast(
        result.ok ? "ok" : "error",
        result.ok
          ? `Proxy reachable · ${result.latencyMs} ms${result.exitIp ? ` · exit IP ${result.exitIp}` : ""}`
          : result.error ?? "The proxy test failed"
      );
    } catch (error) {
      data.toast("error", errorMessage(error));
    } finally {
      setPending((current) => current.filter((value) => value !== id));
    }
  };

  const testAll = async () => {
    setTestingAll(true);
    try {
      await api.testAllProxies();
      await data.refreshProxies();
      data.toast("info", "All proxies tested");
    } catch (error) {
      data.toast("error", errorMessage(error));
    } finally {
      setTestingAll(false);
    }
  };

  const doImport = async () => {
    try {
      const count = await api.importProxies(importText);
      await data.refreshProxies();
      setImporting(false);
      setImportText("");
      data.toast("ok", `${count} proxy configuration(s) imported`);
    } catch (error) {
      data.toast("error", errorMessage(error));
    }
  };

  const importFromFile = async () => {
    const path = await pickOpenPath([
      { name: "Proxy list", extensions: ["txt", "csv", "list"] },
      { name: "All files", extensions: ["*"] },
    ]);
    if (!path) return;
    try {
      const text = await api.readTextFile(path);
      setImportText(text);
      setImporting(true);
    } catch (error) {
      data.toast("error", errorMessage(error));
    }
  };

  const exportAll = async () => {
    const path = await pickSavePath(stampedName("prism-proxies", "txt"), [
      { name: "Proxy list", extensions: ["txt"] },
    ]);
    if (!path) return;
    try {
      const text = await api.exportProxies();
      await api.saveTextFile(path, text);
      data.toast("ok", `Exported ${data.proxies.length} proxies`);
    } catch (error) {
      data.toast("error", errorMessage(error));
    }
  };

  const healthy = data.proxies.filter((proxy) => proxy.lastTest?.ok).length;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Proxies</h1>
          <p className="page-subtitle">
            HTTP, HTTPS and SOCKS5, with optional username/password. Prism starts a loopback relay per
            profile so credentials never reach the browser process.
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn" onClick={() => void importFromFile()}>
            <Icon name="upload" size={14} /> Import file
          </button>
          <button className="btn" onClick={() => setImporting(true)}>
            <Icon name="upload" size={14} /> Paste list
          </button>
          <button className="btn" onClick={() => void exportAll()} disabled={!data.proxies.length}>
            <Icon name="download" size={14} /> Export
          </button>
          <button
            className="btn btn-primary"
            onClick={() => setEditing({ ...emptyDraft })}
          >
            <Icon name="plus" size={15} /> Add proxy
          </button>
        </div>
      </div>

      <div className="page-body">
        <div className="stat-row" style={{ marginTop: 16 }}>
          <Stat label="Configured" value={data.proxies.length} note="Assigned per profile" />
          <Stat
            label="Healthy"
            value={`${healthy}/${data.proxies.length}`}
            note="Last connectivity test"
          />
          <Stat
            label="Protocols"
            value={new Set(data.proxies.map((proxy) => proxy.protocol)).size || 0}
            note="HTTP · HTTPS · SOCKS5"
          />
          <Stat
            label="Unassigned"
            value={data.proxies.filter((proxy) => profilesFor(proxy.id) === 0).length}
            note="Not used by any profile"
          />
        </div>

        <div className="row between mt-8" style={{ marginBottom: 10 }}>
          <strong style={{ fontSize: 13 }}>Proxy pool</strong>
          <button className="btn btn-sm" onClick={() => void testAll()} disabled={testingAll || !data.proxies.length}>
            {testingAll ? <span className="spinner" /> : <Icon name="refresh" size={13} />}
            Test all
          </button>
        </div>

        {data.proxies.length === 0 ? (
          <EmptyState
            icon="proxy"
            title="No proxies yet"
            description="Add a proxy to give a profile its own network identity. You can paste a whole list — one per line — in the formats providers usually hand out."
            action={
              <button className="btn btn-primary" onClick={() => setEditing({ ...emptyDraft })}>
                <Icon name="plus" size={15} /> Add proxy
              </button>
            }
          />
        ) : (
          <div className="table-wrap" style={{ marginTop: 0 }}>
            <table>
              <thead>
                <tr>
                  <th>Label</th>
                  <th>Endpoint</th>
                  <th>Auth</th>
                  <th>Last test</th>
                  <th>Profiles</th>
                  <th style={{ width: 150 }} />
                </tr>
              </thead>
              <tbody>
                {data.proxies.map((proxy) => {
                  const busy = pending.includes(proxy.id);
                  return (
                    <tr key={proxy.id}>
                      <td>
                        <div className="row gap-4">
                          <span
                            className="status-dot"
                            style={{
                              background: proxy.lastTest
                                ? proxy.lastTest.ok
                                  ? "var(--ok)"
                                  : "var(--danger)"
                                : "#5a6478",
                            }}
                          />
                          <span style={{ fontWeight: 550 }}>{proxy.label}</span>
                        </div>
                      </td>
                      <td className="mono text-dim">
                        {proxy.protocol}://{proxy.host}:{proxy.port}
                      </td>
                      <td className="text-dim">
                        {proxy.username ? (
                          <span className="row gap-4">
                            <Icon name="lock" size={12} /> {proxy.username}
                          </span>
                        ) : (
                          "none"
                        )}
                      </td>
                      <td>
                        <TestCell proxy={proxy} />
                      </td>
                      <td className="num text-dim">{profilesFor(proxy.id)}</td>
                      <td>
                        <div className="row-actions">
                          <button
                            className="icon-button"
                            onClick={() => void test(proxy.id)}
                            disabled={busy}
                            aria-label="Test proxy"
                          >
                            {busy ? <span className="spinner" /> : <Icon name="activity" size={14} />}
                          </button>
                          <button
                            className="icon-button"
                            onClick={() =>
                              setEditing({
                                id: proxy.id,
                                label: proxy.label,
                                protocol: proxy.protocol,
                                host: proxy.host,
                                port: proxy.port,
                                username: proxy.username,
                                password: proxy.password,
                              })
                            }
                            aria-label="Edit proxy"
                          >
                            <Icon name="edit" size={14} />
                          </button>
                          <button
                            className="icon-button"
                            onClick={() => void data.deleteProxy(proxy.id)}
                            aria-label="Delete proxy"
                          >
                            <Icon name="trash" size={14} />
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

        <Panel title="How credentials are handled" description="">
          <div className="text-dim" style={{ fontSize: 12, lineHeight: 1.7 }}>
            Chromium's <span className="mono">--proxy-server</span> flag cannot carry a password and its
            SOCKS5 client has no authentication support at all. Prism therefore starts a relay bound to
            127.0.0.1 for each running profile. The browser speaks unauthenticated SOCKS5 to that
            relay; the relay performs the authenticated handshake with your provider and pipes the
            bytes. Passwords are stored with AES-256-GCM, and the master key is protected by Windows
            DPAPI for the current user.
          </div>
        </Panel>
      </div>

      {editing ? (
        <ProxyDialog
          initial={editing}
          onClose={() => setEditing(null)}
          onSave={async (input) => {
            const saved = await data.saveProxy(input);
            if (saved) setEditing(null);
          }}
        />
      ) : null}

      {importing ? (
        <Modal
          title="Import proxies"
          description="One per line. Supported: socks5://user:pass@host:port, http://host:8080, host:port, host:port:user:pass, user:pass@host:port, or label,protocol,host,port,user,pass"
          onClose={() => setImporting(false)}
          wide
          footer={
            <>
              <button className="btn btn-ghost" onClick={() => setImporting(false)}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={() => void doImport()} disabled={!importText.trim()}>
                <Icon name="upload" size={14} /> Import {importText.trim() ? "" : ""}
              </button>
            </>
          }
        >
          <textarea
            className="textarea mono"
            style={{ minHeight: 220 }}
            value={importText}
            onChange={(event) => setImportText(event.target.value)}
            placeholder={"socks5://user:password@gate.example.com:1080\nhttp://10.0.0.12:8080"}
          />
        </Modal>
      ) : null}
    </div>
  );
}

function TestCell({ proxy }: { proxy: Proxy }) {
  if (!proxy.lastTest) return <span className="text-faint">Not tested</span>;
  const test = proxy.lastTest;
  if (!test.ok) {
    return (
      <span className="text-danger truncate" style={{ display: "inline-block", maxWidth: 260 }} title={test.error ?? ""}>
        {test.error ?? "Failed"}
      </span>
    );
  }
  return (
    <div className="col" style={{ gap: 1 }}>
      <span className="text-ok">
        {test.latencyMs} ms{test.exitIp ? ` · ${test.exitIp}` : ""}
      </span>
      <span className="text-faint" style={{ fontSize: 11 }}>
        {timeAgo(test.testedAt)}
      </span>
    </div>
  );
}

function ProxyDialog({
  initial,
  onClose,
  onSave,
}: {
  initial: ProxyInput;
  onClose: () => void;
  onSave: (input: ProxyInput) => Promise<void>;
}) {
  const [draft, setDraft] = useState<ProxyInput>(initial);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<ProxyTest | null>(null);
  const [error, setError] = useState<string | null>(null);

  const update = (values: Partial<ProxyInput>) => setDraft((current) => ({ ...current, ...values }));

  const test = async () => {
    setTesting(true);
    setResult(null);
    try {
      setResult(await api.testProxyDraft(draft));
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setTesting(false);
    }
  };

  return (
    <Modal
      title={draft.id ? "Edit proxy" : "Add proxy"}
      description="Credentials are encrypted before they are written to disk."
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose} disabled={saving}>
            Cancel
          </button>
          <button className="btn" onClick={() => void test()} disabled={testing || !draft.host}>
            {testing ? <span className="spinner" /> : <Icon name="activity" size={14} />}
            Test connection
          </button>
          <button
            className="btn btn-primary"
            disabled={saving || !draft.host}
            onClick={async () => {
              setSaving(true);
              setError(null);
              await onSave(draft);
              setSaving(false);
            }}
          >
            {saving ? <span className="spinner" /> : <Icon name="check" size={14} />}
            {draft.id ? "Save proxy" : "Add proxy"}
          </button>
        </>
      }
    >
      <Field label="Label">
        <input
          className="input"
          value={draft.label}
          onChange={(event) => update({ label: event.target.value })}
          placeholder="e.g. Frankfurt residential"
        />
      </Field>
      <div className="grid-3">
        <Field label="Protocol">
          <select
            className="select"
            value={draft.protocol}
            onChange={(event) => {
              const protocol = event.target.value as ProxyProtocol;
              update({ protocol, port: protocol === "socks5" ? 1080 : 8080 });
            }}
          >
            <option value="http">HTTP</option>
            <option value="https">HTTPS</option>
            <option value="socks5">SOCKS5</option>
          </select>
        </Field>
        <Field label="Host">
          <input
            className="input mono"
            value={draft.host}
            onChange={(event) => update({ host: event.target.value.trim() })}
            placeholder="gate.example.com"
          />
        </Field>
        <Field label="Port">
          <input
            className="input mono"
            type="number"
            min={1}
            max={65535}
            value={draft.port}
            onChange={(event) => update({ port: Number(event.target.value) })}
          />
        </Field>
      </div>
      <div className="grid-2">
        <Field label="Username" hint="Optional. Leave empty for IP-authorised proxies.">
          <input
            className="input"
            value={draft.username}
            onChange={(event) => update({ username: event.target.value })}
            autoComplete="off"
          />
        </Field>
        <Field label="Password">
          <input
            className="input"
            type="password"
            value={draft.password}
            onChange={(event) => update({ password: event.target.value })}
            autoComplete="new-password"
          />
        </Field>
      </div>

      {error ? <div className="field-error">{error}</div> : null}
      {result ? (
        <div className="panel" style={{ background: result.ok ? "var(--ok-soft)" : "var(--danger-soft)" }}>
          <div className="row" style={{ alignItems: "flex-start", gap: 10 }}>
            <Icon
              name={result.ok ? "check" : "alert"}
              size={15}
              style={{ color: result.ok ? "var(--ok)" : "var(--danger)", marginTop: 2 }}
            />
            <div className="col" style={{ gap: 2 }}>
              <strong style={{ fontSize: 12.5 }}>
                {result.ok ? "Proxy is reachable" : "Proxy test failed"}
              </strong>
              <span className="text-dim" style={{ fontSize: 12 }}>
                {result.ok
                  ? `${result.latencyMs} ms${result.exitIp ? ` · exit IP ${result.exitIp}` : ""}${
                      result.country ? ` · ${result.country}` : ""
                    }`
                  : result.error}
              </span>
              {result.testedAt ? (
                <span className="text-faint" style={{ fontSize: 11 }}>
                  {formatDate(result.testedAt)}
                </span>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}
    </Modal>
  );
}
