import { useQuery } from "@tanstack/react-query";
import { Filter, Search, ScrollText } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../lib/api";
import type { RequestLog } from "../lib/types";
import { protocolLabel } from "../lib/types";
import {
  EmptyState,
  Input,
  Modal,
  ProtocolBadge,
  SectionHead,
  Select,
  Spinner,
  formatMs,
  formatNumber,
  timeAgo,
} from "../components/ui";

export function Logs() {
  const logs = useQuery({ queryKey: ["requests", 100], queryFn: () => api.requests(100), refetchInterval: 4000 });
  const [protocol, setProtocol] = useState("all");
  const [status, setStatus] = useState("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<RequestLog | null>(null);

  const rows = useMemo(() => {
    const data = logs.data?.data ?? [];
    return data.filter((log) => {
      if (protocol !== "all" && log.ingress_protocol !== protocol) return false;
      if (status === "error" && log.status_code < 400) return false;
      if (status === "ok" && log.status_code >= 400) return false;
      if (query) {
        const haystack = `${log.model_requested} ${log.model_upstream} ${log.provider_name ?? ""} ${log.id}`.toLowerCase();
        if (!haystack.includes(query.toLowerCase())) return false;
      }
      return true;
    });
  }, [logs.data, protocol, query, status]);

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="LOGS / 06"
        title="请求日志"
        description="查看入口协议、上游协议、转换模式、耗时、token 与错误。默认不保存请求正文。"
        actions={<span className="live-indicator"><span className="pulse-dot pulse-ok" /> 实时刷新 · 4s</span>}
      />

      <div className="filter-bar">
        <div className="filter-search">
          <Search size={15} />
          <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索模型、Provider 或请求 ID…" />
        </div>
        <Select value={protocol} onChange={setProtocol}>
          <option value="all">全部入口协议</option>
          <option value="chat_completions">Chat Completions</option>
          <option value="responses">Responses</option>
          <option value="anthropic_messages">Messages</option>
        </Select>
        <Select value={status} onChange={setStatus}>
          <option value="all">全部状态</option>
          <option value="ok">成功</option>
          <option value="error">错误</option>
        </Select>
        <span className="filter-count"><Filter size={14} /> {rows.length} traces</span>
      </div>

      {logs.isLoading ? (
        <Spinner label="读取请求日志" />
      ) : rows.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<ScrollText size={24} />}
            title="没有匹配的请求记录"
            description="调整筛选条件，或从 Playground 发送请求。"
          />
        </div>
      ) : (
        <div className="panel log-panel">
          <div className="data-table-wrap">
            <table className="data-table log-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>入口 → 上游</th>
                  <th>模型</th>
                  <th>Provider</th>
                  <th>模式</th>
                  <th>状态</th>
                  <th>延迟</th>
                  <th>Tokens</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((log) => (
                  <tr key={log.id} onClick={() => setSelected(log)} className="clickable-row">
                    <td className="muted nowrap">{timeAgo(log.ts)}</td>
                    <td>
                      <div className="protocol-pair">
                        <ProtocolBadge protocol={log.ingress_protocol} />
                        <span>→</span>
                        <ProtocolBadge protocol={log.egress_protocol} />
                      </div>
                    </td>
                    <td>
                      <div className="model-cell">
                        <strong>{log.model_requested}</strong>
                        {log.model_upstream !== log.model_requested && <span>{log.model_upstream}</span>}
                      </div>
                    </td>
                    <td>{log.provider_name ?? "—"}</td>
                    <td><span className={`path-chip ${log.conversion_mode}`}>{log.conversion_mode}</span></td>
                    <td><span className={log.status_code >= 400 ? "status-code danger" : "status-code ok"}>{log.status_code}</span></td>
                    <td className="muted nowrap">{formatMs(log.latency_ms)}</td>
                    <td className="muted nowrap">{formatNumber((log.input_tokens ?? 0) + (log.output_tokens ?? 0))}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      <Modal open={Boolean(selected)} onClose={() => setSelected(null)} title="请求详情" eyebrow="TRACE INSPECTOR" wide>
        {selected && <LogDetail log={selected} />}
      </Modal>
    </div>
  );
}

function LogDetail({ log }: { log: RequestLog }) {
  const warnings = safeWarnings(log.conversion_warnings);
  return (
    <div className="log-detail">
      <div className="detail-hero">
        <div>
          <span className="eyebrow">REQUEST ID</span>
          <code>{log.id}</code>
        </div>
        <span className={log.status_code >= 400 ? "status-code danger" : "status-code ok"}>{log.status_code}</span>
      </div>
      <div className="detail-grid">
        <Detail label="时间" value={new Date(log.ts * 1000).toLocaleString()} />
        <Detail label="入口协议" value={protocolLabel[log.ingress_protocol] ?? log.ingress_protocol} />
        <Detail label="上游协议" value={protocolLabel[log.egress_protocol] ?? log.egress_protocol} />
        <Detail label="转换模式" value={log.conversion_mode} />
        <Detail label="请求模型" value={log.model_requested} />
        <Detail label="上游模型" value={log.model_upstream} />
        <Detail label="Provider" value={log.provider_name ?? "—"} />
        <Detail label="流式" value={log.stream ? "SSE" : "JSON"} />
        <Detail label="延迟" value={formatMs(log.latency_ms)} />
        <Detail label="TTFT" value={log.ttft_ms === null ? "—" : formatMs(log.ttft_ms)} />
        <Detail label="输入 tokens" value={String(log.input_tokens ?? "—")} />
        <Detail label="输出 tokens" value={String(log.output_tokens ?? "—")} />
        <Detail label="缓存读取" value={String(log.cached_tokens ?? "—")} />
        <Detail label="推理 tokens" value={String(log.reasoning_tokens ?? "—")} />
        <Detail label="上游 request id" value={log.upstream_request_id ?? "—"} />
        <Detail label="错误码" value={log.error_code ?? "—"} />
      </div>
      {log.error_message && (
        <div className="detail-error">
          <span className="eyebrow">ERROR</span>
          <p>{log.error_message}</p>
        </div>
      )}
      <div className="detail-section">
        <span className="eyebrow">CONVERSION WARNINGS</span>
        {warnings.length ? (
          <ul className="warning-list">
            {warnings.map((warning) => <li key={warning}>{warning}</li>)}
          </ul>
        ) : (
          <p className="muted">无转换警告。</p>
        )}
      </div>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-item">
      <span>{label}</span>
      <strong className={value.length > 24 ? "mono wrap" : ""}>{value}</strong>
    </div>
  );
}

function safeWarnings(value: string): string[] {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}
