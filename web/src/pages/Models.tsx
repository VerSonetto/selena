import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Boxes, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../lib/api";
import type { ProviderModelGroup } from "../lib/types";
import { protocolLabel } from "../lib/types";
import {
  CopyButton,
  EmptyState,
  Input,
  Notice,
  ProtocolBadge,
  SectionHead,
  Select,
  Spinner,
} from "../components/ui";

export function Models() {
  const queryClient = useQueryClient();
  const pool = useQuery({ queryKey: ["model-pool"], queryFn: () => api.modelPool(false), refetchInterval: 60_000 });
  const refresh = useMutation({
    mutationFn: () => api.modelPool(true),
    onSuccess: (data) => queryClient.setQueryData(["model-pool"], data),
  });
  const [query, setQuery] = useState("");
  const [protocol, setProtocol] = useState("all");

  const groups = pool.data?.groups ?? [];
  const models = useMemo(() => {
    const search = query.trim().toLowerCase();
    return (pool.data?.models ?? []).filter((model) => {
      const matchesProtocol =
        protocol === "all" || model.providers.some((provider) => provider.protocol === protocol);
      const haystack = `${model.id} ${model.display_name ?? ""} ${model.providers
        .map((provider) => provider.name)
        .join(" ")}`.toLowerCase();
      return matchesProtocol && (!search || haystack.includes(search));
    });
  }, [pool.data, protocol, query]);

  const providerErrors = groups.filter((group) => group.error).length;
  const modelEntries = groups.reduce((sum, group) => sum + group.models.length, 0);

  if (pool.isLoading) return <Spinner label="正在从 Provider 获取模型" />;
  if (pool.error) {
    return (
      <div className="page">
        <EmptyState
          title="无法读取模型池"
          description={pool.error instanceof Error ? pool.error.message : "请检查本地服务和 Provider 配置。"}
        />
      </div>
    );
  }

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="MODEL POOL"
        title="模型池"
        description="从已接入的 Provider 动态读取可用模型。单个 Provider 获取失败不会影响其他 Provider。"
        actions={
          <button
            className="btn btn-secondary btn-md"
            onClick={() => refresh.mutate()}
            disabled={refresh.isPending}
          >
            <RefreshCw size={15} className={refresh.isPending ? "spin" : ""} />
            {refresh.isPending ? "获取中…" : "刷新模型"}
          </button>
        }
      />

      <div className="metric-grid model-metrics">
        <ModelMetric label="PROVIDERS" value={groups.length} note={`${providerErrors} fetch errors`} />
        <ModelMetric label="UNIQUE MODELS" value={pool.data?.models.length ?? 0} note="按模型 ID 去重" />
        <ModelMetric label="PROVIDER ENTRIES" value={modelEntries} note="同一模型可来自多个 Provider" />
        <ModelMetric
          label="LAST SYNC"
          value={pool.data ? new Date(pool.data.fetched_at * 1000).toLocaleTimeString() : "—"}
          note="缓存 120 秒"
        />
      </div>

      {refresh.error && <Notice tone="danger">{String(refresh.error)}</Notice>}
      {providerErrors > 0 && (
        <Notice tone="info">
          有 {providerErrors} 个 Provider 无法获取模型列表。请检查 Base URL、Key 或该服务是否实现了 `/models`。
        </Notice>
      )}

      <section className="panel model-pool-panel">
        <div className="panel-head">
          <div>
            <div className="eyebrow">AGGREGATED MODELS</div>
            <h2>全部模型</h2>
          </div>
          <div className="filter-bar model-filter-bar">
            <div className="filter-search">
              <Search size={15} />
              <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索模型或 Provider…" />
            </div>
            <Select value={protocol} onChange={setProtocol}>
              <option value="all">全部协议</option>
              <option value="chat_completions">Chat Completions</option>
              <option value="responses">Responses</option>
              <option value="anthropic_messages">Messages</option>
            </Select>
          </div>
        </div>

        {models.length === 0 ? (
          <EmptyState
            icon={<Boxes size={24} />}
            title={groups.length === 0 ? "尚未配置 Provider" : "没有可展示的模型"}
            description={
              groups.length === 0
                ? "添加 Provider 和 API Key 后，Selena 会自动获取模型列表。"
                : "Provider 未返回模型，或当前筛选条件没有匹配项。"
            }
          />
        ) : (
          <div className="data-table-wrap">
            <table className="data-table model-table">
              <thead>
                <tr>
                  <th>模型 ID</th>
                  <th>显示名称</th>
                  <th>Provider</th>
                  <th>协议</th>
                  <th>直接调用</th>
                </tr>
              </thead>
              <tbody>
                {models.map((model) => (
                  <tr key={model.id}>
                    <td><code className="model-id">{model.id}</code></td>
                    <td className="muted">{model.display_name ?? "—"}</td>
                    <td>
                      <div className="provider-badge-list">
                        {model.providers.map((provider) => (
                          <span className="provider-name-chip" key={provider.id}>{provider.name}</span>
                        ))}
                      </div>
                    </td>
                    <td>
                      <div className="provider-badge-list">
                        {[...new Set(model.providers.map((provider) => provider.protocol))].map((item) => (
                          <ProtocolBadge key={item} protocol={item} />
                        ))}
                      </div>
                    </td>
                    <td>
                      <CopyButton
                        value={`${model.providers[0]?.name ?? ""}/${model.id}`}
                        label="复制 model"
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="models-by-provider">
        <div className="panel-head">
          <div>
            <div className="eyebrow">BY PROVIDER</div>
            <h2>Provider 支持模型</h2>
          </div>
        </div>
        {groups.length === 0 ? (
          <div className="panel">
            <EmptyState title="尚无 Provider" description="在 Providers 页面上游 Provider。" />
          </div>
        ) : (
          <div className="model-group-grid">
            {groups.map((group) => <ProviderModelCard group={group} key={group.provider_id} />)}
          </div>
        )}
      </section>

      <p className="model-pool-note">
        模型来自各 Provider 的 <code>GET /models</code>。部分 OpenAI-compatible 服务不支持该端点时，请手动填写上游模型名。
      </p>
    </div>
  );
}

function ProviderModelCard({ group }: { group: ProviderModelGroup }) {
  return (
    <article className="panel model-group-card">
      <div className="model-group-head">
        <div>
          <h3>{group.provider_name}</h3>
          <span className="model-group-meta">
            {protocolLabel[group.protocol]} · {group.models.length} models
          </span>
        </div>
        <ProtocolBadge protocol={group.protocol} />
      </div>
      {group.error ? (
        <div className="model-fetch-error">{group.error}</div>
      ) : group.models.length === 0 ? (
        <p className="model-empty-text">该 Provider 未返回模型。</p>
      ) : (
        <div className="model-chip-list">
          {group.models.map((model) => (
            <span className="model-chip" key={model.id} title={model.display_name ?? model.id}>
              {model.id}
            </span>
          ))}
        </div>
      )}
      <div className="model-group-foot">
        <span>{group.fetched_at ? `更新于 ${new Date(group.fetched_at * 1000).toLocaleTimeString()}` : "尚未获取"}</span>
        <span>{group.enabled ? "enabled" : "disabled"}</span>
      </div>
    </article>
  );
}

function ModelMetric({ label, value, note }: { label: string; value: string | number; note: string }) {
  return (
    <article className="metric-card accent-teal">
      <div className="metric-top">
        <span className="metric-label">{label}</span>
      </div>
      <strong>{value}</strong>
      <span className="metric-note">{note}</span>
    </article>
  );
}
