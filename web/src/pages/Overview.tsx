import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  ArrowUpRight,
  Cable,
  CircleDollarSign,
  Gauge,
  Plus,
  TerminalSquare,
  Waypoints,
} from "lucide-react";
import { Link } from "react-router-dom";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
} from "recharts";
import { api, copyText, gatewayUrl } from "../lib/api";
import type { Protocol } from "../lib/types";
import { protocolLabel, protocolPath } from "../lib/types";
import {
  CopyButton,
  EmptyState,
  ProtocolBadge,
  SectionHead,
  Spinner,
  formatMs,
  formatNumber,
  timeAgo,
} from "../components/ui";

const endpointMeta: Array<{ protocol: Protocol; caption: string; tone: string }> = [
  { protocol: "responses", caption: "输入项 / 输出项 · 语义事件", tone: "coral" },
  { protocol: "anthropic_messages", caption: "内容块 · thinking / server tools", tone: "gold" },
  { protocol: "chat_completions", caption: "消息数组 · 兼容性最广", tone: "teal" },
];

export function Overview() {
  const overview = useQuery({ queryKey: ["overview"], queryFn: api.overview, refetchInterval: 5000 });
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const usage = useQuery({ queryKey: ["usage"], queryFn: () => api.usage() });
  const requests = useQuery({ queryKey: ["requests", 8], queryFn: () => api.requests(8) });

  if (overview.isLoading) return <Spinner label="正在读取 Selena 状态" />;
  if (overview.error) {
    return (
      <div className="page">
        <EmptyState
          title="无法连接本地服务"
          description={overview.error instanceof Error ? overview.error.message : "请确认 selena web 正在运行。"}
        />
      </div>
    );
  }

  const data = overview.data!;
  const successRate = data.requests_today
    ? ((data.requests_today - data.errors_today) / data.requests_today) * 100
    : 100;
  const providerEntries = providers.data?.data ?? [];
  const usageData = usage.data?.daily ?? [];
  const recent = requests.data?.data ?? data.recent_requests;

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="OVERVIEW / 01"
        title="服务概览"
        description="查看服务状态、协议入口、近 7 天流量和上游 Provider 状态。"
        actions={
          <>
            <Link className="btn btn-secondary btn-md" to="/playground">
              <TerminalSquare size={15} /> 打开 Playground
            </Link>
            <Link className="btn btn-primary btn-md" to="/providers">
              <Plus size={15} /> 添加 Provider
            </Link>
          </>
        }
      />

      <div className="endpoint-grid">
        {endpointMeta.map((item, index) => {
          const path = protocolPath[item.protocol];
          const url = gatewayUrl(path);
          const body =
            item.protocol === "chat_completions"
              ? `{"model":"your-model","messages":[{"role":"user","content":"Hello"}]}`
              : item.protocol === "responses"
                ? `{"model":"your-model","input":"Hello"}`
                : `{"model":"your-model","max_tokens":256,"messages":[{"role":"user","content":"Hello"}]}`;
          const curl = `curl ${url} \\\n  -H "Authorization: Bearer $SELENA_API_KEY" \\\n  -H "Content-Type: application/json" \\\n  -d '${body}'`;
          return (
            <article className={`endpoint-card endpoint-${item.tone}`} key={item.protocol}>
              <div className="endpoint-top">
                <span className="endpoint-index">0{index + 1}</span>
                <ProtocolBadge protocol={item.protocol} />
              </div>
              <h3>{protocolLabel[item.protocol]}</h3>
              <p>{item.caption}</p>
              <div className="endpoint-path">
                <code>{path}</code>
                <CopyButton value={url} label="复制 URL" />
              </div>
              <details className="endpoint-curl">
                <summary>
                  <Cable size={13} /> cURL
                </summary>
                <div className="code-inline">
                  <pre>{curl}</pre>
                  <button
                    className="code-copy"
                    onClick={() => void copyText(curl)}
                    aria-label="复制 cURL"
                  >
                    copy
                  </button>
                </div>
              </details>
            </article>
          );
        })}
      </div>

      <div className="metric-grid">
        <Metric
          icon={<Activity size={17} />}
          label="REQUESTS / 24H"
          value={formatNumber(data.requests_today)}
          note={`${data.errors_today} errors`}
          accent="coral"
        />
        <Metric
          icon={<Gauge size={17} />}
          label="SUCCESS RATE"
          value={`${successRate.toFixed(1)}%`}
          note={data.errors_today ? "inspect traces" : "all clear"}
          accent="teal"
        />
        <Metric
          icon={<CircleDollarSign size={17} />}
          label="TOKENS / 24H"
          value={formatNumber(data.input_tokens_today + data.output_tokens_today)}
          note={`${formatNumber(data.input_tokens_today)} in · ${formatNumber(data.output_tokens_today)} out`}
          accent="gold"
        />
        <Metric
          icon={<Waypoints size={17} />}
          label="AVG LATENCY"
          value={formatMs(data.avg_latency_ms)}
          note={`${data.enabled_providers}/${data.providers} providers ready`}
          accent="ink"
        />
      </div>

      <div className="overview-split">
        <section className="panel chart-panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">TRAFFIC / 7 DAYS</div>
              <h2>近 7 天流量</h2>
            </div>
            <Link className="quiet-link" to="/analytics">
              查看分析 <ArrowUpRight size={14} />
            </Link>
          </div>
          {usageData.length === 0 ? (
            <div className="chart-empty">
              <div className="chart-empty-line" />
              <p>暂无流量数据。调用任一入口后，这里会显示请求与 token 趋势。</p>
            </div>
          ) : (
            <div className="chart-wrap">
              <ResponsiveContainer width="100%" height={228}>
                <AreaChart data={usageData} margin={{ top: 12, right: 8, left: -18, bottom: 0 }}>
                  <defs>
                    <linearGradient id="requestsFill" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#d9634f" stopOpacity={0.28} />
                      <stop offset="100%" stopColor="#d9634f" stopOpacity={0.01} />
                    </linearGradient>
                    <linearGradient id="tokensFill" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#2c6e68" stopOpacity={0.2} />
                      <stop offset="100%" stopColor="#2c6e68" stopOpacity={0.01} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid stroke="#e3d8c8" strokeDasharray="2 6" vertical={false} />
                  <XAxis dataKey="key" tick={{ fill: "#7b7166", fontSize: 11 }} axisLine={false} tickLine={false} />
                  <Tooltip
                    contentStyle={{
                      background: "#fffbf3",
                      border: "1px solid #e3d8c8",
                      borderRadius: 10,
                      color: "#211d19",
                      fontSize: 12,
                    }}
                  />
                  <Area type="monotone" dataKey="requests" stroke="#d9634f" strokeWidth={2} fill="url(#requestsFill)" />
                  <Area type="monotone" dataKey="input_tokens" stroke="#2c6e68" strokeWidth={1.5} fill="url(#tokensFill)" />
                </AreaChart>
              </ResponsiveContainer>
              <div className="chart-legend">
                <span><i style={{ background: "#d9634f" }} /> requests</span>
                <span><i style={{ background: "#2c6e68" }} /> input tokens</span>
              </div>
            </div>
          )}
        </section>

        <section className="panel health-panel">
          <div className="panel-head">
            <div>
              <div className="eyebrow">UPSTREAM STATUS</div>
              <h2>Provider 状态</h2>
            </div>
            <Link className="quiet-link" to="/providers">
              管理 <ArrowUpRight size={14} />
            </Link>
          </div>
          {providerEntries.length === 0 ? (
            <EmptyState
              icon={<Waypoints size={20} />}
              title="尚未配置 Provider"
              description="添加 Provider 并录入 API Key 后，才能路由请求。"
              action={<Link className="btn btn-primary btn-sm" to="/providers">添加 Provider</Link>}
            />
          ) : (
            <div className="health-list">
              {providerEntries.map((entry) => (
                <div className="health-row" key={entry.provider.id}>
                  <span className={entry.provider.enabled && entry.ready_keys > 0 ? "pulse-dot pulse-ok" : "pulse-dot pulse-warn"} />
                  <div className="health-name">
                    <strong>{entry.provider.name}</strong>
                    <span>{entry.provider.base_url.replace(/^https?:\/\//, "")}</span>
                  </div>
                  <ProtocolBadge protocol={entry.provider.protocol} />
                  <span className="health-keys">{entry.ready_keys}/{entry.key_count} keys</span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>

      <section className="panel recent-panel">
        <div className="panel-head">
          <div>
            <div className="eyebrow">RECENT REQUESTS</div>
            <h2>最近请求</h2>
          </div>
          <Link className="quiet-link" to="/logs">全部日志 <ArrowUpRight size={14} /></Link>
        </div>
        {recent.length === 0 ? (
          <EmptyState title="暂无请求记录" description="调用任一入口后，请求会显示在这里。" />
        ) : (
          <div className="data-table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>入口</th>
                  <th>模型</th>
                  <th>Provider</th>
                  <th>路径</th>
                  <th>状态</th>
                  <th>延迟</th>
                </tr>
              </thead>
              <tbody>
                {recent.map((log) => (
                  <tr key={log.id}>
                    <td className="muted">{timeAgo(log.ts)}</td>
                    <td><ProtocolBadge protocol={log.ingress_protocol} /></td>
                    <td className="mono-cell">{log.model_requested}</td>
                    <td>{log.provider_name ?? "—"}</td>
                    <td><span className={`path-chip ${log.conversion_mode}`}>{log.conversion_mode}</span></td>
                    <td><span className={log.status_code >= 400 ? "status-code danger" : "status-code ok"}>{log.status_code}</span></td>
                    <td className="muted">{formatMs(log.latency_ms)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

function Metric({
  icon,
  label,
  value,
  note,
  accent,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  note: string;
  accent: string;
}) {
  return (
    <article className={`metric-card accent-${accent}`}>
      <div className="metric-top">
        <span className="metric-icon">{icon}</span>
        <span className="metric-label">{label}</span>
      </div>
      <strong>{value}</strong>
      <span className="metric-note">{note}</span>
    </article>
  );
}
