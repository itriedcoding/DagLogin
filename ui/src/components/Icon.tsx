import type { CSSProperties } from "react";

export type IconName =
  | "profiles"
  | "proxy"
  | "activity"
  | "archive"
  | "settings"
  | "plus"
  | "search"
  | "play"
  | "stop"
  | "more"
  | "edit"
  | "copy"
  | "trash"
  | "folder"
  | "globe"
  | "clock"
  | "monitor"
  | "tag"
  | "note"
  | "chevronDown"
  | "chevronRight"
  | "x"
  | "check"
  | "alert"
  | "info"
  | "download"
  | "upload"
  | "refresh"
  | "minimize"
  | "maximize"
  | "close"
  | "cpu"
  | "memory"
  | "lock"
  | "shield"
  | "layers"
  | "terminal";

const paths: Record<IconName, JSX.Element> = {
  profiles: (
    <>
      <rect x="3" y="4" width="7" height="7" rx="1.6" />
      <rect x="14" y="4" width="7" height="7" rx="1.6" />
      <rect x="3" y="14" width="7" height="7" rx="1.6" />
      <rect x="14" y="14" width="7" height="7" rx="1.6" />
    </>
  ),
  proxy: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c2.5 2.4 3.8 5.6 3.8 9s-1.3 6.6-3.8 9c-2.5-2.4-3.8-5.6-3.8-9S9.5 5.4 12 3z" />
    </>
  ),
  activity: <path d="M3 12h3.5l2.5-6 4 12 2.5-6H21" />,
  archive: (
    <>
      <rect x="3" y="4" width="18" height="4.5" rx="1.4" />
      <path d="M4.6 8.5h14.8V19a1.5 1.5 0 0 1-1.5 1.5H6.1A1.5 1.5 0 0 1 4.6 19V8.5z" />
      <path d="M10 12.5h4" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M12 2.8v2.4M12 18.8v2.4M4.5 4.5l1.7 1.7M17.8 17.8l1.7 1.7M2.8 12h2.4M18.8 12h2.4M4.5 19.5l1.7-1.7M17.8 6.2l1.7-1.7" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  search: (
    <>
      <circle cx="11" cy="11" r="6.5" />
      <path d="M16 16l4.5 4.5" />
    </>
  ),
  play: <path d="M8 5.5l11 6.5-11 6.5z" />,
  stop: <rect x="6" y="6" width="12" height="12" rx="2" />,
  more: (
    <>
      <circle cx="12" cy="5.5" r="1.4" />
      <circle cx="12" cy="12" r="1.4" />
      <circle cx="12" cy="18.5" r="1.4" />
    </>
  ),
  edit: (
    <>
      <path d="M4 20h4l10.5-10.5a2.1 2.1 0 0 0-3-3L5 17v3z" />
      <path d="M14 5.5l4.5 4.5" />
    </>
  ),
  copy: (
    <>
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M15 9V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h3" />
    </>
  ),
  trash: (
    <>
      <path d="M4 7h16M9 7V5h6v2M6.5 7l.8 12.1A1.5 1.5 0 0 0 8.8 20.5h6.4a1.5 1.5 0 0 0 1.5-1.4L17.5 7" />
      <path d="M10 11v5M14 11v5" />
    </>
  ),
  folder: (
    <path d="M3.5 7.5A1.5 1.5 0 0 1 5 6h4l2 2.5h8A1.5 1.5 0 0 1 20.5 10v7.5A1.5 1.5 0 0 1 19 19H5a1.5 1.5 0 0 1-1.5-1.5z" />
  ),
  globe: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M3.5 12h17M12 3.5c2.3 2.3 3.5 5.3 3.5 8.5S14.3 18.2 12 20.5C9.7 18.2 8.5 15.2 8.5 12S9.7 5.8 12 3.5z" />
    </>
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5V12l3 2" />
    </>
  ),
  monitor: (
    <>
      <rect x="3" y="4.5" width="18" height="12" rx="2" />
      <path d="M9 20h6M12 16.5V20" />
    </>
  ),
  tag: (
    <>
      <path d="M12.6 3.6H20v7.4l-8.6 8.6a1.6 1.6 0 0 1-2.3 0l-5.1-5.1a1.6 1.6 0 0 1 0-2.3z" />
      <circle cx="16.4" cy="7.6" r="1.2" />
    </>
  ),
  note: (
    <>
      <path d="M5 4.5h9.5L19 9v10.5a1.5 1.5 0 0 1-1.5 1.5H5a1.5 1.5 0 0 1-1.5-1.5v-13A1.5 1.5 0 0 1 5 4.5z" />
      <path d="M14 4.5V9h4.5M7.5 13h8M7.5 16.5h5" />
    </>
  ),
  chevronDown: <path d="M6 9.5l6 6 6-6" />,
  chevronRight: <path d="M9.5 6l6 6-6 6" />,
  x: <path d="M6 6l12 12M18 6L6 18" />,
  check: <path d="M4.5 12.5l5 5 10-11" />,
  alert: (
    <>
      <path d="M12 3.8l8.5 15H3.5z" />
      <path d="M12 9.5v4M12 16.4v.1" />
    </>
  ),
  info: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 11v5.5M12 7.8v.1" />
    </>
  ),
  download: <path d="M12 3.5v11M7.5 10.5l4.5 4.5 4.5-4.5M4.5 19.5h15" />,
  upload: <path d="M12 20.5v-11M7.5 13.5L12 9l4.5 4.5M4.5 4.5h15" />,
  refresh: (
    <>
      <path d="M20 5.5v5h-5" />
      <path d="M19.2 10.5a7.5 7.5 0 1 0-2 7.2" />
    </>
  ),
  minimize: <path d="M5 12h14" />,
  maximize: <rect x="5.5" y="5.5" width="13" height="13" rx="1.6" />,
  close: <path d="M6 6l12 12M18 6L6 18" />,
  cpu: (
    <>
      <rect x="6" y="6" width="12" height="12" rx="2" />
      <path d="M10 3v3M14 3v3M10 18v3M14 18v3M3 10h3M3 14h3M18 10h3M18 14h3" />
    </>
  ),
  memory: (
    <>
      <rect x="3" y="7" width="18" height="10" rx="2" />
      <path d="M7 17v2.5M12 17v2.5M17 17v2.5M7 10.5h2M11 10.5h2M15 10.5h2" />
    </>
  ),
  lock: (
    <>
      <rect x="4.5" y="10" width="15" height="10.5" rx="2" />
      <path d="M8 10V7.5a4 4 0 0 1 8 0V10" />
    </>
  ),
  shield: (
    <>
      <path d="M12 3l7.5 3v6c0 4.3-3 7.9-7.5 9-4.5-1.1-7.5-4.7-7.5-9V6z" />
      <path d="M9 12l2.2 2.2L15.5 10" />
    </>
  ),
  layers: (
    <>
      <path d="M12 3.5l8.5 4.5-8.5 4.5L3.5 8z" />
      <path d="M3.5 12.5L12 17l8.5-4.5M3.5 16.8L12 21.3l8.5-4.5" />
    </>
  ),
  terminal: (
    <>
      <rect x="3" y="4.5" width="18" height="15" rx="2" />
      <path d="M7 10l2.5 2.5L7 15M12.5 15H17" />
    </>
  ),
};

export function Icon({
  name,
  size = 16,
  style,
  filled = false,
}: {
  name: IconName;
  size?: number;
  style?: CSSProperties;
  filled?: boolean;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      style={style}
    >
      {paths[name]}
    </svg>
  );
}
