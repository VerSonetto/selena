import { adminHeaders } from "./api";

export interface StreamUpdate {
  event?: string;
  data: string;
  parsed?: unknown;
}

export async function* streamRequest(
  path: string,
  body: Record<string, unknown>,
): AsyncGenerator<StreamUpdate> {
  const response = await fetch(path, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      accept: "text/event-stream",
      ...adminHeaders(),
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `HTTP ${response.status}`);
  }
  if (!response.body) throw new Error("stream body is unavailable");
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let eventName: string | undefined;
  const dataLines: string[] = [];

  const flush = (): StreamUpdate | null => {
    if (dataLines.length === 0 && !eventName) return null;
    const data = dataLines.join("\n");
    dataLines.length = 0;
    const event = eventName;
    eventName = undefined;
    if (!data) return null;
    let parsed: unknown = undefined;
    if (data.trim() !== "[DONE]") {
      try {
        parsed = JSON.parse(data);
      } catch {
        parsed = undefined;
      }
    }
    return { event, data, parsed };
  };

  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      let newline = buffer.indexOf("\n");
      while (newline >= 0) {
        let line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        if (line.endsWith("\r")) line = line.slice(0, -1);
        if (line === "") {
          const update = flush();
          if (update) yield update;
        } else if (line.startsWith("event:")) {
          eventName = line.slice(6).trim();
        } else if (line.startsWith("data:")) {
          dataLines.push(line.slice(5).trimStart());
        }
        newline = buffer.indexOf("\n");
      }
    }
    const last = flush();
    if (last) yield last;
  } finally {
    reader.releaseLock();
  }
}

export function extractText(update: StreamUpdate, protocol: string): string {
  const data = update.parsed as Record<string, unknown> | undefined;
  if (!data) return "";
  if (protocol === "chat_completions") {
    const choices = data.choices as Array<{ delta?: { content?: string } }> | undefined;
    return choices?.[0]?.delta?.content ?? "";
  }
  if (protocol === "responses") {
    if (data.type === "response.output_text.delta" && typeof data.delta === "string") {
      return data.delta;
    }
    return "";
  }
  if (protocol === "anthropic_messages") {
    const delta = data.delta as { type?: string; text?: string } | undefined;
    return delta?.type === "text_delta" ? (delta.text ?? "") : "";
  }
  return "";
}

export function isTerminal(update: StreamUpdate, protocol: string): boolean {
  const data = update.parsed as { type?: string } | undefined;
  if (protocol === "chat_completions") return update.data === "[DONE]";
  if (protocol === "responses") return data?.type === "response.completed" || data?.type === "response.failed" || data?.type === "response.incomplete";
  return data?.type === "message_stop";
}
