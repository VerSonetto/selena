import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRight, Plus, Route as RouteIcon, Trash2 } from "lucide-react";
import { useState } from "react";
import { api } from "../lib/api";
import {
  Button,
  EmptyState,
  Field,
  Input,
  Notice,
  ProtocolBadge,
  SectionHead,
  Select,
  Spinner,
} from "../components/ui";

export function RoutesPage() {
  const queryClient = useQueryClient();
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const routes = useQuery({ queryKey: ["routes"], queryFn: api.routes });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const [alias, setAlias] = useState("");
  const [providerId, setProviderId] = useState("");
  const [upstream, setUpstream] = useState("");
  const [defaultProvider, setDefaultProvider] = useState("");
  const [notice, setNotice] = useState("");

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["routes"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
  };

  const create = useMutation({
    mutationFn: () =>
      api.createRoute({ model_alias: alias.trim(), provider_id: providerId, upstream_model: upstream.trim() }),
    onSuccess: () => {
      setAlias("");
      setUpstream("");
      setNotice("路由已创建");
      refresh();
    },
    onError: (error) => setNotice(String(error)),
  });
  const remove = useMutation({
    mutationFn: (id: string) => api.deleteRoute(id),
    onSuccess: () => refresh(),
  });
  const saveDefault = useMutation({
    mutationFn: () => api.saveSettings({ default_provider: defaultProvider || null }),
    onSuccess: () => {
      setNotice("默认 Provider 已更新");
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
  });

  const entries = providers.data?.data ?? [];
  const providerMap = new Map(entries.map((entry) => [entry.provider.id, entry.provider]));
  const providerIdValue = providerId || entries[0]?.provider.id || "";
  const currentDefault = defaultProvider || settings.data?.default_provider || "";

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="ROUTING / 03"
        title="模型路由"
        description="将客户端模型名映射到 Provider 和上游模型；也可以使用 `provider/model` 直接指定。未命中时使用默认 Provider。"
      />

      {notice && <Notice tone="info">{notice}</Notice>}

      <div className="route-layout">
        <section className="panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">ALIASES</div>
              <h2>模型别名</h2>
            </div>
            <span className="count-badge">{routes.data?.data.length ?? 0}</span>
          </div>
          {routes.isLoading ? (
            <Spinner label="读取路由" />
          ) : routes.data?.data.length === 0 ? (
            <EmptyState icon={<RouteIcon size={20} />} title="尚未配置模型别名" description="在右侧创建第一条模型路由。" />
          ) : (
            <div className="route-list">
              {routes.data?.data.map((route) => {
                const provider = providerMap.get(route.provider_id);
                return (
                  <div className="route-row" key={route.id}>
                    <div className="route-alias">
                      <span className="mono">{route.model_alias}</span>
                      <span className="route-note">客户端模型</span>
                    </div>
                    <ArrowRight size={15} className="route-arrow" />
                    <div className="route-target">
                      <strong>{provider?.name ?? route.provider_id}</strong>
                      <span className="mono">{route.upstream_model}</span>
                    </div>
                    {provider && <ProtocolBadge protocol={provider.protocol} />}
                    <button className="icon-btn danger-hover" onClick={() => remove.mutate(route.id)} aria-label="删除路由">
                      <Trash2 size={15} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </section>

        <aside className="stack">
          <section className="panel">
            <div className="panel-head">
              <div>
                <div className="eyebrow">NEW ALIAS</div>
                <h2>添加路由</h2>
              </div>
            </div>
            <form
              className="stack-form"
              onSubmit={(event) => {
                event.preventDefault();
                create.mutate();
              }}
            >
              <Field label="客户端模型名">
                <Input value={alias} onChange={(event) => setAlias(event.target.value)} placeholder="gpt-4o / claude-sonnet" required />
              </Field>
              <Field label="Provider">
                <Select value={providerIdValue} onChange={setProviderId}>
                  {entries.map((entry) => (
                    <option key={entry.provider.id} value={entry.provider.id}>
                      {entry.provider.name} · {entry.provider.protocol}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="上游模型名">
                <Input value={upstream} onChange={(event) => setUpstream(event.target.value)} placeholder="gpt-4o-mini" required />
              </Field>
              <Button type="submit" disabled={!entries.length || !alias.trim() || !upstream.trim() || create.isPending}>
                <Plus size={15} /> 创建路由
              </Button>
            </form>
            {!entries.length && <p className="modal-note">请先在 Providers 页面添加上游。</p>}
          </section>

          <section className="panel">
            <div className="panel-head">
              <div>
                <div className="eyebrow">FALLBACK</div>
                <h2>默认 Provider</h2>
              </div>
            </div>
            <p className="panel-copy">未命中别名和 `provider/model` 前缀时使用。</p>
            <Select value={currentDefault} onChange={setDefaultProvider}>
              <option value="">— 未设置 —</option>
              {entries.map((entry) => (
                <option key={entry.provider.id} value={entry.provider.id}>{entry.provider.name}</option>
              ))}
            </Select>
            <Button variant="secondary" onClick={() => saveDefault.mutate()} disabled={saveDefault.isPending}>
              保存默认 Provider
            </Button>
          </section>

          <section className="panel route-syntax">
            <div className="eyebrow">DIRECT OVERRIDE</div>
            <h2>`provider/model`</h2>
            <p>直接用模型名前缀指定 Provider，无需先建别名。</p>
            <code>my-provider/gpt-4.1</code>
            <p className="muted">同协议按原始 SSE 透传；跨协议执行 IR 转换和能力检查。</p>
          </section>
        </aside>
      </div>
    </div>
  );
}
