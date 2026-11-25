import { useMemo, useState, type CSSProperties } from "react";
import type { AppData } from "../state/useAppData";
import type { MenuItem } from "../components/ui";
import type { Profile, ProfileView } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import { pickSavePath, stampedName } from "../lib/files";
import { colorFor, formatBytes, initials, protocolLabel, timeAgo } from "../lib/format";
import { Icon } from "../components/Icon";
import { ContextMenu, EmptyState } from "../components/ui";

type ViewMode = "grid" | "table";

export function ProfilesPage({
  data,
  onEdit,
  onNew,
  requestDelete,
}: {
  data: AppData;
  onEdit: (profile: Profile) => void;
  onNew: () => void;
  requestDelete: (ids: string[], label: string) => void;
}) {
  const [view, setView] = useState<ViewMode>("grid");
  const [selected, setSelected] = useState<string[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number; profile: ProfileView } | null>(null);

  const { query, setQuery, profiles, proxies, tags, runningIds, busyIds, loading } = data;
  const hasFilters =
    query.search.trim() !== "" || query.status !== "all" || query.tag !== "" || query.proxyId !== "";

  const toggleSelected = (id: string) => {
    setSelected((current) =>
      current.includes(id) ? current.filter((value) => value !== id) : [...current, id]
    );
  };

  const exportOne = async (profile: ProfileView) => {
    const path = await pickSavePath(stampedName(`prism-${profile.name.replace(/\s+/g, "-").toLowerCase()}`, "prism"));
    if (!path) return;
    try {
      const report = await api.exportConfig(path, [profile.id], null);
      data.toast("ok", `Exported ${report.profiles} profile to ${report.path}`);
    } catch (error) {
      data.toast("error", errorMessage(error));
    }
  };

  const openFolder = async (profile: ProfileView) => {
    try {
      const dir = await api.profileDataDir(profile.id);
      await api.openPath(dir, false);
    } catch (error) {
      data.toast("error", errorMessage(error));
    }
  };

  const menuItems = (profile: ProfileView): MenuItem[] => {
    const running = runningIds.has(profile.id);
    const archived = profile.status === "archived";
    return [
      {
        label: running ? "Stop browser" : "Launch browser",
        icon: running ? "stop" : "play",
        onSelect: () => (running ? data.stop(profile.id) : data.launch(profile.id)),
      },
      { label: "Edit profile", icon: "edit", onSelect: () => onEdit(profile) },
      { separator: true, label: "" },
      {
        label: "Duplicate",
        icon: "copy",
        onSelect: () => void data.duplicateProfile(profile.id, false),
      },
      {
        label: "Duplicate with browser data",
        icon: "layers",
        onSelect: () => void data.duplicateProfile(profile.id, true),
      },
      { label: "Open data folder", icon: "folder", onSelect: () => void openFolder(profile) },
      { label: "Export configuration", icon: "download", onSelect: () => void exportOne(profile) },
      { separator: true, label: "" },
      {
        label: archived ? "Restore from archive" : "Archive profile",
        icon: "archive",
        onSelect: () => void data.archiveProfile(profile.id, !archived),
      },
      {
        label: "Delete profile",
        icon: "trash",
        danger: true,
        onSelect: () => requestDelete([profile.id], `“${profile.name}” will be deleted`),
      },
    ];
  };

  const allSelected = profiles.length > 0 && selected.length === profiles.length;

  const bulkBar = selected.length > 0 && (
    <div className="row wrap gap-12" style={{ marginBottom: 8 }}>
      <span className="text-dim">{selected.length} selected</span>
      <button className="btn btn-sm" onClick={() => void data.bulk(selected, "archive")}>
        <Icon name="archive" size={13} /> Archive
      </button>
      <button className="btn btn-sm" onClick={() => void data.bulk(selected, "unarchive")}>
        <Icon name="layers" size={13} /> Restore
      </button>
      <button className="btn btn-sm" onClick={() => void data.bulk(selected, "stop")}>
        <Icon name="stop" size={13} /> Stop
      </button>
      <button
        className="btn btn-sm btn-danger"
        onClick={() => {
          requestDelete(selected, `${selected.length} profiles will be deleted`);
          setSelected([]);
        }}
      >
        <Icon name="trash" size={13} /> Delete
      </button>
      <button className="btn btn-sm btn-ghost" onClick={() => setSelected([])}>
        Clear
      </button>
    </div>
  );

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1 className="page-title">Profiles</h1>
          <p className="page-subtitle">
            Every profile is a separate browser instance with its own cookies, storage, cache and
            session. Launch as many as you need — there is no profile limit.
          </p>
        </div>
        <div className="page-header-actions">
          <button className="btn btn-ghost" onClick={() => void data.refreshProfiles()}>
            <Icon name="refresh" size={14} /> Refresh
          </button>
          <button className="btn btn-primary" onClick={onNew}>
            <Icon name="plus" size={15} /> New profile
          </button>
        </div>
      </div>

      <div className="toolbar">
        <div className="search">
          <Icon name="search" size={14} />
          <input
            placeholder="Search name, notes or tags…"
            value={query.search}
            onChange={(event) => setQuery({ ...query, search: event.target.value })}
            aria-label="Search profiles"
          />
        </div>
        {(["all", "active", "archived"] as const).map((status) => (
          <button
            key={status}
            className={`filter-chip ${query.status === status ? "active" : ""}`}
            onClick={() => setQuery({ ...query, status })}
          >
            {status === "all" ? "All" : status === "active" ? "Active" : "Archived"}
          </button>
        ))}
        <select
          className="select"
          style={{ width: 150 }}
          value={query.tag}
          onChange={(event) => setQuery({ ...query, tag: event.target.value })}
          aria-label="Filter by tag"
        >
          <option value="">All tags</option>
          {tags.map((tag) => (
            <option key={tag} value={tag}>
              {tag}
            </option>
          ))}
        </select>
        <select
          className="select"
          style={{ width: 160 }}
          value={query.proxyId}
          onChange={(event) => setQuery({ ...query, proxyId: event.target.value })}
          aria-label="Filter by proxy"
        >
          <option value="">All proxies</option>
          <option value="none">No proxy</option>
          {proxies.map((proxy) => (
            <option key={proxy.id} value={proxy.id}>
              {proxy.label}
            </option>
          ))}
        </select>
        <select
          className="select"
          style={{ width: 150 }}
          value={query.sort}
          onChange={(event) => setQuery({ ...query, sort: event.target.value })}
          aria-label="Sort profiles"
        >
          <option value="name">Name A–Z</option>
          <option value="created">Newest first</option>
          <option value="lastLaunch">Recently launched</option>
          <option value="launchCount">Most launched</option>
        </select>
        <div className="segmented" style={{ marginLeft: "auto" }}>
          <button
            className={view === "grid" ? "active" : ""}
            onClick={() => setView("grid")}
            aria-label="Grid view"
          >
            <Icon name="profiles" size={14} />
          </button>
          <button
            className={view === "table" ? "active" : ""}
            onClick={() => setView("table")}
            aria-label="Table view"
          >
            <Icon name="layers" size={14} />
          </button>
        </div>
      </div>

      <div className="page-body">
        {bulkBar}

        {loading ? (
          <div className="profile-grid">
            {[0, 1, 2, 3, 4, 5].map((index) => (
              <div className="skeleton" key={index} />
            ))}
          </div>
        ) : profiles.length === 0 ? (
          <EmptyState
            icon="profiles"
            title={hasFilters ? "No profiles match these filters" : "Create your first profile"}
            description={
              hasFilters
                ? "Try clearing the search, tag or proxy filter. Archived profiles are hidden when the Active filter is selected."
                : "A profile is a sealed browser: its own cookies, local storage, cache, history and session. Give it a name, assign a proxy and launch it."
            }
            action={
              hasFilters ? (
                <button
                  className="btn"
                  onClick={() =>
                    setQuery({ ...query, search: "", status: "all", tag: "", proxyId: "" })
                  }
                >
                  Clear filters
                </button>
              ) : (
                <button className="btn btn-primary" onClick={onNew}>
                  <Icon name="plus" size={15} /> New profile
                </button>
              )
            }
          />
        ) : view === "grid" ? (
          <div className="profile-grid">
            {profiles.map((profile) => {
              const running = runningIds.has(profile.id);
              const busy = busyIds.includes(profile.id);
              const accent = profile.color || colorFor(profile.id);
              return (
                <article
                  key={profile.id}
                  className={`profile-card ${running ? "running" : ""} ${
                    selected.includes(profile.id) ? "selected" : ""
                  }`}
                  style={{ "--card-accent": accent } as CSSProperties}
                  onClick={() => onEdit(profile)}
                >
                  <div className="profile-card-head">
                    <div className="avatar" style={{ background: `linear-gradient(150deg, ${accent}, ${accent}99)` }}>
                      {initials(profile.name)}
                    </div>
                    <div className="grow">
                      <div className="profile-name" title={profile.name}>
                        {profile.name}
                      </div>
                      <div className="profile-meta">
                        <span className={`status-dot ${running ? "running" : profile.status === "archived" ? "archived" : "idle"}`} />
                        <span>
                          {running
                            ? "Running"
                            : profile.status === "archived"
                            ? "Archived"
                            : `Launched ${profile.launchCount}×`}
                        </span>
                        <span>·</span>
                        <span>{timeAgo(profile.lastLaunchedAt)}</span>
                      </div>
                    </div>
                    <button
                      className="icon-button"
                      aria-label="Profile actions"
                      onClick={(event) => {
                        event.stopPropagation();
                        setMenu({ x: event.clientX, y: event.clientY, profile });
                      }}
                    >
                      <Icon name="more" size={15} />
                    </button>
                  </div>

                  <div className="row wrap" style={{ gap: 5 }}>
                    <span className={`tag ${profile.proxyLabel ? "accent" : ""}`}>
                      <Icon name="proxy" size={11} />
                      {profile.proxyLabel ?? "Direct connection"}
                    </span>
                    {profile.tags.slice(0, 3).map((tag) => (
                      <span className="tag" key={tag}>
                        {tag}
                      </span>
                    ))}
                    {profile.tags.length > 3 ? (
                      <span className="tag">+{profile.tags.length - 3}</span>
                    ) : null}
                  </div>

                  <div className="profile-facts">
                    <div className="fact" title={profile.fingerprint.timezone}>
                      <Icon name="clock" size={12} />
                      <span>{profile.fingerprint.timezone.split("/").pop()?.replace(/_/g, " ")}</span>
                    </div>
                    <div className="fact" title={profile.fingerprint.locale}>
                      <Icon name="globe" size={12} />
                      <span>{profile.fingerprint.locale}</span>
                    </div>
                    <div className="fact">
                      <Icon name="monitor" size={12} />
                      <span>
                        {profile.fingerprint.screen.width}×{profile.fingerprint.screen.height}
                      </span>
                    </div>
                    <div className="fact">
                      <Icon name="folder" size={12} />
                      <span>{formatBytes(profile.storageBytes)}</span>
                    </div>
                  </div>

                  {profile.notes.trim() ? <div className="notes-preview">{profile.notes}</div> : null}

                  <div className="profile-card-foot">
                    <button
                      className={`btn btn-sm ${running ? "" : "btn-primary"}`}
                      disabled={busy || profile.status === "archived"}
                      onClick={(event) => {
                        event.stopPropagation();
                        void (running ? data.stop(profile.id) : data.launch(profile.id));
                      }}
                    >
                      {busy ? <span className="spinner" /> : <Icon name={running ? "stop" : "play"} size={13} />}
                      {running ? "Stop" : "Launch"}
                    </button>
                    <button
                      className="btn btn-sm btn-ghost"
                      onClick={(event) => {
                        event.stopPropagation();
                        onEdit(profile);
                      }}
                    >
                      <Icon name="edit" size={13} /> Edit
                    </button>
                    <span className="grow" />
                    <span className="text-faint num" style={{ fontSize: 11 }}>
                      {protocolLabel(profile.proxyProtocol)}
                    </span>
                  </div>
                </article>
              );
            })}
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th style={{ width: 34 }}>
                    <input
                      type="checkbox"
                      checked={allSelected}
                      onChange={() => setSelected(allSelected ? [] : profiles.map((p) => p.id))}
                      aria-label="Select all profiles"
                    />
                  </th>
                  <th>Profile</th>
                  <th>Proxy</th>
                  <th>Timezone</th>
                  <th>Locale</th>
                  <th>Storage</th>
                  <th>Last launch</th>
                  <th style={{ width: 120 }} />
                </tr>
              </thead>
              <tbody>
                {profiles.map((profile) => {
                  const running = runningIds.has(profile.id);
                  const busy = busyIds.includes(profile.id);
                  return (
                    <tr key={profile.id} className={selected.includes(profile.id) ? "selected" : ""}>
                      <td>
                        <input
                          type="checkbox"
                          checked={selected.includes(profile.id)}
                          onChange={() => toggleSelected(profile.id)}
                          aria-label={`Select ${profile.name}`}
                        />
                      </td>
                      <td>
                        <div className="row gap-4">
                          <span
                            className={`status-dot ${running ? "running" : profile.status === "archived" ? "archived" : "idle"}`}
                          />
                          <div className="col" style={{ gap: 0 }}>
                            <span style={{ fontWeight: 550 }}>{profile.name}</span>
                            <span className="text-faint" style={{ fontSize: 11 }}>
                              {profile.tags.join(" · ") || "no tags"}
                            </span>
                          </div>
                        </div>
                      </td>
                      <td className="text-dim truncate">{profile.proxyLabel ?? "Direct"}</td>
                      <td className="text-dim">{profile.fingerprint.timezone}</td>
                      <td className="text-dim">{profile.fingerprint.locale}</td>
                      <td className="num text-dim">{formatBytes(profile.storageBytes)}</td>
                      <td className="text-dim">{timeAgo(profile.lastLaunchedAt)}</td>
                      <td>
                        <div className="row-actions">
                          <button
                            className="icon-button"
                            disabled={busy || profile.status === "archived"}
                            onClick={() => void (running ? data.stop(profile.id) : data.launch(profile.id))}
                            aria-label={running ? "Stop" : "Launch"}
                          >
                            {busy ? <span className="spinner" /> : <Icon name={running ? "stop" : "play"} size={14} />}
                          </button>
                          <button className="icon-button" onClick={() => onEdit(profile)} aria-label="Edit">
                            <Icon name="edit" size={14} />
                          </button>
                          <button
                            className="icon-button"
                            onClick={(event) => setMenu({ x: event.clientX, y: event.clientY, profile })}
                            aria-label="More actions"
                          >
                            <Icon name="more" size={14} />
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
      </div>

      {menu ? (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.profile)} onClose={() => setMenu(null)} />
      ) : null}
    </div>
  );
}

export function useProfileCounts(profiles: ProfileView[]) {
  return useMemo(
    () => ({
      total: profiles.length,
      archived: profiles.filter((profile) => profile.status === "archived").length,
    }),
    [profiles]
  );
}
