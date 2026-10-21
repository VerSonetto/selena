import { useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import {
  Boxes,
  ChartNoAxesCombined,
  Command,
  Copy,
  KeyRound,
  LayoutDashboard,
  Menu,
  Route,
  ScrollText,
  Settings,
  Sparkles,
  TerminalSquare,
  Waypoints,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { api, copyText, gatewayUrl, getToken, clearToken } from "../lib/api";
import { ProtocolBadge, StatusPill } from "./ui";
import { TitleBar } from "./TitleBar";

const navItems = [
  { path: "/", label: "概览", eyebrow: "Overview", icon: LayoutDashboard },
  { path: "/providers", label: "Providers", eyebrow: "Upstreams", icon: Waypoints },
  { path: "/models", label: "模型池", eyebrow: "Models", icon: Boxes },
  { path: "/routes", label: "路由", eyebrow: "Routing", icon: Route },
  { path: "/keys", label: "客户端密钥", eyebrow: "Access", icon: KeyRound },
  { path: "/playground", label: "Playground", eyebrow: "Protocol lab", icon: TerminalSquare },
  { path: "/logs", label: "日志", eyebrow: "Traces", icon: ScrollText },
  { path: "/analytics", label: "分析", eyebrow: "Usage", icon: ChartNoAxesCombined },
  { path: "/settings", label: "设置", eyebrow: "Settings", icon: Settings },
];

export function Shell() {
  const location = useLocation();
  const navigate = useNavigate();
  const [mobileOpen, setMobileOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [copied, setCopied] = useState(false);
  const { data: overview, error } = useQuery({
    queryKey: ["overview"],
    queryFn: api.overview,
    refetchInterval: 5000,
    retry: 1,
  });

  const current = navItems.find((item) =>
    item.path === "/" ? location.pathname === "/" : location.pathname.startsWith(item.path),
  );

  useEffect(() => {
    setMobileOpen(false);
    setPaletteOpen(false);
  }, [location.pathname]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const paletteItems = useMemo(() => {
    const query = paletteQuery.trim().toLowerCase();
    return navItems.filter(
      (item) =>
        !query ||
        item.label.toLowerCase().includes(query) ||
        item.eyebrow.toLowerCase().includes(query),
    );
  }, [paletteQuery]);

  const endpoint = gatewayUrl("/v1/chat/completions");
  const hasToken = Boolean(getToken());

  return (
    <div className="app-frame">
      <TitleBar />
      <div className="app-shell">
        <aside className={clsx("sidebar", mobileOpen && "sidebar-open")}>
          <div className="sidebar-top">
            <div className="brand">
              <div className="brand-mark">
                <span>S</span>
                <i />
              </div>
              <div>
                <div className="brand-name">Selena</div>
                <div className="brand-sub">LOCAL API GATEWAY</div>
              </div>
            </div>
            <button className="sidebar-close" onClick={() => setMobileOpen(false)} aria-label="关闭菜单">
              <X size={18} />
            </button>
          </div>

          <div className="sidebar-section-label">CONTROL ROOM</div>
          <nav className="sidebar-nav">
            {navItems.map((item) => {
              const Icon = item.icon;
              return (
                <NavLink
                  key={item.path}
                  to={item.path}
                  end={item.path === "/"}
                  className={({ isActive }) => clsx("nav-item", isActive && "nav-active")}
                >
                  <Icon size={17} strokeWidth={1.8} />
                  <span>{item.label}</span>
                  <em>{item.eyebrow}</em>
                </NavLink>
              );
            })}
          </nav>

          <div className="sidebar-note">
            <div className="sidebar-note-head">
              <Sparkles size={14} />
              <span>协议入口</span>
            </div>
            <p>Chat、Responses、Messages 均可作为入口和上游。同协议透传，跨协议转换。</p>
            <div className="sidebar-protocols">
              <ProtocolBadge protocol="chat_completions" />
              <span className="protocol-arrow">→</span>
              <ProtocolBadge protocol="responses" />
              <span className="protocol-arrow">→</span>
              <ProtocolBadge protocol="anthropic_messages" />
            </div>
          </div>

          <div className="sidebar-footer">
            <div className="service-state">
              <span className={clsx("pulse-dot", error ? "pulse-danger" : "pulse-ok")} />
              <div>
                <strong>{error ? "服务离线" : "本地服务运行中"}</strong>
                <span>{overview ? `${overview.host}:${overview.port}` : "127.0.0.1:13697"}</span>
              </div>
            </div>
            <div className="version-row">
              <span>v{overview?.version ?? "0.1.0"}</span>
              {hasToken && (
                <button
                  className="text-button"
                  onClick={() => {
                    clearToken();
                    window.location.reload();
                  }}
                >
                  退出
                </button>
              )}
            </div>
          </div>
        </aside>

        {mobileOpen && <div className="sidebar-scrim" onClick={() => setMobileOpen(false)} />}

        <main className="main-area">
          <header className="topbar">
            <div className="topbar-left">
              <button
                className="mobile-menu"
                aria-label="打开菜单"
                onClick={() => setMobileOpen(true)}
              >
                <Menu size={20} />
              </button>
              <div>
                <div className="topbar-eyebrow">{current?.eyebrow ?? "Control room"}</div>
                <h2>{current?.label ?? "Selena"}</h2>
              </div>
            </div>
            <div className="topbar-actions">
              <button
                className="endpoint-copy"
                onClick={async () => {
                  await copyText(endpoint);
                  setCopied(true);
                  window.setTimeout(() => setCopied(false), 1500);
                }}
                title="复制 Chat 入口"
              >
                <span className="endpoint-dot" />
                <span className="mono">{endpoint.replace(/^https?:\/\//, "")}</span>
                <Copy size={13} />
                {copied && <em>已复制</em>}
              </button>
              <button className="command-trigger" onClick={() => setPaletteOpen(true)}>
                <Command size={14} />
                <span>快速跳转</span>
                <kbd>⌘K</kbd>
              </button>
              <StatusPill tone={error ? "danger" : "ok"}>{error ? "offline" : "live"}</StatusPill>
            </div>
          </header>
          <div className="page-scroll">
            <Outlet />
          </div>
        </main>
      </div>

      {paletteOpen && (
        <div className="palette-backdrop" onMouseDown={() => setPaletteOpen(false)}>
          <div className="palette" onMouseDown={(event) => event.stopPropagation()}>
            <div className="palette-input-row">
              <Command size={16} />
              <input
                autoFocus
                placeholder="跳转到页面…"
                value={paletteQuery}
                onChange={(event) => setPaletteQuery(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") setPaletteOpen(false);
                  if (event.key === "Enter" && paletteItems[0]) {
                    navigate(paletteItems[0].path);
                    setPaletteOpen(false);
                  }
                }}
              />
              <kbd>esc</kbd>
            </div>
            <div className="palette-list">
              {paletteItems.map((item) => {
                const Icon = item.icon;
                return (
                  <button
                    key={item.path}
                    className="palette-item"
                    onClick={() => {
                      navigate(item.path);
                      setPaletteOpen(false);
                    }}
                  >
                    <Icon size={16} />
                    <span>{item.label}</span>
                    <em>{item.eyebrow}</em>
                  </button>
                );
              })}
              {paletteItems.length === 0 && <div className="palette-empty">没有匹配项</div>}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
