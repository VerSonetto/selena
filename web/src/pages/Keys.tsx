import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { KeyRound, Plus, ShieldAlert, Trash2 } from "lucide-react";
import { useState } from "react";
import { api, copyText } from "../lib/api";
import type { CreatedClientKey } from "../lib/types";
import {
  Button,
  CopyButton,
  EmptyState,
  Field,
  IconButton,
  Input,
  Modal,
  Notice,
  SectionHead,
  Spinner,
  Switch,
  timeAgo,
} from "../components/ui";

export function Keys() {
  const queryClient = useQueryClient();
  const keys = useQuery({ queryKey: ["client-keys"], queryFn: api.clientKeys });
  const [createOpen, setCreateOpen] = useState(false);
  const [created, setCreated] = useState<CreatedClientKey | null>(null);
  const [notice, setNotice] = useState("");

  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["client-keys"] });
    void queryClient.invalidateQueries({ queryKey: ["overview"] });
  };

  const remove = useMutation({
    mutationFn: (id: string) => api.deleteClientKey(id),
    onSuccess: () => {
      setNotice("客户端 Key 已删除");
      refresh();
    },
  });
  const toggle = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.setClientKeyEnabled(id, enabled),
    onSuccess: () => refresh(),
  });

  const list = keys.data?.data ?? [];

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="CLIENT KEYS / 04"
        title="客户端密钥"
        description="管理调用网关的客户端 API Key、RPM 限制和启用状态。支持 Bearer 与 x-api-key。"
        actions={<Button onClick={() => setCreateOpen(true)}><Plus size={15} /> 创建客户端 Key</Button>}
      />

      {notice && <Notice tone="success">{notice}</Notice>}

      <div className="access-note">
        <ShieldAlert size={18} />
        <div>
          <strong>未创建任何客户端 Key 时，网关默认放行本机请求。</strong>
          <p>一旦创建至少一个 Key，所有入口都需要携带有效 Key。适合本地首次体验，不建议长期保持开放。</p>
        </div>
      </div>

      {keys.isLoading ? (
        <Spinner label="读取客户端 Key" />
      ) : list.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<KeyRound size={24} />}
            title="尚未创建客户端 Key"
            description="创建后，在 Authorization: Bearer 或 x-api-key 请求头中携带。"
            action={<Button onClick={() => setCreateOpen(true)}>创建第一个 Key</Button>}
          />
        </div>
      ) : (
        <div className="panel">
          <div className="data-table-wrap">
            <table className="data-table key-table">
              <thead>
                <tr>
                  <th>名称</th>
                  <th>前缀</th>
                  <th>速率</th>
                  <th>创建</th>
                  <th>状态</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {list.map((key) => (
                  <tr key={key.id}>
                    <td><strong>{key.name}</strong></td>
                    <td><code className="key-prefix">{key.key_prefix}</code></td>
                    <td className="muted">{key.rate_limit_rpm ? `${key.rate_limit_rpm} rpm` : "unlimited"}</td>
                    <td className="muted">{timeAgo(key.created_at)}</td>
                    <td>
                      <div className="switch-inline">
                        <Switch checked={key.enabled} onChange={(enabled) => toggle.mutate({ id: key.id, enabled })} />
                        <span className="muted">{key.enabled ? "enabled" : "disabled"}</span>
                      </div>
                    </td>
                    <td>
                      <IconButton
                        label="删除 Key"
                        className="danger-hover"
                        onClick={() => {
                          if (window.confirm(`删除客户端 Key「${key.name}」？`)) remove.mutate(key.id);
                        }}
                      >
                        <Trash2 size={15} />
                      </IconButton>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      <CreateKeyModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={(value) => {
          setCreateOpen(false);
          setCreated(value);
          refresh();
        }}
      />

      <Modal open={Boolean(created)} onClose={() => setCreated(null)} title="Key 已创建" eyebrow="COPY ONCE">
        {created && (
          <div className="secret-reveal">
            <p>完整 Key 只显示一次。关闭后无法再次查看，只能重新创建。</p>
            <div className="secret-box">
              <code>{created.secret}</code>
              <CopyButton value={created.secret} label="复制 Key" />
            </div>
            <div className="secret-meta">
              <span>名称：{created.name}</span>
              <span>前缀：{created.key_prefix}</span>
            </div>
            <Button onClick={() => setCreated(null)}>我已保存</Button>
          </div>
        )}
      </Modal>
    </div>
  );
}

function CreateKeyModal({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (key: CreatedClientKey) => void;
}) {
  const [name, setName] = useState("");
  const [rate, setRate] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const value = await api.createClientKey({
        name: name.trim(),
        rate_limit_rpm: rate ? Number(rate) : null,
      });
      setName("");
      setRate("");
      onCreated(value);
    } catch (err) {
      setError(err instanceof Error ? err.message : "创建失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={onClose} title="创建客户端 Key" eyebrow="NEW ACCESS KEY">
      <form className="stack-form" onSubmit={submit}>
        <Field label="名称">
          <Input value={name} onChange={(event) => setName(event.target.value)} placeholder="my-laptop / codex-cli" required />
        </Field>
        <Field label="RPM 限制" hint="可选。留空表示不限制。">
          <Input type="number" min={1} value={rate} onChange={(event) => setRate(event.target.value)} placeholder="120" />
        </Field>
        {error && <div className="form-error">{error}</div>}
        <div className="form-actions">
          <Button variant="ghost" type="button" onClick={onClose}>取消</Button>
          <Button type="submit" disabled={busy || !name.trim()}>{busy ? "创建中…" : "创建 Key"}</Button>
        </div>
      </form>
    </Modal>
  );
}
