import { useEffect, useMemo, useState } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { api, bootstrapToken, clearToken, getToken, setToken } from "./lib/api";
import { Shell } from "./components/Shell";
import { Login } from "./pages/Login";
import { Overview } from "./pages/Overview";
import { Providers } from "./pages/Providers";
import { Models } from "./pages/Models";
import { RoutesPage } from "./pages/Routes";
import { Keys } from "./pages/Keys";
import { Playground } from "./pages/Playground";
import { Logs } from "./pages/Logs";
import { Analytics } from "./pages/Analytics";
import { SettingsPage } from "./pages/Settings";
import { NotFound } from "./pages/NotFound";

export type ThemeMode = "system" | "ivory" | "dusk";
type ResolvedTheme = "ivory" | "dusk";

function storedThemeMode(): ThemeMode {
  const stored = localStorage.getItem("selena.theme");
  return stored === "ivory" || stored === "dusk" || stored === "system" ? stored : "system";
}

export function App() {
  const [token, setTokenState] = useState<string | null>(() => bootstrapToken() ?? getToken());
  const [themeMode, setThemeMode] = useState<ThemeMode>(storedThemeMode);
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() =>
    window.matchMedia("(prefers-color-scheme: dark)").matches ? "dusk" : "ivory",
  );

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => setSystemTheme(event.matches ? "dusk" : "ivory");
    setSystemTheme(media.matches ? "dusk" : "ivory");
    media.addEventListener?.("change", onChange);
    return () => media.removeEventListener?.("change", onChange);
  }, []);

  const resolvedTheme = useMemo<ResolvedTheme>(
    () => (themeMode === "system" ? systemTheme : themeMode),
    [systemTheme, themeMode],
  );

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.dataset.themeMode = themeMode;
    localStorage.setItem("selena.theme", themeMode);
  }, [resolvedTheme, themeMode]);

  useEffect(() => {
    const onStorage = () => setTokenState(getToken());
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  useEffect(() => {
    if (!token) return;
    let cancelled = false;
    api
      .session()
      .catch(() => {
        if (cancelled) return;
        clearToken();
        setTokenState(null);
      });
    return () => {
      cancelled = true;
    };
  }, [token]);

  if (!token) {
    return (
      <Login
        onLogin={(value) => {
          setToken(value);
          setTokenState(value);
        }}
      />
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route index element={<Overview />} />
          <Route path="providers" element={<Providers />} />
          <Route path="models" element={<Models />} />
          <Route path="routes" element={<RoutesPage />} />
          <Route path="keys" element={<Keys />} />
          <Route path="playground" element={<Playground />} />
          <Route path="logs" element={<Logs />} />
          <Route path="analytics" element={<Analytics />} />
          <Route
            path="settings"
            element={
              <SettingsPage
                themeMode={themeMode}
                systemTheme={systemTheme}
                onThemeMode={setThemeMode}
              />
            }
          />
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export function logout() {
  clearToken();
  window.location.reload();
}

export function navigateHome() {
  window.location.href = "/";
}
