import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage } from "../lib/api";
import type {
  AppInfo,
  EngineEvent,
  LaunchOutcome,
  Presets,
  Profile,
  ProfileQuery,
  ProfileView,
  Proxy,
  ProxyInput,
  RunningInstance,
  Settings,
} from "../lib/types";

export type ToastKind = "ok" | "error" | "info" | "warn";

export interface Toast {
  id: number;
  kind: ToastKind;
  message: string;
}

const defaultQuery: ProfileQuery = {
  search: "",
  status: "all",
  tag: "",
  proxyId: "",
  folder: "",
  sort: "name",
};

let toastSeq = 0;

/**
 * Single source of truth for the window: profiles, proxies, running browsers,
 * settings and toasts. Polls the engine for live process statistics and reacts
 * to engine events pushed from Rust.
 */
export function useAppData() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [presets, setPresets] = useState<Presets | null>(null);
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [running, setRunning] = useState<RunningInstance[]>([]);
  const [query, setQuery] = useState<ProfileQuery>(defaultQuery);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyIds, setBusyIds] = useState<string[]>([]);
  const [fatal, setFatal] = useState<string | null>(null);
  const queryRef = useRef(query);
  queryRef.current = query;

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const toast = useCallback(
    (kind: ToastKind, message: string) => {
      toastSeq += 1;
      const id = toastSeq;
      setToasts((current) => [...current.slice(-3), { id, kind, message }]);
      const timeout = kind === "error" ? 9000 : 4500;
      window.setTimeout(() => dismiss(id), timeout);
    },
    [dismiss]
  );

  const refreshProfiles = useCallback(async () => {
    const list = await api.listProfiles(queryRef.current);
    setProfiles(list);
  }, []);

  const refreshProxies = useCallback(async () => {
    setProxies(await api.proxies());
  }, []);

  const refreshRunning = useCallback(async () => {
    setRunning(await api.running());
  }, []);

  const refreshInfo = useCallback(async () => {
    setInfo(await api.appInfo());
  }, []);

  const refreshAll = useCallback(async () => {
    try {
      const [appInfo, appSettings, appPresets, profileList, proxyList, runningList] =
        await Promise.all([
          api.appInfo(),
          api.settings(),
          api.presets(),
          api.listProfiles(queryRef.current),
          api.proxies(),
          api.running(),
        ]);
      setInfo(appInfo);
      setSettings(appSettings);
      setPresets(appPresets);
      setProfiles(profileList);
      setProxies(proxyList);
      setRunning(runningList);
      setFatal(null);
    } catch (error) {
      setFatal(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshAll();
  }, [refreshAll]);

  // Re-query whenever the filter changes.
  useEffect(() => {
    if (loading) return;
    let cancelled = false;
    api
      .listProfiles(query)
      .then((list) => {
        if (!cancelled) setProfiles(list);
      })
      .catch((error) => toast("error", errorMessage(error)));
    return () => {
      cancelled = true;
    };
  }, [query, loading, toast]);

  // Live process statistics.
  useEffect(() => {
    const timer = window.setInterval(() => {
      api
        .running()
        .then(setRunning)
        .catch(() => undefined);
    }, 3000);
    return () => window.clearInterval(timer);
  }, []);

  // Engine events (profile started/stopped, warnings).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<EngineEvent>("engine://event", (event) => {
      const payload = event.payload;
      if (payload.type === "profileStarted") {
        toast("ok", `${payload.name} launched`);
      } else if (payload.type === "profileStopped") {
        const detail =
          payload.error ??
          (payload.exitCode && payload.exitCode !== 0
            ? `exit code ${payload.exitCode}`
            : null);
        toast(detail ? "warn" : "info", detail ? `${payload.name} closed (${detail})` : `${payload.name} closed`);
      } else if (payload.type === "notice") {
        toast(payload.level === "error" ? "error" : "info", payload.message);
      }
      void refreshRunning();
      void refreshProfiles();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [toast, refreshProfiles, refreshRunning]);

  const withBusy = useCallback(
    async <T,>(id: string, action: () => Promise<T>): Promise<T | undefined> => {
      setBusyIds((current) => [...current, id]);
      try {
        return await action();
      } catch (error) {
        toast("error", errorMessage(error));
        return undefined;
      } finally {
        setBusyIds((current) => current.filter((value) => value !== id));
      }
    },
    [toast]
  );

  const createProfile = useCallback(
    async (name?: string) => {
      const profile = await withBusy("new", () => api.createProfile(name));
      if (profile) {
        toast("ok", `Profile “${profile.name}” created`);
        await refreshProfiles();
        await refreshInfo();
      }
      return profile;
    },
    [refreshInfo, refreshProfiles, toast, withBusy]
  );

  const saveProfile = useCallback(
    async (profile: Profile) => {
      const saved = await withBusy(profile.id, () => api.updateProfile(profile));
      if (saved) {
        toast("ok", "Profile saved");
        await refreshProfiles();
      }
      return saved;
    },
    [refreshProfiles, toast, withBusy]
  );

  const duplicateProfile = useCallback(
    async (id: string, withData: boolean) => {
      const copy = await withBusy(id, () => api.duplicateProfile(id, withData));
      if (copy) {
        toast("ok", `Duplicated as “${copy.name}”`);
        await refreshProfiles();
        await refreshInfo();
      }
      return copy;
    },
    [refreshInfo, refreshProfiles, toast, withBusy]
  );

  const deleteProfile = useCallback(
    async (id: string) => {
      const done = await withBusy(id, () => api.deleteProfile(id));
      if (done !== undefined) {
        toast("ok", "Profile deleted");
        await refreshProfiles();
        await refreshRunning();
        await refreshInfo();
      }
    },
    [refreshInfo, refreshProfiles, refreshRunning, toast, withBusy]
  );

  const archiveProfile = useCallback(
    async (id: string, archived: boolean) => {
      const result = await withBusy(id, () => api.setArchived(id, archived));
      if (result) {
        toast("ok", archived ? "Profile archived" : "Profile restored");
        await refreshProfiles();
      }
    },
    [refreshProfiles, toast, withBusy]
  );

  const launch = useCallback(
    async (id: string): Promise<LaunchOutcome | undefined> => {
      const outcome = await withBusy(id, () => api.launch(id));
      if (outcome) {
        outcome.warnings.forEach((warning) => toast("warn", warning));
        await refreshRunning();
        await refreshProfiles();
      }
      return outcome;
    },
    [refreshProfiles, refreshRunning, toast, withBusy]
  );

  const stop = useCallback(
    async (id: string) => {
      const done = await withBusy(id, () => api.stop(id));
      if (done !== undefined) {
        await refreshRunning();
        await refreshProfiles();
      }
    },
    [refreshProfiles, refreshRunning, withBusy]
  );

  const stopAll = useCallback(async () => {
    try {
      const count = await api.stopAll();
      toast("info", count === 0 ? "No profiles were running" : `Stopped ${count} profile(s)`);
      await refreshRunning();
      await refreshProfiles();
    } catch (error) {
      toast("error", errorMessage(error));
    }
  }, [refreshProfiles, refreshRunning, toast]);

  const saveSettings = useCallback(
    async (next: Settings) => {
      try {
        const saved = await api.saveSettings(next);
        setSettings(saved);
        toast("ok", "Settings saved");
        await refreshInfo();
        return saved;
      } catch (error) {
        toast("error", errorMessage(error));
        return undefined;
      }
    },
    [refreshInfo, toast]
  );

  const saveProxy = useCallback(
    async (input: ProxyInput) => {
      try {
        const saved = await api.saveProxy(input);
        setProxies(await api.proxies());
        toast("ok", input.id ? "Proxy updated" : "Proxy added");
        return saved;
      } catch (error) {
        toast("error", errorMessage(error));
        return undefined;
      }
    },
    [toast]
  );

  const deleteProxy = useCallback(
    async (id: string) => {
      try {
        await api.deleteProxy(id);
        setProxies(await api.proxies());
        await refreshProfiles();
        toast("ok", "Proxy deleted");
      } catch (error) {
        toast("error", errorMessage(error));
      }
    },
    [refreshProfiles, toast]
  );

  const bulk = useCallback(
    async (ids: string[], action: string) => {
      try {
        const count = await api.bulkAction(ids, action);
        toast("ok", `${count} profile(s) updated`);
        await refreshProfiles();
        await refreshRunning();
        await refreshInfo();
      } catch (error) {
        toast("error", errorMessage(error));
      }
    },
    [refreshInfo, refreshProfiles, refreshRunning, toast]
  );

  const runningIds = useMemo(() => new Set(running.map((r) => r.profileId)), [running]);
  const tags = useMemo(() => {
    const set = new Set<string>();
    profiles.forEach((profile) => profile.tags.forEach((tag) => set.add(tag)));
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [profiles]);

  return {
    info,
    settings,
    presets,
    profiles,
    proxies,
    running,
    runningIds,
    tags,
    query,
    setQuery,
    toasts,
    toast,
    dismiss,
    loading,
    fatal,
    busyIds,
    refreshAll,
    refreshProfiles,
    refreshProxies,
    refreshRunning,
    refreshInfo,
    createProfile,
    saveProfile,
    duplicateProfile,
    deleteProfile,
    archiveProfile,
    launch,
    stop,
    stopAll,
    saveSettings,
    saveProxy,
    deleteProxy,
    bulk,
  };
}

export type AppData = ReturnType<typeof useAppData>;
