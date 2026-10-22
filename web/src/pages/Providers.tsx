import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Boxes,
  KeyRound,
  MoreHorizontal,
  Plus,
  RefreshCw,
  ShieldCheck,
  Trash2,
  Waypoints,
} from "lucide-react";
import { useState } from "react";
import { api } from "../lib/api";
import type { ProviderEntry, ProviderKey, ProviderModelGroup } from "../lib/types";
import {
  Button,
  EmptyState,
  Field,
  IconButton,
  Input,
  Modal,
  Notice,
  ProtocolBadge,
  SectionHead,
  Select,
  Spinner,
  Switch,
  formatNumber,
  timeAgo,
} from "../components/ui";

const protocolOptions = [
  { value: "chat_completions", label: "Chat Completions" },
  { value: "responses", label: "Responses" },
  { value: "anthropic_messages", label: "Anthropic Messages" },
];

export function Providers() {
  const queryClient = useQueryClient();
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const modelPool = useQuery({ queryKey: ["model-pool"], queryFn: () => api.modelPool(false) });
  const [createOpen, setCreateOpen] = useState(false);
  const [keyTarget, setKeyTarget] = useState<ProviderEntry | null>(null);
  const [modelTarget, setModelTarget] = useState<ProviderEntry | null>(null);
  const [notice, setNotice] = useState<{ tone: "success" | "danger" | "info"; message: string } | null>(null);

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
    void queryClient.invalidateQueries({ queryKey: ["overview"] });
    void queryClient.invalidateQueries({ queryKey: ["model-pool"] });
  };

  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.updateProvider(id, { enabled }),
    onSuccess: () => refresh(),
    onError: (error) => setNotice({ tone: "danger", message: String(error) }),
  });

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteProvider(id),
    onSuccess: () => refresh(),
    onError: (error) => setNotice({ tone: "danger", message: String(error) }),
  });

  const test = useMutation({
    mutationFn: (id: string) => api.testProvider(id),
    onSuccess: (result) =>
      setNotice({ tone: result.ok ? "success" : "info", message: result.message }),
    onError: (error) => setNotice({ tone: "danger", message: String(error) }),
  });

  if (providers.isLoading) return <Spinner label="读取 Provider" />;
  const entries = providers.data?.data ?? [];
  const modelGroups = new Map((modelPool.data?.groups ?? []).map((group) => [group.provider_id, group]));

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="PROVIDERS / 02"
        title="上游 Provider"
        description="配置上游协议、Base URL、API Key 与请求超时。入口协议和上游协议可以相同，也可以不同。"
        actions={
          <Button onClick={() => setCreateOpen(true)}>
            <Plus size={15} /> 添加 Provider
          </Button>
        }
      />

      <div className="protocol-ribbon">
        <div>
          <span className="eyebrow">PROTOCOL MATRIX</span>
          <strong>入口协议与上游协议相互独立</strong>
        </div>
        <div className="ribbon-flow">
          <ProtocolBadge protocol="chat_completions" />
          <span>↔</span>
          <ProtocolBadge protocol="responses" />
          <span>↔</span>
          <ProtocolBadge protocol="anthropic_messages" />
          <span className="ribbon-note">3 × 3 组合</span>
        </div>
      </div>

      {notice && (
        <Notice tone={notice.tone === "success" ? "success" : notice.tone === "danger" ? "danger" : "info"}>
          {notice.message}
        </Notice>
      )}

      {entries.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<Waypoints size={24} />}
            title="尚未配置 Provider"
            description="Provider 可以是 OpenAI Chat、Responses，或 Anthropic Messages。需要 Base URL 和至少一个 API Key。"
            action={<Button onClick={() => setCreateOpen(true)}><Plus size={15} /> 添加第一个 Provider</Button>}
          />
        </div>
      ) : (
        <div className="provider-grid">
          {entries.map((entry) => (
            <ProviderCard
              key={entry.provider.id}
              entry={entry}
              onKeys={() => setKeyTarget(entry)}
              modelGroup={modelGroups.get(entry.provider.id)}
              onModels={() => setModelTarget(entry)}
              onToggle={(enabled) => toggle.mutate({ id: entry.provider.id, enabled })}
              onTest={() => test.mutate(entry.provider.id)}
              onDelete={() => {
                if (window.confirm(`删除 Provider「${entry.provider.name}」及其 Key？`)) {
                  remove.mutate(entry.provider.id);
                }
              }}
            />
          ))}
        </div>
      )}

      <CreateProviderModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={() => {
          setCreateOpen(false);
          refresh();
          setNotice({ tone: "success", message: "Provider 已添加。下一步：添加至少一个 Key 即可调用。" });
        }}
      />
      {keyTarget && (
        <KeysModal
          entry={keyTarget}
          onClose={() => {
            setKeyTarget(null);
            refresh();
          }}
        />
      )}
      {modelTarget && (
        <ProviderModelsModal
          entry={modelTarget}
          group={modelGroups.get(modelTarget.provider.id)}
          onClose={() => setModelTarget(null)}
        />
      )}
    </div>
  );
}

function ProviderCard({
  entry,
  modelGroup,
  onKeys,
  onModels,
  onToggle,
  onTest,
  onDelete,
}: {
  entry: ProviderEntry;
  modelGroup?: ProviderModelGroup;
  onKeys: () => void;
  onModels: () => void;
  onToggle: (enabled: boolean) => void;
  onTest: () => void;
  onDelete: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const provider = entry.provider;
  const healthy = provider.enabled && entry.ready_keys > 0;

  return (
    <article className={`provider-card ${healthy ? "provider-healthy" : "provider-muted"}`}>
      <div className="provider-head">
        <div className={`protocol-orb orb-${provider.protocol}`}>
          <span>{provider.protocol === "responses" ? "R" : provider.protocol === "anthropic_messages" ? "M" : "C"}</span>
        </div>
        <div className="provider-title">
          <h3>{provider.name}</h3>
          <p>{provider.base_url.replace(/^https?:\/\//, "")}</p>
        </div>
        <div className="provider-menu">
          <IconButton label="更多" onClick={() => setMenu((open) => !open)}>
            <MoreHorizontal size={17} />
          </IconButton>
          {menu && (
            <div className="menu-popover">
              <button onClick={onTest}><RefreshCw size={14} /> 测试可用性</button>
              <button onClick={onKeys}><KeyRound size={14} /> 管理 Key</button>
              <button className="menu-danger" onClick={onDelete}><Trash2 size={14} /> 删除</button>
            </div>
          )}
        </div>
      </div>

      <div className="provider-meta">
        <ProtocolBadge protocol={provider.protocol} />
        <span className={`status-pill ${healthy ? "status-ok" : "status-warn"}`}>
          {healthy ? "ready" : provider.enabled ? "no key" : "disabled"}
        </span>
        <span className="provider-timeout">{provider.timeout_ms / 1000}s timeout</span>
      </div>

      <div className="provider-stats">
        <div>
          <span>KEYS</span>
          <strong>{entry.ready_keys}<em>/{entry.key_count}</em></strong>
        </div>
        <div>
          <span>AUTH</span>
          <strong className="small-strong">{provider.auth_type === "x_api_key" ? "x-api-key" : "Bearer"}</strong>
        </div>
        <div>
          <span>ADDED</span>
          <strong className="small-strong">{timeAgo(provider.created_at)}</strong>
        </div>
      </div>

      <div className="provider-models">
        <div className="provider-models-head">
          <span><Boxes size={13} /> 支持模型</span>
          <strong>{modelGroup ? modelGroup.models.length : "—"}</strong>
        </div>
        {modelGroup?.error ? (
          <p className="provider-models-error">{modelGroup.error}</p>
        ) : modelGroup && modelGroup.models.length > 0 ? (
          <div className="provider-model-chips">
            {modelGroup.models.slice(0, 3).map((model) => (
              <span className="model-chip" key={model.id}>{model.id}</span>
            ))}
            {modelGroup.models.length > 3 && <span className="model-chip more">+{modelGroup.models.length - 3}</span>}
          </div>
        ) : (
          <p className="provider-models-empty">{modelGroup ? "未返回模型" : "读取中…"}</p>
        )}
        <button className="text-button provider-models-link" onClick={onModels}>查看全部模型</button>
      </div>

      <div className="provider-actions">
        <button className="text-button" onClick={onKeys}>
          <KeyRound size={14} /> Key 池
        </button>
        <button className="text-button" onClick={onTest}>
          <ShieldCheck size={14} /> 检查
        </button>
        <div className="provider-switch">
          <span>{provider.enabled ? "启用" : "停用"}</span>
          <Switch checked={provider.enabled} onChange={onToggle} label="启用 Provider" />
        </div>
      </div>
    </article>
  );
}

function CreateProviderModal({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState("chat_completions");
  const [baseUrl, setBaseUrl] = useState("");
  const [authType, setAuthType] = useState("bearer");
  const [timeout, setTimeoutValue] = useState("300");
  const [label, setLabel] = useState("primary");
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const provider = await api.createProvider({
        name: name.trim(),
        protocol,
        base_url: baseUrl.trim(),
        auth_type: protocol === "anthropic_messages" ? "x_api_key" : authType,
        timeout_ms: Number(timeout) * 1000,
      });
      if (secret.trim()) {
        await api.createProviderKey(provider.id, { label: label.trim() || "primary", secret: secret.trim() });
      }
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="添加 Provider" eyebrow="NEW UPSTREAM" wide>
      <form className="form-grid" onSubmit={submit}>
        <Field label="名称">
          <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="OpenAI · Production" required />
        </Field>
        <Field label="上游协议">
          <Select
            value={protocol}
            onChange={(value) => {
              setProtocol(value);
              if (value === "anthropic_messages") setAuthType("x_api_key");
            }}
          >
            {protocolOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
          </Select>
        </Field>
        <Field label="Base URL" className="span-2" hint="Selena 会在此地址后拼接 /chat/completions、/responses 或 /messages。">
          <Input value={baseUrl} onChange={(event) => setBaseUrl(event.target.value)} placeholder="https://api.openai.com/v1" required />
        </Field>
        <Field label="认证方式">
          <Select value={authType} onChange={setAuthType}>
            <option value="bearer">Bearer</option>
            <option value="x_api_key">x-api-key</option>
          </Select>
        </Field>
        <Field label="超时（秒）">
          <Input type="number" value={timeout} onChange={(event) => setTimeoutValue(event.target.value)} min={1} />
        </Field>
        <div className="form-divider span-2"><span>可选的第一个 Key</span></div>
        <Field label="Key 标签">
          <Input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="primary" />
        </Field>
        <Field label="API Key" hint="加密后存入本地 SQLite，只在创建时接收明文。">
          <Input type="password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder="sk-…" />
        </Field>
        {error && <div className="form-error span-2">{error}</div>}
        <div className="form-actions span-2">
          <Button variant="ghost" type="button" onClick={onClose}>取消</Button>
          <Button type="submit" disabled={busy || !name.trim() || !baseUrl.trim()}>
            {busy ? "创建中…" : "创建 Provider"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function ProviderModelsModal({
  entry,
  group,
  onClose,
}: {
  entry: ProviderEntry;
  group?: ProviderModelGroup;
  onClose: () => void;
}) {
  return (
    <Modal
      open
      onClose={onClose}
      title={`${entry.provider.name} · 模型列表`}
      eyebrow="PROVIDER MODELS"
      wide
    >
      {!group ? (
        <Spinner label="正在读取模型" />
      ) : group.error ? (
        <Notice tone="danger">{group.error}</Notice>
      ) : group.models.length === 0 ? (
        <EmptyState title="未返回模型" description="该 Provider 的 /models 端点没有返回模型。" />
      ) : (
        <div className="provider-model-modal-list">
          {group.models.map((model) => (
            <div className="provider-model-row" key={model.id}>
              <div>
                <code>{model.id}</code>
                <span>{model.display_name ?? "—"}</span>
              </div>
              <span className="muted">{model.owned_by ?? ""}</span>
            </div>
          ))}
        </div>
      )}
      <p className="modal-note">
        来源：{entry.provider.base_url.replace(/^https?:\/\//, "")}/models。模型列表由 Provider 动态返回，可能受账号权限影响。
      </p>
    </Modal>
  );
}

function KeysModal({ entry, onClose }: { entry: ProviderEntry; onClose: () => void }) {
  const queryClient = useQueryClient();
  const keys = useQuery({
    queryKey: ["provider-keys", entry.provider.id],
    queryFn: () => api.providerKeys(entry.provider.id),
  });
  const [label, setLabel] = useState("primary");
  const [secret, setSecret] = useState("");
  const [error, setError] = useState("");

  const create = useMutation({
    mutationFn: () => api.createProviderKey(entry.provider.id, { label, secret }),
    onSuccess: () => {
      setSecret("");
      setError("");
      void queryClient.invalidateQueries({ queryKey: ["provider-keys", entry.provider.id] });
      void queryClient.invalidateQueries({ queryKey: ["providers"] });
    },
    onError: (err) => setError(err instanceof Error ? err.message : "添加 Key 失败"),
  });
  const remove = useMutation({
    mutationFn: (key: ProviderKey) => api.deleteProviderKey(entry.provider.id, key.id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["provider-keys", entry.provider.id] });
      void queryClient.invalidateQueries({ queryKey: ["providers"] });
    },
  });
  const toggle = useMutation({
    mutationFn: ({ key, enabled }: { key: ProviderKey; enabled: boolean }) =>
      api.setProviderKeyEnabled(entry.provider.id, key.id, enabled),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["provider-keys", entry.provider.id] }),
  });

  return (
    <Modal open title={`${entry.provider.name} · Key 池`} eyebrow="CREDENTIALS" onClose={onClose} wide>
      <div className="key-list">
        {keys.isLoading && <Spinner label="读取 Key" />}
        {keys.data?.data.length === 0 && <p className="muted-line">还没有 Key。添加至少一个才能路由请求。</p>}
        {keys.data?.data.map((key) => (
          <div className="key-row" key={key.id}>
            <div>
              <strong>{key.label}</strong>
              <span className="mono">{key.prefix}</span>
            </div>
            <span className={`status-pill ${key.status === "ready" ? "status-ok" : key.status === "cooling" ? "status-warn" : "status-danger"}`}>
              {key.status}
            </span>
            <span className="muted">{key.failure_count} failures</span>
            <Switch checked={key.enabled} onChange={(enabled) => toggle.mutate({ key, enabled })} />
            <IconButton label="删除 Key" onClick={() => remove.mutate(key)}><Trash2 size={15} /></IconButton>
          </div>
        ))}
      </div>
      <div className="form-divider"><span>添加 Key</span></div>
      <form
        className="inline-form"
        onSubmit={(event) => {
          event.preventDefault();
          create.mutate();
        }}
      >
        <Input value={label} onChange={(event) => setLabel(event.target.value)} placeholder="标签" />
        <Input type="password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder="API Key" />
        <Button type="submit" disabled={!secret.trim() || create.isPending}>添加</Button>
      </form>
      {error && <div className="form-error">{error}</div>}
      <p className="modal-note">Key 以加密形式保存在本地数据库，界面和日志不会再次显示明文。</p>
    </Modal>
  );
}
