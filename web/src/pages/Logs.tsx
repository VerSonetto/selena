import { useQuery } from "@tanstack/react-query";
import { Filter, RefreshCw, Search, ScrollText } from "lucide-react";
import { useDeferredValue, useState } from "react";
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
  const [protocol, setProtocol] = useState("all");
  const [status, setStatus] = useState("all");
  const [stream, setStream] = useState("all");
  const [providerId, setProviderId] = useState("all");
  const [days, setDays] = useState("7");
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query.trim());
  const [limit, setLimit] = useState(100);
  const [selected, setSelected] = useState<RequestLog | null>(null);
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });

  const logs = useQuery({
    queryKey: ["requests", protocol, status, stream, providerId, days, deferredQuery, limit],
    queryFn: () =>
      api.requests({
        limit,
        offset: 0,
        protocol: protocol === "all" ? undefined : protocol,
        status: status === "all" ? undefined : status,
        provider_id: providerId === "all" ? undefined : providerId,
        stream: stream === "all" ? undefined : stream === "stream",
        search: deferredQuery || undefined,
        days: days === "all" ? undefined : Number(days),
      }),
    refetchInterval: 4000,
  });

  const rows = logs.data?.data ?? [];
  const total = logs.data?.total ?? rows.length;
  const providerEntries = providers.data?.data ?? [];

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="LOGS / 06"
        title="请求日志"
        description="逐条查看入口协议、上游转换、延迟、TTFT、token 与错误。不保存请求正文。"
        actions={<span className="live-indicator"><span className="pulse-dot pulse-ok" /> 实时刷新 · 4s</span>}
      />

      <div className="filter-bar log-filter-bar">
        <div className="filter-search">
          <Search size={15} />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索模型、Provider 或请求 ID…"
          />
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
        <Select value={stream} onChange={setStream}>
          <option value="all">全部模式</option>
          <option value="stream">流式 SSE</option>
          <option value="json">非流式 JSON</option>
        </Select>
        <Select value={providerId} onChange={setProviderId}>
          <option value="all">全部 Provider</option>
          {providerEntries.map((entry) => (
            <option key={entry.provider.id} value={entry.provider.id}>{entry.provider.name}</option>
          ))}
        </Select>
        <Select value={days} onChange={setDays}>
          <option value="1">最近 24 小时</option>
          <option value="7">最近 7 天</option>
          <option value="30">最近 30 天</option>
          <option value="all">全部时间</option>
        </Select>
        <span className="filter-count"><Filter size={14} /> {formatNumber(total)} traces</span>
      </div>

      {logs.isLoading ? (
        <Spinner label="读取请求日志" />
      ) : rows.length === 0 ? (
        <div className="panel">
          <EmptyState
            icon={<ScrollText size={24} />}
            title="没有匹配的请求记录"
            description="调整筛选条件，或从 Playground / 客户端发送请求。"
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
                  <th>警告</th>
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
                    <td className="muted nowrap">
                      <span className="latency-cell">
                        {formatMs(log.latency_ms)}
                        {log.ttft_ms !== null && <em>TTFT {formatMs(log.ttft_ms)}</em>}
                      </span>
                    </td>
                    <td className="nowrap"><TokenCell log={log} /></td>
                    <td>
                      {safeWarnings(log.conversion_warnings).length > 0 ? (
                        <span
                          className="warning-count"
                          title={safeWarnings(log.conversion_warnings).join("\n")}
                        >
                          {safeWarnings(log.conversion_warnings).length}
                        </span>
                      ) : (
                        <span className="muted">—</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <div className="log-foot">
            <span>显示 {formatNumber(rows.length)} / {formatNumber(total)} 条</span>
            <div className="log-foot-actions">
              <button className="btn btn-ghost btn-sm" onClick={() => logs.refetch()} disabled={logs.isFetching}>
                <RefreshCw size={14} className={logs.isFetching ? "spin" : ""} /> 刷新
              </button>
              {logs.data?.has_more && (
                <button className="btn btn-secondary btn-sm" onClick={() => setLimit((value) => value + 100)}>
                  加载更多
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      <Modal open={Boolean(selected)} onClose={() => setSelected(null)} title="请求详情" eyebrow="TRACE INSPECTOR" wide>
        {selected && <LogDetail log={selected} />}
      </Modal>
    </div>
  );
}

function TokenCell({ log }: { log: RequestLog }) {
  const total = tokenTotal(log);
  if (total === null) return <span className="muted">—</span>;
  return (
    <span className="token-cell">
      <strong>{formatNumber(total)}</strong>
      <em>{formatNumber(log.input_tokens)} in · {formatNumber(log.output_tokens)} out</em>
    </span>
  );
}

function LogDetail({ log }: { log: RequestLog }) {
  const warnings = safeWarnings(log.conversion_warnings);
  const total = tokenTotal(log);
  const hasUsage = total !== null || log.cached_tokens !== null || log.reasoning_tokens !== null;
  return (
    <div className="log-detail">
      <div className="detail-hero">
        <div>
          <span className="eyebrow">REQUEST ID</span>
          <code>{log.id}</code>
          <p className="detail-route">
            <ProtocolBadge protocol={log.ingress_protocol} />
            <span>→</span>
            <ProtocolBadge protocol={log.egress_protocol} />
            <span className={`path-chip ${log.conversion_mode}`}>{log.conversion_mode}</span>
            <span className="muted">{log.stream ? "SSE" : "JSON"}</span>
          </p>
        </div>
        <span className={log.status_code >= 400 ? "status-code danger" : "status-code ok"}>{log.status_code}</span>
      </div>

      <div className="record-metrics">
        <RecordMetric label="延迟" value={formatMs(log.latency_ms)} />
        <RecordMetric label="TTFT" value={log.ttft_ms === null ? "—" : formatMs(log.ttft_ms)} />
        <RecordMetric label="Tokens" value={total === null ? "—" : formatNumber(total)} />
        <RecordMetric label="输出速度" value={outputSpeed(log)} />
      </div>

      <div className="detail-grid">
        <Detail label="时间" value={new Date(log.ts * 1000).toLocaleString()} />
        <Detail label="请求模型" value={log.model_requested} />
        <Detail label="上游模型" value={log.model_upstream} />
        <Detail label="Provider" value={log.provider_name ?? "—"} />
        <Detail label="入口协议" value={protocolLabel[log.ingress_protocol] ?? log.ingress_protocol} />
        <Detail label="上游协议" value={protocolLabel[log.egress_protocol] ?? log.egress_protocol} />
        <Detail label="上游 request id" value={log.upstream_request_id ?? "—"} />
        <Detail label="错误码" value={log.error_code ?? "—"} />
      </div>

      <div className="detail-section">
        <span className="eyebrow">TOKEN BREAKDOWN</span>
        {hasUsage ? (
          <div className="token-breakdown-grid">
            <TokenValue label="输入" value={log.input_tokens} />
            <TokenValue label="输出" value={log.output_tokens} />
            <TokenValue label="总计" value={total} />
            <TokenValue label="缓存读取" value={log.cached_tokens} />
            <TokenValue label="缓存写入" value={log.cache_write_tokens} />
            <TokenValue label="推理" value={log.reasoning_tokens} />
          </div>
        ) : (
          <p className="muted">上游没有返回 usage；Selena 不会用本地估算填充。</p>
        )}
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

function TokenValue({ label, value }: { label: string; value: number | null }) {
  return (
    <div className="token-value">
      <span>{label}</span>
      <strong>{value === null ? "—" : formatNumber(value)}</strong>
    </div>
  );
}

function RecordMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="record-metric">
      <span>{label}</span>
      <strong>{value}</strong>
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

function tokenTotal(log: RequestLog): number | null {
  if (log.total_tokens !== null) return log.total_tokens;
  if (log.input_tokens === null && log.output_tokens === null) return null;
  return (log.input_tokens ?? 0) + (log.output_tokens ?? 0);
}

function outputSpeed(log: RequestLog): string {
  if (!log.output_tokens || !log.latency_ms) return "—";
  return `${(log.output_tokens / (log.latency_ms / 1000)).toFixed(1)} tok/s`;
}

function safeWarnings(value: string): string[] {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}
