import { useEffect, useState } from "react";
import { Minus, Square, X } from "lucide-react";

type AppWindow = {
  startDragging: () => Promise<void>;
  minimize: () => Promise<void>;
  toggleMaximize: () => Promise<void>;
  close: () => Promise<void>;
};

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function TitleBar() {
  const [appWindow, setAppWindow] = useState<AppWindow | null>(null);
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    setIsMac(/Mac|iPhone|iPad/.test(navigator.userAgent));
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) => setAppWindow(getCurrentWindow() as unknown as AppWindow))
      .catch(() => setAppWindow(null));
  }, []);

  if (!isTauri()) return null;

  return (
    <div
      className={`titlebar ${isMac ? "titlebar-mac" : ""}`}
      data-tauri-drag-region="deep"
      onMouseDown={(event) => {
        if (event.buttons === 1 && event.target === event.currentTarget) {
          void appWindow?.startDragging();
        }
      }}
      onDoubleClick={() => void appWindow?.toggleMaximize()}
    >
      <div className="titlebar-drag" data-tauri-drag-region="deep">
        <span className="titlebar-caption">
          <i />
          CONTROL ROOM
        </span>
      </div>
      <div className="titlebar-controls">
        <button aria-label="最小化" onClick={() => void appWindow?.minimize()}>
          <Minus size={14} />
        </button>
        <button aria-label="最大化" onClick={() => void appWindow?.toggleMaximize()}>
          <Square size={12} />
        </button>
        <button className="titlebar-close" aria-label="关闭" onClick={() => void appWindow?.close()}>
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
