import type {
  ClientKey,
  CreatedClientKey,
  ModelPool,
  Overview,
  Provider,
  ProviderEntry,
  ProviderKey,
  Route,
  RequestLog,
  Settings,
  UsagePoint,
} from "./types";

const TOKEN_KEY = "selena.admin_token";

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export function bootstrapToken(): string | null {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token") ?? url.searchParams.get("bootstrap");
  if (token) {
    localStorage.setItem(TOKEN_KEY, token);
    url.searchParams.delete("token");
    url.searchParams.delete("bootstrap");
    window.history.replaceState({}, "", `${url.pathname}${url.search}${url.hash}`);
    return token;
  }
  return localStorage.getItem(TOKEN_KEY);
}

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setToken(token: string) {
  localStorage.setItem(TOKEN_KEY, token.trim());
}

export function clearToken() {
  localStorage.removeItem(TOKEN_KEY);
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = getToken();
  const headers = new Headers(init.headers);
  if (token) headers.set("x-selena-admin", token);
  if (init.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  const response = await fetch(path, { ...init, headers });
  const text = await response.text();
  let payload: unknown = null;
  try {
    payload = text ? JSON.parse(text) : null;
  } catch {
    payload = text;
  }
  if (!response.ok) {
    const message =
      (payload as { error?: { message?: string } | string } | null)?.error &&
      typeof (payload as { error?: { message?: string } | string }).error === "object"
        ? ((payload as { error?: { message?: string } }).error?.message ?? response.statusText)
        : typeof (payload as { error?: unknown } | null)?.error === "string"
          ? ((payload as { error: string }).error)
          : response.statusText;
    throw new ApiError(response.status, message || `HTTP ${response.status}`);
  }
  return payload as T;
}

export const api = {
  session: () => request<{ ok: boolean; version: string }>("/api/auth/session"),
  login: (token: string) =>
    request<{ ok: boolean }>("/api/auth/login", {
      method: "POST",
      body: JSON.stringify({ token }),
    }),
  overview: () => request<Overview>("/api/overview"),
  providers: () => request<{ data: ProviderEntry[] }>("/api/providers"),
  modelPool: (refresh = false) =>
    request<ModelPool>(`/api/model-pool${refresh ? "?refresh=true" : ""}`),
  createProvider: (input: Record<string, unknown>) =>
    request<Provider>("/api/providers", { method: "POST", body: JSON.stringify(input) }),
  updateProvider: (id: string, input: Record<string, unknown>) =>
    request(`/api/providers/${id}`, { method: "PATCH", body: JSON.stringify(input) }),
  deleteProvider: (id: string) => request(`/api/providers/${id}`, { method: "DELETE" }),
  providerKeys: (id: string) => request<{ data: ProviderKey[] }>(`/api/providers/${id}/keys`),
  createProviderKey: (id: string, input: { label: string; secret: string }) =>
    request<ProviderKey>(`/api/providers/${id}/keys`, {
      method: "POST",
      body: JSON.stringify(input),
    }),
  deleteProviderKey: (providerId: string, keyId: string) =>
    request(`/api/providers/${providerId}/keys/${keyId}`, { method: "DELETE" }),
  setProviderKeyEnabled: (providerId: string, keyId: string, enabled: boolean) =>
    request(`/api/providers/${providerId}/keys/${keyId}/enabled`, {
      method: "PATCH",
      body: JSON.stringify({ enabled }),
    }),
  testProvider: (id: string) =>
    request<{ ok: boolean; message: string; key?: string }>(`/api/providers/${id}/test`, {
      method: "POST",
    }),
  routes: () => request<{ data: Route[] }>("/api/routes"),
  createRoute: (input: { model_alias: string; provider_id: string; upstream_model: string }) =>
    request("/api/routes", { method: "POST", body: JSON.stringify(input) }),
  deleteRoute: (id: string) => request(`/api/routes/${id}`, { method: "DELETE" }),
  clientKeys: () => request<{ data: ClientKey[] }>("/api/client-keys"),
  createClientKey: (input: { name: string; rate_limit_rpm?: number | null }) =>
    request<CreatedClientKey>("/api/client-keys", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  deleteClientKey: (id: string) => request(`/api/client-keys/${id}`, { method: "DELETE" }),
  setClientKeyEnabled: (id: string, enabled: boolean) =>
    request(`/api/client-keys/${id}/enabled`, {
      method: "PATCH",
      body: JSON.stringify({ enabled }),
    }),
  requests: (limit = 100) => request<{ data: RequestLog[] }>(`/api/requests?limit=${limit}`),
  usage: () => request<{ data: UsagePoint[] }>("/api/usage"),
  settings: () => request<Settings>("/api/settings"),
  saveSettings: (input: {
    default_provider?: string | null;
    theme?: string;
    port?: number;
  }) => request<Settings>("/api/settings", { method: "PUT", body: JSON.stringify(input) }),
  capabilities: () => request<Record<string, unknown>>("/api/capabilities"),
};

export function adminHeaders(): HeadersInit {
  const token = getToken();
  return token ? { "x-selena-admin": token } : {};
}

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

export function gatewayUrl(path: string): string {
  return `${window.location.origin}${path}`;
}
