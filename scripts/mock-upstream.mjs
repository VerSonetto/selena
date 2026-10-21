import http from "node:http";

const port = Number(process.env.MOCK_PORT || 8899);

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

function sse(res, events) {
  res.writeHead(200, {
    "content-type": "text/event-stream; charset=utf-8",
    "cache-control": "no-cache",
    connection: "keep-alive",
    "x-request-id": `mock_${Date.now()}`,
  });
  let index = 0;
  const write = () => {
    if (index >= events.length) {
      res.end();
      return;
    }
    const event = events[index++];
    if (typeof event === "string") res.write(event);
    else res.write(event);
    setTimeout(write, 80);
  };
  write();
}

function json(res, body, status = 200) {
  res.writeHead(status, {
    "content-type": "application/json",
    "x-request-id": `mock_${Date.now()}`,
  });
  res.end(JSON.stringify(body));
}

function socketName() {
  return "mock-model";
}

const server = http.createServer(async (req, res) => {
  if (req.method === "GET" && req.url === "/v1/models") {
    const anthropic = Boolean(req.headers["x-api-key"]);
    const ids = anthropic
      ? ["claude-mock-sonnet", "claude-mock-haiku"]
      : ["mock-model", "mock-mini", "mock-vision"];
    json(res, {
      object: "list",
      data: ids.map((id, index) => ({
        id,
        type: "model",
        display_name: id.replaceAll("-", " "),
        created_at: 1700000000 + index,
        owned_by: anthropic ? "anthropic-mock" : "openai-mock",
      })),
    });
    return;
  }
  if (req.method !== "POST") {
    json(res, { error: { message: "not found" } }, 404);
    return;
  }
  const body = JSON.parse((await readBody(req)) || "{}");
  if (req.url === "/v1/chat/completions") {
    if (body.stream) {
      const id = `chatcmpl-mock-${Date.now()}`;
      const chunk = (delta, finish = null) =>
        `data: ${JSON.stringify({
          id,
          object: "chat.completion.chunk",
          created: Math.floor(Date.now() / 1000),
          model: socketName(),
          choices: [{ index: 0, delta, finish_reason: finish }],
        })}\n\n`;
      sse(res, [
        chunk({ role: "assistant", content: "" }),
        chunk({ content: "Hello" }),
        chunk({ content: " from Selena" }),
        chunk({ content: " mock." }),
        chunk({}, "stop"),
        `data: ${JSON.stringify({
          id,
          object: "chat.completion.chunk",
          created: Math.floor(Date.now() / 1000),
          model: socketName(),
          choices: [],
          usage: { prompt_tokens: 18, completion_tokens: 7, total_tokens: 25 },
        })}\n\n`,
        "data: [DONE]\n\n",
      ]);
      return;
    }
    json(res, {
      id: `chatcmpl-mock-${Date.now()}`,
      object: "chat.completion",
      created: Math.floor(Date.now() / 1000),
      model: socketName(),
      choices: [{
        index: 0,
        message: { role: "assistant", content: "Hello from Selena mock (Chat Completions)." },
        finish_reason: "stop",
      }],
      usage: { prompt_tokens: 18, completion_tokens: 9, total_tokens: 27 },
    });
    return;
  }

  if (req.url === "/v1/responses") {
    const id = `resp_mock_${Date.now()}`;
    if (body.stream) {
      const event = (name, payload) =>
        `event: ${name}\ndata: ${JSON.stringify({ type: name, ...payload, sequence_number: 1 })}\n\n`;
      sse(res, [
        event("response.created", {
          response: { id, object: "response", created_at: Math.floor(Date.now() / 1000), status: "in_progress", model: socketName(), output: [] },
        }),
        event("response.output_item.added", {
          output_index: 0,
          item: { id: "msg_mock", type: "message", role: "assistant", content: [], status: "in_progress" },
        }),
        event("response.content_part.added", {
          item_id: "msg_mock", output_index: 0, content_index: 0,
          part: { type: "output_text", text: "", annotations: [], logprobs: [] },
        }),
        event("response.output_text.delta", { item_id: "msg_mock", output_index: 0, content_index: 0, delta: "Hello" }),
        event("response.output_text.delta", { item_id: "msg_mock", output_index: 0, content_index: 0, delta: " from Selena" }),
        event("response.output_text.delta", { item_id: "msg_mock", output_index: 0, content_index: 0, delta: " mock." }),
        event("response.output_text.done", { item_id: "msg_mock", output_index: 0, content_index: 0, text: "Hello from Selena mock.", logprobs: [] }),
        event("response.completed", {
          response: {
            id, object: "response", created_at: Math.floor(Date.now() / 1000), status: "completed", model: socketName(),
            output: [{ id: "msg_mock", type: "message", role: "assistant", content: [{ type: "output_text", text: "Hello from Selena mock.", annotations: [], logprobs: [] }], status: "completed" }],
            usage: { input_tokens: 18, output_tokens: 7, total_tokens: 25 },
          },
        }),
      ]);
      return;
    }
    json(res, {
      id,
      object: "response",
      created_at: Math.floor(Date.now() / 1000),
      status: "completed",
      model: socketName(),
      output: [{
        id: "msg_mock",
        type: "message",
        role: "assistant",
        content: [{ type: "output_text", text: "Hello from Selena mock (Responses).", annotations: [], logprobs: [] }],
        status: "completed",
      }],
      usage: { input_tokens: 18, output_tokens: 9, total_tokens: 27 },
    });
    return;
  }

  if (req.url === "/v1/messages") {
    const id = `msg_mock_${Date.now()}`;
    if (body.stream) {
      sse(res, [
        `event: message_start\ndata: ${JSON.stringify({ type: "message_start", message: { id, type: "message", role: "assistant", content: [], model: socketName(), stop_reason: null, stop_sequence: null, usage: { input_tokens: 18, output_tokens: 0 } } })}\n\n`,
        `event: content_block_start\ndata: ${JSON.stringify({ type: "content_block_start", index: 0, content_block: { type: "text", text: "" } })}\n\n`,
        `event: content_block_delta\ndata: ${JSON.stringify({ type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "Hello" } })}\n\n`,
        `event: content_block_delta\ndata: ${JSON.stringify({ type: "content_block_delta", index: 0, delta: { type: "text_delta", text: " from Selena" } })}\n\n`,
        `event: content_block_delta\ndata: ${JSON.stringify({ type: "content_block_delta", index: 0, delta: { type: "text_delta", text: " mock." } })}\n\n`,
        `event: content_block_stop\ndata: ${JSON.stringify({ type: "content_block_stop", index: 0 })}\n\n`,
        `event: message_delta\ndata: ${JSON.stringify({ type: "message_delta", delta: { stop_reason: "end_turn", stop_sequence: null }, usage: { input_tokens: 18, output_tokens: 7, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 } })}\n\n`,
        `event: message_stop\ndata: ${JSON.stringify({ type: "message_stop" })}\n\n`,
      ]);
      return;
    }
    json(res, {
      id,
      type: "message",
      role: "assistant",
      model: socketName(),
      content: [{ type: "text", text: "Hello from Selena mock (Messages)." }],
      stop_reason: "end_turn",
      stop_sequence: null,
      usage: { input_tokens: 18, output_tokens: 9, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 },
    });
    return;
  }

  json(res, { error: { message: `unknown path ${req.url}` } }, 404);
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Selena mock upstream listening on http://127.0.0.1:${port}`);
});
