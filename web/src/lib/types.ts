export type Protocol = "chat_completions" | "responses" | "anthropic_messages";

export interface Provider {
  id: string;
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_type: "bearer" | "x_api_key";
  extra_headers: string;
  enabled: boolean;
  timeout_ms: number;
  created_at: number;
  updated_at: number;
}

export interface ProviderEntry {
  provider: Provider;
  key_count: number;
  ready_keys: number;
}

export interface ProviderKey {
  id: string;
  provider_id: string;
  label: string;
  prefix: string;
  enabled: boolean;
  status: "ready" | "cooling" | "disabled" | string;
  cooldown_until: number | null;
  failure_count: number;
  last_used_at: number | null;
  created_at: number;
}

export interface ClientKey {
  id: string;
  name: string;
  key_prefix: string;
  enabled: boolean;
  rate_limit_rpm: number | null;
  created_at: number;
}

export interface CreatedClientKey extends ClientKey {
  secret: string;
}

export interface RequestLog {
  id: string;
  ts: number;
  ingress_protocol: string;
  egress_protocol: string;
  provider_id: string | null;
  provider_name: string | null;
  model_requested: string;
  model_upstream: string;
  client_key_id: string | null;
  status_code: number;
  error_code: string | null;
  stream: boolean;
  conversion_mode: string;
  latency_ms: number;
  ttft_ms: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  total_tokens: number | null;
  cached_tokens: number | null;
  cache_write_tokens: number | null;
  reasoning_tokens: number | null;
  conversion_warnings: string;
  upstream_request_id: string | null;
  error_message: string | null;
}

export interface RequestLogPage {
  data: RequestLog[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export interface RequestLogQuery {
  limit?: number;
  offset?: number;
  protocol?: string;
  status?: string;
  provider_id?: string;
  stream?: boolean;
  search?: string;
  days?: number;
}

export interface ProviderModel {
  id: string;
  display_name: string | null;
  created_at: number | null;
  owned_by: string | null;
}

export interface ProviderModelGroup {
  provider_id: string;
  provider_name: string;
  protocol: Protocol;
  enabled: boolean;
  models: ProviderModel[];
  error: string | null;
  fetched_at: number | null;
}

export interface ModelProviderRef {
  id: string;
  name: string;
  protocol: Protocol;
}

export interface ModelPoolEntry {
  id: string;
  display_name: string | null;
  providers: ModelProviderRef[];
}

export interface ModelPool {
  groups: ProviderModelGroup[];
  models: ModelPoolEntry[];
  fetched_at: number;
}

export interface Overview {
  version: string;
  host: string;
  port: number;
  providers: number;
  enabled_providers: number;
  client_keys: number;
  requests_today: number;
  errors_today: number;
  input_tokens_today: number;
  output_tokens_today: number;
  avg_latency_ms: number;
  recent_requests: RequestLog[];
}

export interface UsageBucket {
  key: string;
  label: string;
  requests: number;
  errors: number;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  cached_tokens: number;
  cache_write_tokens: number;
  reasoning_tokens: number;
  avg_latency_ms: number;
  avg_ttft_ms: number | null;
}

export interface UsageSummary {
  requests: number;
  errors: number;
  usage_requests: number;
  stream_requests: number;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  cached_tokens: number;
  cache_write_tokens: number;
  reasoning_tokens: number;
  avg_latency_ms: number;
  p50_latency_ms: number;
  p95_latency_ms: number;
  avg_ttft_ms: number | null;
}

export interface UsageReport {
  days: number;
  since: number;
  summary: UsageSummary;
  daily: UsageBucket[];
  by_provider: UsageBucket[];
  by_model: UsageBucket[];
  by_protocol: UsageBucket[];
}

export interface Settings {
  default_provider: string | null;
  theme: string;
  host: string;
  port: number;
  configured_port: number;
  restart_required: boolean;
  data_dir: string;
}

export const protocolLabel: Record<string, string> = {
  chat_completions: "Chat",
  responses: "Responses",
  anthropic_messages: "Messages",
};

export const protocolPath: Record<string, string> = {
  chat_completions: "/v1/chat/completions",
  responses: "/v1/responses",
  anthropic_messages: "/v1/messages",
};

export const protocolAccent: Record<string, string> = {
  chat_completions: "teal",
  responses: "coral",
  anthropic_messages: "gold",
};
