import { useQuery } from "@tanstack/react-query";
import { BarChart3, ChartNoAxesCombined, Coins, Gauge, TrendingUp } from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "../lib/api";
import {
  EmptyState,
  SectionHead,
  Spinner,
  formatMs,
  formatNumber,
} from "../components/ui";

export function Analytics() {
  const usage = useQuery({ queryKey: ["usage"], queryFn: api.usage });
  const overview = useQuery({ queryKey: ["overview"], queryFn: api.overview });
  const data = usage.data?.data ?? [];
  const summary = overview.data;

  if (usage.isLoading) return <Spinner label="读取用量" />;

  const totalRequests = data.reduce((sum, point) => sum + point.requests, 0);
  const totalInput = data.reduce((sum, point) => sum + point.input_tokens, 0);
  const totalOutput = data.reduce((sum, point) => sum + point.output_tokens, 0);
  const totalErrors = data.reduce((sum, point) => sum + point.errors, 0);
  const successRate = totalRequests ? ((totalRequests - totalErrors) / totalRequests) * 100 : 100;

  return (
    <div className="page page-enter">
      <SectionHead
        eyebrow="ANALYTICS / 07"
        title="用量分析"
        description="最近 7 天的请求量、token、错误率和延迟。数据存储在本机 SQLite。"
      />

      <div className="metric-grid">
        <Metric icon={<TrendingUp size={17} />} label="REQUESTS / 7D" value={formatNumber(totalRequests)} note="all ingress protocols" accent="coral" />
        <Metric icon={<Coins size={17} />} label="INPUT TOKENS" value={formatNumber(totalInput)} note={`${formatNumber(totalOutput)} output`} accent="teal" />
        <Metric icon={<Gauge size={17} />} label="SUCCESS RATE" value={`${successRate.toFixed(1)}%`} note={`${totalErrors} errors`} accent="gold" />
        <Metric icon={<ChartNoAxesCombined size={17} />} label="AVG LATENCY" value={formatMs(summary?.avg_latency_ms ?? 0)} note="rolling 24h" accent="ink" />
      </div>

      <section className="panel analytics-chart">
        <div className="panel-head">
          <div>
            <div className="eyebrow">DAILY USAGE</div>
            <h2>每日请求与 token</h2>
          </div>
          <span className="chart-note"><BarChart3 size={14} /> last 7 days</span>
        </div>
        {data.length === 0 ? (
          <EmptyState title="暂无用量的数据" description="调用入口后，这里会显示每日用量。" />
        ) : (
          <div className="chart-wrap">
            <ResponsiveContainer width="100%" height={320}>
              <BarChart data={data} margin={{ top: 20, right: 10, left: -16, bottom: 0 }} barGap={4}>
                <CartesianGrid stroke="#e3d8c8" strokeDasharray="2 6" vertical={false} />
                <XAxis dataKey="day" tick={{ fill: "#7b7166", fontSize: 11 }} axisLine={false} tickLine={false} />
                <YAxis tick={{ fill: "#7b7166", fontSize: 11 }} axisLine={false} tickLine={false} />
                <Tooltip
                  contentStyle={{
                    background: "#fffbf3",
                    border: "1px solid #e3d8c8",
                    borderRadius: 10,
                    fontSize: 12,
                    color: "#211d19",
                  }}
                />
                <Bar dataKey="requests" fill="#d9634f" radius={[4, 4, 0, 0]} maxBarSize={34} />
                <Bar dataKey="input_tokens" fill="#2c6e68" radius={[4, 4, 0, 0]} maxBarSize={34} opacity={0.75} />
                <Bar dataKey="output_tokens" fill="#b8892d" radius={[4, 4, 0, 0]} maxBarSize={34} opacity={0.7} />
              </BarChart>
            </ResponsiveContainer>
            <div className="chart-legend">
              <span><i style={{ background: "#d9634f" }} /> requests</span>
              <span><i style={{ background: "#2c6e68" }} /> input tokens</span>
              <span><i style={{ background: "#b8892d" }} /> output tokens</span>
            </div>
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
        {data.length === 0 ? (
          <EmptyState title="暂无记录" description="请求会按天聚合到这里。" />
        ) : (
          <div className="data-table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>日期</th>
                  <th>请求</th>
                  <th>错误</th>
                  <th>输入 tokens</th>
                  <th>输出 tokens</th>
                  <th>缓存读取</th>
                  <th>推理 tokens</th>
                </tr>
              </thead>
              <tbody>
                {data.map((point) => (
                  <tr key={point.day}>
                    <td className="mono-cell">{point.day}</td>
                    <td>{formatNumber(point.requests)}</td>
                    <td className={point.errors ? "danger-text" : "muted"}>{point.errors}</td>
                    <td>{formatNumber(point.input_tokens)}</td>
                    <td>{formatNumber(point.output_tokens)}</td>
                    <td className="muted">{formatNumber(point.cached_tokens)}</td>
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
