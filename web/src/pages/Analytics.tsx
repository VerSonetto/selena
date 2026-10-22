import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  BarChart3,
  ChartNoAxesCombined,
  Coins,
  Gauge,
  RefreshCw,
  TrendingUp,
} from "lucide-react";
import { useState } from "react";
import {
  Bar,
  CartesianGrid,
  ComposedChart,
  Line,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "../lib/api";
import { protocolLabel } from "../lib/types";
import {
  EmptyState,
  Notice,
  SectionHead,
  Select,
  Spinner,
  formatMs,
  formatNumber,
} from "../components/ui";

type Dimension = "provider" | "model" | "protocol";

export function Analytics() {
  const [days, setDays] = useState("7");
  const [dimension, setDimension] = useState<Dimension>("provider");
  const usage = useQuery({
    queryKey: ["usage", days],
    queryFn: () => api.usage(Number(days)),
    refetchInterval: 30_000,
  });

  if (usage.isLoading) return <Spinner label="读取用量" />;
  if (usage.error) {
    return (
      <div className="page">
        <EmptyState
          title="无法读取用量"
          description={usage.error instanceof Error ? usage.error.message : "请检查本地服务状态。"}
        />
      </div>
    );
  }

  const report = usage.data;
  const summary = report?.summary;
  const daily = report?.daily ?? [];
  const hasRequests = (summary?.requests ?? 0) > 0;
  const hasUsage = (summary?.usage_requests ?? 0) > 0;
  const successRate = summary?.requests
    ? ((summary.requests - summary.errors) / summary.requests) * 100
    : 100;
  const breakdown =
    dimension === "provider"
      ? report?.by_provider ?? []
      : dimension === "model"
        ? report?.by_model ?? []
        : report?.by_protocol ?? [];

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="ANALYTICS / 07"
        title="用量分析"
        description="按请求、token 与延迟拆解上游使用情况。未知用量不会被记为零。"
        actions={
          <div className="analytics-actions">
            <Select value={days} onChange={setDays}>
              <option value="1">最近 24 小时</option>
              <option value="7">最近 7 天</option>
              <option value="30">最近 30 天</option>
            </Select>
            <button
              className="btn btn-secondary btn-md"
              onClick={() => usage.refetch()}
              disabled={usage.isFetching}
            >
              <RefreshCw size={15} className={usage.isFetching ? "spin" : ""} />
              刷新
            </button>
          </div>
        }
      />

      <div className="metric-grid">
        <Metric
          icon={<TrendingUp size={17} />}
          label="REQUESTS"
          value={formatNumber(summary?.requests ?? 0)}
          note={`${formatNumber(summary?.errors ?? 0)} errors · ${formatNumber(summary?.stream_requests ?? 0)} streams`}
          accent="coral"
        />
        <Metric
          icon={<Coins size={17} />}
          label="TOTAL TOKENS"
          value={hasUsage ? formatNumber(summary?.total_tokens ?? 0) : "—"}
          note={
            hasUsage
              ? `${formatNumber(summary?.input_tokens ?? 0)} in · ${formatNumber(summary?.output_tokens ?? 0)} out`
              : `${formatNumber(summary?.usage_requests ?? 0)} / ${formatNumber(summary?.requests ?? 0)} requests reported usage`
          }
          accent="teal"
        />
        <Metric
          icon={<Gauge size={17} />}
          label="SUCCESS RATE"
          value={`${successRate.toFixed(1)}%`}
          note={`P50 ${formatMs(summary?.p50_latency_ms ?? 0)} · P95 ${formatMs(summary?.p95_latency_ms ?? 0)}`}
          accent="gold"
        />
        <Metric
          icon={<ChartNoAxesCombined size={17} />}
          label="AVG LATENCY"
          value={hasRequests ? formatMs(summary?.avg_latency_ms ?? 0) : "—"}
          note={
            summary?.avg_ttft_ms === null || summary?.avg_ttft_ms === undefined
              ? "TTFT 暂无数据"
              : `TTFT ${formatMs(summary.avg_ttft_ms)}`
          }
          accent="ink"
        />
      </div>

      {!hasUsage && hasRequests && (
        <Notice tone="info">
          请求已被记录，但上游没有返回 usage。Selena 会将 token 保持为未知，不做本地估算。
        </Notice>
      )}

      <section className="panel token-ledger-panel">
        <div className="panel-head">
          <div>
            <div className="eyebrow">TOKEN LEDGER</div>
            <h2>Token 构成</h2>
          </div>
          <span className="chart-note">
            <Activity size={14} />
            {hasUsage
              ? `${formatNumber(summary?.usage_requests ?? 0)} / ${formatNumber(summary?.requests ?? 0)} 有用量`
              : "等待上游 usage"}
          </span>
        </div>
        <div className="token-ledger">
          <div className="token-ledger-total">
            <span className="eyebrow">BILLED FLOW</span>
            <strong>{hasUsage ? formatNumber(summary?.total_tokens ?? 0) : "—"}</strong>
            <p>
              输入 {hasUsage ? formatNumber(summary?.input_tokens ?? 0) : "—"} · 输出{" "}
              {hasUsage ? formatNumber(summary?.output_tokens ?? 0) : "—"}
            </p>
            <span className="token-ledger-note">
              缓存读取与推理 token 是子集，不重复计入总量。
            </span>
          </div>
          <div className="token-mix">
            <TokenMixRow label="输入" value={summary?.input_tokens} total={summary?.total_tokens} tone="teal" />
            <TokenMixRow label="输出" value={summary?.output_tokens} total={summary?.total_tokens} tone="gold" />
            <TokenMixRow label="缓存读取" value={summary?.cached_tokens} total={summary?.total_tokens} tone="teal-soft" />
            <TokenMixRow label="缓存写入" value={summary?.cache_write_tokens} total={summary?.total_tokens} tone="ink" />
            <TokenMixRow label="推理" value={summary?.reasoning_tokens} total={summary?.output_tokens} tone="coral" />
          </div>
        </div>
      </section>

      <section className="panel analytics-chart">
        <div className="panel-head">
          <div>
            <div className="eyebrow">DAILY USAGE</div>
            <h2>每日 token 与请求</h2>
          </div>
          <span className="chart-note"><BarChart3 size={14} /> {days === "1" ? "24h" : `${days} days`}</span>
        </div>
        {!hasRequests ? (
          <EmptyState title="暂无用量数据" description="调用任一协议入口后，这里会按天聚合请求与 token。" />
        ) : (
          <div className="chart-wrap">
            <ResponsiveContainer width="100%" height={340}>
              <ComposedChart data={daily} margin={{ top: 20, right: 8, left: -12, bottom: 0 }} barGap={2}>
                <CartesianGrid stroke="#e3d8c8" strokeDasharray="2 6" vertical={false} />
                <XAxis dataKey="key" tick={{ fill: "#7b7166", fontSize: 11 }} axisLine={false} tickLine={false} />
                <YAxis
                  yAxisId="tokens"
                  tick={{ fill: "#7b7166", fontSize: 11 }}
                  axisLine={false}
                  tickLine={false}
                />
                <YAxis
                  yAxisId="requests"
                  orientation="right"
                  tick={{ fill: "#b0a496", fontSize: 11 }}
                  axisLine={false}
                  tickLine={false}
                  allowDecimals={false}
                />
                <Tooltip
                  cursor={{ fill: "rgba(217, 99, 79, 0.06)" }}
                  contentStyle={{
                    background: "#fffbf3",
                    border: "1px solid #e3d8c8",
                    borderRadius: 10,
                    fontSize: 12,
                    color: "#211d19",
                  }}
                />
                <Bar yAxisId="tokens" dataKey="input_tokens" name="输入 tokens" stackId="tokens" fill="#2c6e68" maxBarSize={34} />
                <Bar yAxisId="tokens" dataKey="output_tokens" name="输出 tokens" stackId="tokens" fill="#b8892d" maxBarSize={34} radius={[4, 4, 0, 0]} />
                <Line
                  yAxisId="requests"
                  type="monotone"
                  dataKey="requests"
                  name="请求"
                  stroke="#d9634f"
                  strokeWidth={1.8}
                  dot={{ r: 2.5, fill: "#d9634f" }}
                  activeDot={{ r: 4 }}
                />
              </ComposedChart>
            </ResponsiveContainer>
            <div className="chart-legend">
              <span><i style={{ background: "#2c6e68" }} /> input tokens</span>
              <span><i style={{ background: "#b8892d" }} /> output tokens</span>
              <span><i style={{ background: "#d9634f" }} /> requests</span>
            </div>
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-head">
          <div>
            <div className="eyebrow">BREAKDOWN</div>
            <h2>用量拆分</h2>
          </div>
          <Select value={dimension} onChange={(value) => setDimension(value as Dimension)}>
            <option value="provider">按 Provider</option>
            <option value="model">按模型</option>
            <option value="protocol">按入口协议</option>
          </Select>
        </div>
        {breakdown.length === 0 ? (
          <EmptyState title="暂无拆分数据" description="有请求后这里会显示分组用量。" />
        ) : (
          <div className="data-table-wrap">
            <table className="data-table usage-breakdown-table">
              <thead>
                <tr>
                  <th>{dimension === "provider" ? "Provider" : dimension === "model" ? "模型" : "入口协议"}</th>
                  <th>请求</th>
                  <th>错误</th>
                  <th>输入</th>
                  <th>输出</th>
                  <th>总计</th>
                  <th>缓存读 / 写</th>
                  <th>推理</th>
                  <th>平均延迟</th>
                </tr>
              </thead>
              <tbody>
                {breakdown.map((row) => (
                  <tr key={`${dimension}-${row.key}`}>
                    <td>
                      <strong>{dimension === "protocol" ? protocolLabel[row.label] ?? row.label : row.label}</strong>
                    </td>
                    <td>{formatNumber(row.requests)}</td>
                    <td className={row.errors ? "danger-text" : "muted"}>{formatNumber(row.errors)}</td>
                    <td>{formatNumber(row.input_tokens)}</td>
                    <td>{formatNumber(row.output_tokens)}</td>
                    <td><strong>{formatNumber(row.total_tokens)}</strong></td>
                    <td className="muted">
                      {formatNumber(row.cached_tokens)} / {formatNumber(row.cache_write_tokens)}
                    </td>
                    <td className="muted">{formatNumber(row.reasoning_tokens)}</td>
                    <td className="muted">{formatMs(row.avg_latency_ms)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-head">
          <div>
            <div className="eyebrow">DAILY LEDGER</div>
            <h2>逐日明细</h2>
          </div>
        </div>
        {!hasRequests ? (
          <EmptyState title="暂无记录" description="请求会按天聚合到这里。" />
        ) : (
          <div className="data-table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>日期</th>
                  <th>请求</th>
                  <th>错误</th>
                  <th>输入</th>
                  <th>输出</th>
                  <th>总计</th>
                  <th>缓存读</th>
                  <th>缓存写</th>
                  <th>推理</th>
                </tr>
              </thead>
              <tbody>
                {daily.map((point) => (
                  <tr key={point.key}>
                    <td className="mono-cell">{point.key}</td>
                    <td>{formatNumber(point.requests)}</td>
                    <td className={point.errors ? "danger-text" : "muted"}>{point.errors}</td>
                    <td>{formatNumber(point.input_tokens)}</td>
                    <td>{formatNumber(point.output_tokens)}</td>
                    <td><strong>{formatNumber(point.total_tokens)}</strong></td>
                    <td className="muted">{formatNumber(point.cached_tokens)}</td>
                    <td className="muted">{formatNumber(point.cache_write_tokens)}</td>
                    <td className="muted">{formatNumber(point.reasoning_tokens)}</td>
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

function TokenMixRow({
  label,
  value,
  total,
  tone,
}: {
  label: string;
  value?: number;
  total?: number;
  tone: string;
}) {
  const known = value !== undefined && value !== null;
  const percent = known && total ? Math.min(100, (value / total) * 100) : 0;
  return (
    <div className="token-mix-row">
      <span className="token-mix-label">{label}</span>
      <div className="token-mix-track">
        <i className={`token-mix-fill tone-${tone}`} style={{ width: `${percent}%` }} />
      </div>
      <strong>{known ? formatNumber(value) : "—"}</strong>
      <em>{known && total ? `${percent.toFixed(percent < 10 ? 1 : 0)}%` : ""}</em>
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
