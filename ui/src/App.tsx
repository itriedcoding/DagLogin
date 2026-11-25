import { useCallback, useEffect, useState } from "react";
import { Sidebar, type NavId } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { ConfirmDialog, ToastViewport, type ConfirmOptions } from "./components/ui";
import { Icon } from "./components/Icon";
import { ProfileEditor } from "./pages/ProfileEditor";
import { ProfilesPage } from "./pages/ProfilesPage";
import { ProxiesPage } from "./pages/ProxiesPage";
import { RunningPage } from "./pages/RunningPage";
import { BackupsPage } from "./pages/BackupsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { useAppData } from "./state/useAppData";
import type { Profile } from "./lib/types";

export default function App() {
  const data = useAppData();
  const [nav, setNav] = useState<NavId>("profiles");
  const [editor, setEditor] = useState<Profile | null>(null);
  const [confirm, setConfirm] = useState<ConfirmOptions | null>(null);

  // Reflect the motion preference on the document.
  useEffect(() => {
    const reduce = data.settings?.reduceAnimations ?? false;
    document.documentElement.dataset.reduceMotion = String(reduce);
  }, [data.settings?.reduceAnimations]);

  const openNewProfile = useCallback(async () => {
    const created = await data.createProfile();
    if (created) setEditor(created);
  }, [data]);

  const requestDelete = useCallback(
    (ids: string[], label: string) => {
      const remove = async () => {
        if (ids.length === 1) await data.deleteProfile(ids[0]);
        else await data.bulk(ids, "delete");
      };
      if (data.settings?.confirmOnDelete ?? true) {
        setConfirm({
          title: ids.length === 1 ? "Delete profile" : `Delete ${ids.length} profiles`,
          message: `${label} — this permanently removes the profile's cookies, storage, cache and history from this machine. It cannot be undone.`,
          confirmLabel: "Delete permanently",
          danger: true,
          onConfirm: remove,
        });
      } else {
        void remove();
      }
    },
    [data]
  );

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n" && !editor) {
        event.preventDefault();
        void openNewProfile();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [editor, openNewProfile]);

  if (data.fatal) {
    return (
      <div className="app">
        <TitleBar running={0} browser="Engine unavailable" />
        <div className="page">
          <div className="page-body" style={{ display: "grid", placeItems: "center" }}>
            <div className="empty" style={{ maxWidth: 620 }}>
              <div className="empty-art" style={{ borderColor: "rgba(255,96,114,.5)" }}>
                <Icon name="alert" size={24} />
              </div>
              <h3>Prism could not start its engine</h3>
              <p>{data.fatal}</p>
              <button className="btn btn-primary mt-8" onClick={() => void data.refreshAll()}>
                <Icon name="refresh" size={14} /> Try again
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <TitleBar
        running={data.running.length}
        browser={
          data.info?.browser.detected
            ? `${data.info.browser.name}${data.info.browser.version ? ` ${data.info.browser.version}` : ""}`
            : "No Chromium browser found"
        }
      />
      {data.busyIds.length > 0 ? <div className="progress-line" /> : null}
      <div className="shell">
        <Sidebar
          active={nav}
          onNavigate={setNav}
          profileCount={data.profiles.length}
          runningCount={data.running.length}
          proxyCount={data.proxies.length}
          storageBytes={data.info?.storageBytes ?? 0}
          version={data.info?.version ?? "1.0.0"}
          maxConcurrent={data.settings?.maxConcurrent ?? 0}
        />
        <main style={{ display: "flex", flex: 1, minWidth: 0 }}>
          {nav === "profiles" ? (
            <ProfilesPage
              data={data}
              onEdit={setEditor}
              onNew={() => void openNewProfile()}
              requestDelete={requestDelete}
            />
          ) : nav === "proxies" ? (
            <ProxiesPage data={data} />
          ) : nav === "running" ? (
            <RunningPage data={data} />
          ) : nav === "data" ? (
            <BackupsPage data={data} />
          ) : (
            <SettingsPage data={data} />
          )}
        </main>
      </div>

      {editor ? (
        <ProfileEditor data={data} profile={editor} onClose={() => setEditor(null)} />
      ) : null}

      {confirm ? (
        <ConfirmDialog options={confirm} onClose={() => setConfirm(null)} />
      ) : null}

      <ToastViewport toasts={data.toasts} onDismiss={data.dismiss} />
    </div>
  );
}
