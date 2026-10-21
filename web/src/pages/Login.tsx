import { useState } from "react";
import { ArrowRight, KeyRound, Sparkles } from "lucide-react";
import { api, setToken } from "../lib/api";
import { Button, Field, Input, ProtocolBadge } from "../components/ui";

export function Login({ onLogin }: { onLogin: (token: string) => void }) {
  const [token, setTokenValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setError("");
    setBusy(true);
    try {
      await api.login(token.trim());
      onLogin(token.trim());
      setToken(token.trim());
    } catch (err) {
      setError(err instanceof Error ? err.message : "无法验证令牌");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="login-page">
      <section className="login-story">
        <div className="login-grain" />
        <div className="login-eyebrow">
          <Sparkles size={14} /> SELENA / LOCAL CONTROL ROOM
        </div>
        <h1>
          本地 API
          <br />
          聚合<em>控制台</em>
        </h1>
        <p className="login-lede">
          管理 Chat Completions、Responses、Messages 三种协议入口，将请求路由到任意协议的上游，
          并查看日志、用量与转换状态。
        </p>
        <div className="login-protocols">
          <ProtocolBadge protocol="chat_completions" />
          <span className="protocol-line" />
          <ProtocolBadge protocol="responses" />
          <span className="protocol-line" />
          <ProtocolBadge protocol="anthropic_messages" />
        </div>
        <div className="login-footnote">LOCAL SERVICE · ADMIN ACCESS</div>
      </section>

      <section className="login-panel">
        <div className="login-card">
          <div className="login-card-mark">
            <KeyRound size={18} />
          </div>
          <div className="eyebrow">ADMIN ACCESS</div>
          <h2>管理员登录</h2>
          <p>输入启动 Selena 时生成的管理员令牌。令牌仅保存在当前浏览器，本地服务不会上传数据。</p>
          <form onSubmit={submit}>
            <Field
              label="管理员令牌"
              hint="由 selena web / desktop 启动时打印，也可以在 runtime.json 中找到。"
            >
              <Input
                type="password"
                autoFocus
                value={token}
                onChange={(event) => setTokenValue(event.target.value)}
                placeholder="sk-admin-…"
              />
            </Field>
            {error && <div className="form-error">{error}</div>}
            <Button type="submit" disabled={busy || !token.trim()} className="login-submit">
              {busy ? "验证中…" : "打开控制台"}
              <ArrowRight size={16} />
            </Button>
          </form>
          <div className="login-hint">
            <span className="pulse-dot pulse-ok" />
            本地服务 · 无云端依赖
          </div>
        </div>
        <div className="login-copyright">
          <span>Selena 0.1</span>
          <span>Ivory Studio</span>
        </div>
      </section>
    </main>
  );
}
