import clsx from "clsx";
import { Check, ChevronDown, Copy, LoaderCircle, X } from "lucide-react";
import { copyText } from "../lib/api";
import { protocolLabel } from "../lib/types";
import type { ReactNode } from "react";
import {
  Children,
  isValidElement,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";

export function Button({
  children,
  variant = "primary",
  size = "md",
  className,
  type = "button",
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "sm" | "md";
}) {
  return (
    <button
      type={type}
      className={clsx("btn", `btn-${variant}`, `btn-${size}`, className)}
      {...props}
    >
      {children}
    </button>
  );
}

export function IconButton({
  label,
  children,
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button className={clsx("icon-btn", className)} aria-label={label} title={label} {...props}>
      {children}
    </button>
  );
}

export function Modal({
  open,
  title,
  eyebrow,
  children,
  onClose,
  wide = false,
}: {
  open: boolean;
  title: string;
  eyebrow?: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="modal-backdrop" onMouseDown={onClose} role="presentation">
      <div
        className={clsx("modal", wide && "modal-wide")}
        onMouseDown={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="modal-head">
          <div>
            {eyebrow && <div className="eyebrow">{eyebrow}</div>}
            <h2>{title}</h2>
          </div>
          <IconButton label="关闭" onClick={onClose}>
            <X size={18} />
          </IconButton>
        </div>
        <div className="modal-body">{children}</div>
      </div>
    </div>
  );
}

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={clsx("field", className)}>
      <span className="field-label">{label}</span>
      {children}
      {hint && <span className="field-hint">{hint}</span>}
    </div>
  );
}

export function Input(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input className="input" {...props} />;
}

type SelectOption = {
  value: string;
  label: ReactNode;
  disabled?: boolean;
};

export function Select({
  children,
  value,
  onChange,
  className,
  disabled = false,
  placeholder = "请选择",
}: {
  children: ReactNode;
  value: string;
  onChange: (value: string) => void;
  className?: string;
  disabled?: boolean;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [menuStyle, setMenuStyle] = useState<React.CSSProperties>({});
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const options: SelectOption[] = Children.toArray(children).flatMap((child) => {
    if (!isValidElement<{ value?: string; children?: ReactNode; disabled?: boolean }>(child)) {
      return [];
    }
    if (child.type !== "option") return [];
    return [{
      value: String(child.props.value ?? child.props.children ?? ""),
      label: child.props.children,
      disabled: child.props.disabled,
    }];
  });
  const selected = options.find((option) => option.value === value);

  function positionMenu() {
    const trigger = triggerRef.current;
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const below = window.innerHeight - rect.bottom;
    const above = rect.top;
    const openUp = below < 230 && above > below;
    const maxHeight = Math.min(300, Math.max(160, (openUp ? above : below) - 12));
    setMenuStyle({
      left: rect.left,
      width: rect.width,
      maxHeight,
      ...(openUp
        ? { bottom: window.innerHeight - rect.top + 6 }
        : { top: rect.bottom + 6 }),
    });
  }

  useEffect(() => {
    if (!open) return;
    positionMenu();
    const closeOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (triggerRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    const onViewportChange = () => positionMenu();
    document.addEventListener("mousedown", closeOutside);
    document.addEventListener("keydown", onKey);
    window.addEventListener("resize", onViewportChange);
    window.addEventListener("scroll", onViewportChange, true);
    return () => {
      document.removeEventListener("mousedown", closeOutside);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onViewportChange);
      window.removeEventListener("scroll", onViewportChange, true);
    };
  }, [open]);

  return (
    <div className={clsx("custom-select", open && "custom-select-open", className)}>
      <button
        ref={triggerRef}
        type="button"
        className="custom-select-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        disabled={disabled}
        onClick={() => {
          if (disabled) return;
          setOpen((current) => !current);
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            if (!open) {
              setOpen(true);
            }
          }
          if (event.key === "Escape") setOpen(false);
        }}
      >
        <span className={clsx("custom-select-value", !selected && "custom-select-placeholder")}>
          {selected?.label ?? placeholder}
        </span>
        <ChevronDown size={14} className="custom-select-chevron" />
      </button>
      {open &&
        createPortal(
          <div
            ref={menuRef}
            id={listId}
            className="custom-select-menu"
            role="listbox"
            style={menuStyle}
          >
            {options.map((option) => (
              <button
                key={`${option.value}-${String(option.label)}`}
                type="button"
                role="option"
                aria-selected={option.value === value}
                disabled={option.disabled}
                className={clsx(
                  "custom-select-option",
                  option.value === value && "custom-select-option-active",
                )}
                onClick={() => {
                  if (option.disabled) return;
                  onChange(option.value);
                  setOpen(false);
                  triggerRef.current?.focus();
                }}
              >
                <span>{option.label}</span>
                {option.value === value && <Check size={13} />}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: string;
}) {
  return (
    <button
      className={clsx("switch", checked && "switch-on")}
      role="switch"
      aria-checked={checked}
      aria-label={label ?? "toggle"}
      onClick={() => onChange(!checked)}
      type="button"
    >
      <span />
    </button>
  );
}

export function ProtocolBadge({ protocol }: { protocol: string }) {
  const kind =
    protocol === "responses" ? "coral" : protocol === "anthropic_messages" ? "gold" : "teal";
  return <span className={clsx("protocol-badge", `badge-${kind}`)}>{protocolLabel[protocol] ?? protocol}</span>;
}

export function StatusPill({
  tone = "neutral",
  children,
}: {
  tone?: "ok" | "warn" | "danger" | "neutral";
  children: ReactNode;
}) {
  return <span className={clsx("status-pill", `status-${tone}`)}>{children}</span>;
}

export function CopyButton({
  value,
  label = "复制",
  className,
}: {
  value: string;
  label?: string;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className={clsx("copy-btn", className)}
      onClick={async () => {
        await copyText(value);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1600);
      }}
      title={label}
    >
      {copied ? <Check size={14} /> : <Copy size={14} />}
      <span>{copied ? "已复制" : label}</span>
    </button>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon?: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty-state">
      <div className="empty-mark">{icon ?? "∅"}</div>
      <h3>{title}</h3>
      <p>{description}</p>
      {action}
    </div>
  );
}

export function Spinner({ label = "加载中" }: { label?: string }) {
  return (
    <div className="spinner">
      <LoaderCircle size={18} className="spin" />
      <span>{label}</span>
    </div>
  );
}

export function Notice({
  tone = "success",
  children,
}: {
  tone?: "success" | "danger" | "info";
  children: ReactNode;
}) {
  return <div className={clsx("notice", `notice-${tone}`)}>{children}</div>;
}

export function SectionHead({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="section-head">
      <div>
        {eyebrow && <div className="eyebrow">{eyebrow}</div>}
        <h1>{title}</h1>
        {description && <p>{description}</p>}
      </div>
      {actions && <div className="section-actions">{actions}</div>}
    </div>
  );
}

export function timeAgo(timestamp: number): string {
  const seconds = Math.max(0, Math.floor(Date.now() / 1000 - timestamp));
  if (seconds < 60) return `${seconds} 秒前`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  return `${Math.floor(seconds / 86400)} 天前`;
}

export function formatNumber(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}k`;
  return String(value);
}

export function formatMs(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  if (value >= 1000) return `${(value / 1000).toFixed(2)}s`;
  return `${Math.round(value)}ms`;
}
