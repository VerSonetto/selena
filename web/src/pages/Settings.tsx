import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Database, Globe2, KeyRound, Laptop, MonitorCog, Palette, Save, ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";
import type { ThemeMode } from "../App";
import { api, copyText, gatewayUrl } from "../lib/api";
import { protocolPath } from "../lib/types";
import {
  Button,
  CopyButton,
  Field,
  Input,
  Notice,
  SectionHead,
  Select,
  Spinner,
} from "../components/ui";

export function SettingsPage({
  themeMode,
  systemTheme,
  onThemeMode,
}: {
  themeMode: ThemeMode;
  systemTheme: "ivory" | "dusk";
  onThemeMode: (theme: ThemeMode) => void;
}) {
  const queryClient = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const [defaultProvider, setDefaultProvider] = useState("");
  const [port, setPort] = useState("");
  const [notice, setNotice] = useState("");
  const [noticeTone, setNoticeTone] = useState<"success" | "info" | "danger">("success");
  const [selectedTheme, setSelectedTheme] = useState<ThemeMode>(themeMode);

  useEffect(() => {
    if (!settings.data) return;
    setPort(String(settings.data.configured_port ?? settings.data.port));
  }, [settings.data]);

  const save = useMutation({
    mutationFn: () => {
      const portNumber = Number(port);
      if (!Number.isInteger(portNumber) || portNumber < 1 || portNumber > 65535) {
        throw new Error("端口必须是 1–65535 之间的整数");
      }
      return api.saveSettings({
        default_provider: defaultProvider || settings.data?.default_provider || null,
        theme: selectedTheme,
        port: portNumber,
      });
    },
    onSuccess: (data) => {
      onThemeMode(selectedTheme);
      setNoticeTone(data.restart_required ? "info" : "success");
      setNotice(
        data.restart_required
          ? "端口已保存；重启 Selena 后生效。"
          : "设置已保存",
      );
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => {
      setNoticeTone("danger");
      setNotice(String(error));
    },
  });

  if (settings.isLoading) return <Spinner label="读取设置" />;
  const data = settings.data;
  const entries = providers.data?.data ?? [];
  const currentDefault = defaultProvider || data?.default_provider || "";

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="SETTINGS / 08"
        title="设置"
        description="修改默认 Provider、服务端口、外观和本地数据设置。"
        actions={<Button onClick={() => save.mutate()} disabled={save.isPending}><Save size={15} /> 保存设置</Button>}
      />
      {notice && <Notice tone={noticeTone}>{notice}</Notice>}
      {settings.data?.restart_required && (
        <Notice tone="info">
          配置端口为 {settings.data.configured_port}，当前仍运行在 {settings.data.port}。重启 Selena 后切换。
        </Notice>
      )}

      <div className="settings-grid">
        <section className="panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">GATEWAY</div>
              <h2>服务与路由</h2>
            </div>
            <Globe2 size={18} className="panel-icon" />
          </div>
          <div className="settings-fields">
            <Field label="默认 Provider" hint="未命中别名或 provider/model 前缀时使用。">
              <Select value={currentDefault} onChange={setDefaultProvider}>
                <option value="">— 未设置 —</option>
                {entries.map((entry) => (
                  <option key={entry.provider.id} value={entry.provider.id}>{entry.provider.name}</option>
                ))}
              </Select>
            </Field>
            <Field
              label="自定义端口"
              hint="1–65535。保存后写入 config.toml，重启 Selena 后生效；当前连接不会中断。"
            >
              <div className="port-field">
                <Input
                  type="number"
                  min={1}
                  max={65535}
                  value={port}
                  onChange={(event) => setPort(event.target.value)}
                  placeholder="13697"
                />
                <span className="port-suffix">TCP</span>
              </div>
            </Field>
            <div className="readout-grid">
              <Readout label="当前运行端口" value={String(data?.port ?? "—")} />
              <Readout label="下次启动端口" value={String(data?.configured_port ?? data?.port ?? "—")} />
              <Readout label="监听地址" value={`${data?.host}:${data?.port}`} />
              <Readout label="数据目录" value={data?.data_dir ?? "—"} />
              <Readout label="版本" value={data ? "0.1.0" : "—"} />
              <Readout label="运行模式" value="local-first" />
            </div>
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">APPEARANCE</div>
              <h2>外观</h2>
            </div>
            <Palette size={18} className="panel-icon" />
          </div>
          <p className="panel-copy">主题支持跟随系统、Ivory 和 Dusk。跟随系统会实时响应操作系统的深浅色设置。</p>
          <div className="theme-options">
            <button className={`theme-swatch theme-system ${selectedTheme === "system" ? "swatch-active" : ""}`} onClick={() => setSelectedTheme("system")}>
              <span className="swatch-preview"><i /><i /><i /></span>
              <span>
                <strong>跟随系统 <Laptop size={12} /></strong>
                <em>current · {systemTheme === "dusk" ? "Dusk" : "Ivory"}</em>
              </span>
              {selectedTheme === "system" && <Check size={15} />}
            </button>
            <button className={`theme-swatch theme-ivory ${selectedTheme === "ivory" ? "swatch-active" : ""}`} onClick={() => setSelectedTheme("ivory")}>
              <span className="swatch-preview"><i /><i /><i /></span>
              <span><strong>Ivory</strong><em>warm paper</em></span>
              {selectedTheme === "ivory" && <Check size={15} />}
            </button>
            <button className={`theme-swatch theme-dusk ${selectedTheme === "dusk" ? "swatch-active" : ""}`} onClick={() => setSelectedTheme("dusk")}>
              <span className="swatch-preview"><i /><i /><i /></span>
              <span><strong>Dusk</strong><em>low light</em></span>
              {selectedTheme === "dusk" && <Check size={15} />}
            </button>
          </div>
          <Button variant="secondary" onClick={() => onThemeMode(selectedTheme)}>应用主题</Button>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">ENDPOINTS</div>
              <h2>三个入口</h2>
            </div>
            <KeyRound size={18} className="panel-icon" />
          </div>
          <div className="endpoint-list">
            {Object.entries(protocolPath).map(([protocol, path]) => {
              const url = gatewayUrl(path);
              return (
                <div className="endpoint-list-row" key={protocol}>
                  <div>
                    <span>{protocol === "chat_completions" ? "Chat Completions" : protocol === "responses" ? "Responses" : "Messages"}</span>
                    <code>{url}</code>
                  </div>
                  <CopyButton value={url} />
                </div>
              );
            })}
          </div>
          <p className="modal-note">入口认证使用客户端 Key；管理员令牌仅用于控制台 API 和 Playground。</p>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">LOCAL DATA</div>
              <h2>数据与安全</h2>
            </div>
            <Database size={18} className="panel-icon" />
          </div>
          <ul className="security-list">
            <li><ShieldCheck size={15} /><span>Provider Key 使用本地主密钥加密，UI 仅显示前缀。</span></li>
            <li><MonitorCog size={15} /><span>管理员令牌只保存在浏览器会话存储；退出即清除。</span></li>
            <li><Database size={15} /><span>SQLite WAL 持久化请求元数据；默认不保存请求正文。</span></li>
            <li><Globe2 size={15} /><span>默认只监听 127.0.0.1；暴露到网络前请配置认证与 TLS。</span></li>
          </ul>
          <div className="readout-grid">
            <Readout label="数据库" value={`${data?.data_dir}\\selena.db`} />
            <Readout label="配置" value={`${data?.data_dir}\\config.toml`} />
          </div>
        </section>
      </div>

      <div className="settings-footer">
        <span>Selena · local API gateway</span>
        <button className="text-button" onClick={() => void copyText(gatewayUrl("/v1/responses"))}>复制 Responses 入口</button>
      </div>
    </div>
  );
}

function Readout({ label, value }: { label: string; value: string }) {
  return (
    <div className="readout">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}
