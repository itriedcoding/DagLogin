import { useEffect, useRef, type ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import type { Toast } from "../state/useAppData";

/* ------------------------------------------------------------------ fields */

export function Field({
  label,
  hint,
  error,
  children,
  right,
}: {
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
  right?: ReactNode;
}) {
  return (
    <label className="field">
      <span className="field-label">
        {label}
        {right ? <span style={{ marginLeft: "auto" }}>{right}</span> : null}
      </span>
      {children}
      {hint ? <span className="field-hint">{hint}</span> : null}
      {error ? <span className="field-error">{error}</span> : null}
    </label>
  );
}

export function Switch({
  checked,
  onChange,
  disabled,
  label,
  description,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  label?: string;
  description?: string;
}) {
  const control = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label ?? "Toggle"}
      className={`switch ${checked ? "on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    />
  );
  if (!label) return control;
  return (
    <div className="switch-row">
      <div className="switch-copy">
        <strong>{label}</strong>
        {description ? <p>{description}</p> : null}
      </div>
      {control}
    </div>
  );
}

export function Tabs<T extends string>({
  tabs,
  value,
  onChange,
}: {
  tabs: { id: T; label: string }[];
  value: T;
  onChange: (id: T) => void;
}) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={tab.id === value}
          className={tab.id === value ? "active" : ""}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  );
}

export function ChipsInput({
  values,
  onChange,
  placeholder,
}: {
  values: string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const commit = (raw: string) => {
    const parts = raw
      .split(/[,;]/)
      .map((value) => value.trim())
      .filter(Boolean);
    if (parts.length === 0) return;
    const next = [...values];
    parts.forEach((part) => {
      if (!next.includes(part)) next.push(part);
    });
    onChange(next);
  };
  return (
    <div className="chips-input" onClick={() => inputRef.current?.focus()}>
      {values.map((value) => (
        <span className="chip" key={value}>
          {value}
          <button
            type="button"
            aria-label={`Remove ${value}`}
            onClick={(event) => {
              event.stopPropagation();
              onChange(values.filter((item) => item !== value));
            }}
          >
            <Icon name="x" size={11} />
          </button>
        </span>
      ))}
      <input
        ref={inputRef}
        placeholder={values.length === 0 ? placeholder : ""}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === ",") {
            event.preventDefault();
            commit(event.currentTarget.value);
            event.currentTarget.value = "";
          } else if (event.key === "Backspace" && !event.currentTarget.value && values.length) {
            onChange(values.slice(0, -1));
          }
        }}
        onBlur={(event) => {
          commit(event.currentTarget.value);
          event.currentTarget.value = "";
        }}
      />
    </div>
  );
}

export function Stat({
  label,
  value,
  note,
}: {
  label: string;
  value: ReactNode;
  note?: string;
}) {
  return (
    <div className="stat">
      <div className="stat-label">{label}</div>
      <div className="stat-value">{value}</div>
      {note ? <div className="stat-note">{note}</div> : null}
    </div>
  );
}

export function Panel({
  title,
  description,
  actions,
  children,
}: {
  title?: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="panel">
      {title ? (
        <div className="row between" style={{ marginBottom: description ? 0 : 12 }}>
          <h2 className="panel-title">{title}</h2>
          {actions}
        </div>
      ) : null}
      {description ? <p className="panel-desc">{description}</p> : null}
      {children}
    </section>
  );
}

export function EmptyState({
  icon = "layers",
  title,
  description,
  action,
}: {
  icon?: IconName;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <div className="empty-art">
        <Icon name={icon} size={24} />
      </div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action ? <div className="mt-8">{action}</div> : null}
    </div>
  );
}

/* ----------------------------------------------------------------- overlay */

export function Modal({
  title,
  description,
  children,
  footer,
  onClose,
  wide,
}: {
  title: string;
  description?: string;
  children?: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <>
      <div className="modal-backdrop" onClick={onClose} />
      <div className={`modal ${wide ? "wide" : ""}`} role="dialog" aria-modal="true">
        <div className="modal-header">
          <div className="grow">
            <h2 className="modal-title">{title}</h2>
            {description ? <p className="modal-desc">{description}</p> : null}
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close dialog">
            <Icon name="x" size={15} />
          </button>
        </div>
        {children ? <div className="modal-body">{children}</div> : null}
        {footer ? <div className="modal-footer">{footer}</div> : null}
      </div>
    </>
  );
}

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  onConfirm: () => void | Promise<void>;
}

export function ConfirmDialog({
  options,
  onClose,
  busy,
}: {
  options: ConfirmOptions;
  onClose: () => void;
  busy?: boolean;
}) {
  return (
    <Modal
      title={options.title}
      description={options.message}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button
            className={`btn ${options.danger ? "btn-danger" : "btn-primary"}`}
            disabled={busy}
            onClick={async () => {
              await options.onConfirm();
              onClose();
            }}
          >
            {busy ? <span className="spinner" /> : null}
            {options.confirmLabel ?? "Confirm"}
          </button>
        </>
      }
    />
  );
}

/* ------------------------------------------------------------------- menus */

export interface MenuItem {
  label: string;
  icon?: IconName;
  onSelect?: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  heading?: string;
}

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const width = 220;
  const height = items.length * 31 + 12;
  const left = Math.min(x, window.innerWidth - width - 12);
  const top = Math.min(y, window.innerHeight - height - 12);

  return (
    <div className="context-menu" ref={ref} style={{ left, top }}>
      {items.map((item, index) =>
        item.separator ? (
          <div className="separator" key={`sep-${index}`} />
        ) : item.heading ? (
          <div className="menu-label" key={`head-${index}`}>
            {item.heading}
          </div>
        ) : (
          <button
            key={`${item.label}-${index}`}
            className={item.danger ? "danger" : ""}
            disabled={item.disabled}
            onClick={() => {
              item.onSelect?.();
              onClose();
            }}
          >
            {item.icon ? <Icon name={item.icon} size={14} /> : <span style={{ width: 14 }} />}
            {item.label}
          </button>
        )
      )}
    </div>
  );
}

/* ------------------------------------------------------------------ toasts */

const toastIcon: Record<Toast["kind"], IconName> = {
  ok: "check",
  error: "alert",
  info: "info",
  warn: "alert",
};

const toastColor: Record<Toast["kind"], string> = {
  ok: "var(--ok)",
  error: "var(--danger)",
  info: "var(--accent)",
  warn: "var(--warn)",
};

export function ToastViewport({
  toasts,
  onDismiss,
}: {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}) {
  return (
    <div className="toasts">
      {toasts.map((toast) => (
        <div className={`toast ${toast.kind}`} key={toast.id} role="status">
          <span className="toast-icon" style={{ color: toastColor[toast.kind] }}>
            <Icon name={toastIcon[toast.kind]} size={15} />
          </span>
          <span className="grow">{toast.message}</span>
          <button className="icon-button" onClick={() => onDismiss(toast.id)} aria-label="Dismiss">
            <Icon name="x" size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
