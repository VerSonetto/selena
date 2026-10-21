import { useQuery } from "@tanstack/react-query";
import {
  ArrowRight,
  Braces,
  CircleStop,
  Copy,
  Play,
  Radio,
  Sparkles,
  TerminalSquare,
} from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { adminHeaders, api, copyText, gatewayUrl } from "../lib/api";
import { extractText, streamRequest, type StreamUpdate } from "../lib/sse";
import type { Protocol } from "../lib/types";
import { protocolLabel, protocolPath } from "../lib/types";
import {
  Button,
  CopyButton,
  Field,
  Input,
  Notice,
  ProtocolBadge,
  SectionHead,
  Select,
  Spinner,
} from "../components/ui";

const protocols: Array<{ value: Protocol; label: string }> = [
  { value: "responses", label: "Responses" },
  { value: "anthropic_messages", label: "Messages" },
  { value: "chat_completions", label: "Chat Completions" },
];

export function Playground() {
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const entries = providers.data?.data ?? [];
  const [protocol, setProtocol] = useState<Protocol>("responses");
  const [providerId, setProviderId] = useState("");
  const [upstreamModel, setUpstreamModel] = useState("gpt-4.1-mini");
  const [system, setSystem] = useState("");
  const [prompt, setPrompt] = useState("用三句话解释 Selena 为什么需要协议中间层。");
  const [stream, setStream] = useState(true);
  const [maxTokens, setMaxTokens] = useState("1024");
  const [text, setText] = useState("");
  const [raw, setRaw] = useState("");
  const [events, setEvents] = useState<StreamUpdate[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const abortRef = useRef<AbortController | null>(null);

  const selectedProvider = entries.find((entry) => entry.provider.id === providerId) ?? entries[0];
  const effectiveProvider = selectedProvider?.provider;
  const model = effectiveProvider ? `${effectiveProvider.id}/${upstreamModel}` : upstreamModel;

  const body = useMemo(() => {
    const base: Record<string, unknown> = { model, stream };
    if (protocol === "chat_completions") {
      const messages: Array<Record<string, string>> = [];
      if (system.trim()) messages.push({ role: "system", content: system.trim() });
      messages.push({ role: "user", content: prompt });
      base.messages = messages;
      if (stream) base.stream_options = { include_usage: true };
    } else if (protocol === "responses") {
      if (system.trim()) base.instructions = system.trim();
      base.input = prompt;
      if (maxTokens) base.max_output_tokens = Number(maxTokens);
    } else {
      if (system.trim()) base.system = system.trim();
      base.max_tokens = Number(maxTokens || "1024");
      base.messages = [{ role: "user", content: prompt }];
    }
    return base;
  }, [maxTokens, model, prompt, protocol, stream, system]);

  const path = protocolPath[protocol];
  const url = gatewayUrl(path);
  const curl = `curl ${url} \\\n  -H "x-selena-admin: $SELENA_ADMIN_TOKEN" \\\n  -H "Content-Type: application/json" \\\n  -d '${JSON.stringify(body)}'`;

  async function send() {
    setBusy(true);
    setError("");
    setText("");
    setRaw("");
    setEvents([]);
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    try {
      if (stream) {
        for await (const update of streamRequest(path, body)) {
          if (controller.signal.aborted) break;
          setEvents((current) => [...current.slice(-199), update]);
          const chunk = extractText(update, protocol);
          if (chunk) setText((current) => current + chunk);
          if (update.data === "[DONE]") break;
        }
      } else {
        const response = await fetch(path, {
          method: "POST",
          headers: { "content-type": "application/json", ...adminHeaders() },
          body: JSON.stringify(body),
          signal: controller.signal,
        });
        const payload = await response.json();
        setRaw(JSON.stringify(payload, null, 2));
        if (!response.ok) throw new Error(payload?.error?.message ?? `HTTP ${response.status}`);
        setText(readText(payload, protocol));
      }
    } catch (err) {
      if (!controller.signal.aborted) {
        setError(err instanceof Error ? err.message : "请求失败");
      }
    } finally {
      setBusy(false);
    }
  }

  function stop() {
    abortRef.current?.abort();
    setBusy(false);
  }

  return (
    <div className="page page-enter playground-page">
      <SectionHead
        eyebrow="PLAYGROUND / 05"
        title="协议调试台"
        description="选择入口协议和上游 Provider，发送请求并查看转换后的 SSE 事件、文本与原始 JSON。"
        actions={<ProtocolBadge protocol={protocol} />}
      />

      <div className="playground-grid">
        <section className="panel playground-config">
          <div className="panel-head">
            <div>
              <div className="eyebrow">REQUEST COMPOSER</div>
              <h2>请求</h2>
            </div>
            <span className="protocol-route">
              <ProtocolBadge protocol={protocol} />
              <ArrowRight size={14} />
              {effectiveProvider ? <ProtocolBadge protocol={effectiveProvider.protocol} /> : <span className="muted">no provider</span>}
            </span>
          </div>

          <div className="form-grid compact-grid">
            <Field label="入口协议">
              <Select value={protocol} onChange={(value) => setProtocol(value as Protocol)}>
                {protocols.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
              </Select>
            </Field>
            <Field label="Provider">
              <Select value={providerId || entries[0]?.provider.id || ""} onChange={setProviderId}>
                {entries.map((entry) => (
                  <option key={entry.provider.id} value={entry.provider.id}>
                    {entry.provider.name} · {protocolLabel[entry.provider.protocol]}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="上游模型" className="span-2" hint="请求会使用 provider/model 路由；也可以在路由页配置别名。">
              <Input value={upstreamModel} onChange={(event) => setUpstreamModel(event.target.value)} placeholder="gpt-4.1-mini" />
            </Field>
            <Field label="System / Instructions" className="span-2">
              <textarea
                className="input textarea"
                value={system}
                onChange={(event) => setSystem(event.target.value)}
                placeholder="可选。会映射到 system、instructions 或顶层 system。"
                rows={3}
              />
            </Field>
            <Field label="提示词" className="span-2">
              <textarea
                className="input textarea"
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                rows={5}
                required
              />
            </Field>
            <Field label={protocol === "anthropic_messages" ? "max_tokens" : "max output"}>
              <Input type="number" value={maxTokens} onChange={(event) => setMaxTokens(event.target.value)} min={1} />
            </Field>
            <Field label="流式">
              <button
                type="button"
                className={`stream-toggle ${stream ? "stream-on" : ""}`}
                onClick={() => setStream((value) => !value)}
              >
                <Radio size={14} />
                {stream ? "SSE 开启" : "非流式"}
              </button>
            </Field>
          </div>

          <div className="playground-actions">
            <Button onClick={send} disabled={busy || !prompt.trim() || !effectiveProvider}>
              {busy ? <CircleStop size={15} /> : <Play size={15} />}
              {busy ? "接收中…" : "发送请求"}
            </Button>
            {busy && <Button variant="ghost" onClick={stop}>停止</Button>}
            <CopyButton value={curl} label="复制 cURL" />
          </div>
          {!entries.length && <Notice tone="info">尚未配置 Provider，无法发送请求。</Notice>}
          {error && <Notice tone="danger">{error}</Notice>}
        </section>

        <section className="panel playground-output">
          <div className="panel-head">
            <div>
              <div className="eyebrow">RESPONSE</div>
              <h2>响应与事件</h2>
            </div>
            <div className="output-badges">
              <span className="event-count">{events.length} events</span>
              <ProtocolBadge protocol={protocol} />
            </div>
          </div>

          <div className="output-text">
            {text ? (
              <div className="assistant-text">{text}</div>
            ) : (
              <div className="output-placeholder">
                <Sparkles size={20} />
                <p>发送请求后，这里显示响应文本和 SSE 事件。</p>
              </div>
            )}
            {busy && <span className="typing-caret" />}
          </div>

          {events.length > 0 && (
            <div className="event-list">
              <div className="event-list-title"><TerminalSquare size={14} /> SSE timeline</div>
              {events.map((event, index) => (
                <div className="event-row" key={`${index}-${event.event ?? "data"}`}>
                  <span className="event-index">{String(index + 1).padStart(2, "0")}</span>
                  <span className="event-name">{event.event ?? (event.data === "[DONE]" ? "done" : "data")}</span>
                  <code className="event-data">{event.data.length > 260 ? `${event.data.slice(0, 260)}…` : event.data}</code>
                </div>
              ))}
            </div>
          )}

          {raw && (
            <div className="raw-block">
              <div className="event-list-title"><Braces size={14} /> raw JSON</div>
              <pre>{raw}</pre>
              <button className="code-copy" onClick={() => void copyText(raw)}><Copy size={13} /> copy</button>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function readText(payload: Record<string, any>, protocol: Protocol): string {
  if (protocol === "chat_completions") {
    return payload?.choices?.[0]?.message?.content ?? "";
  }
  if (protocol === "responses") {
    const output = payload?.output;
    if (Array.isArray(output)) {
      return output
        .flatMap((item) => (Array.isArray(item.content) ? item.content : []))
        .filter((part) => part.type === "output_text")
        .map((part) => part.text)
        .join("");
    }
    return "";
  }
  const content = payload?.content;
  return Array.isArray(content)
    ? content.filter((block) => block.type === "text").map((block) => block.text).join("")
    : "";
}
