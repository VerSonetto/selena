# Protocol Research Notes

Retrieved and cross-checked on 2026-10-22. These notes drive `selena-protocol`. Official docs are live and may evolve; unknown fields/events are tolerated by the codecs.

## Endpoints and auth

| | Chat Completions | Responses | Anthropic Messages |
|---|---|---|---|
| Endpoint | `POST /v1/chat/completions` | `POST /v1/responses` | `POST /v1/messages` |
| Auth | `Authorization: Bearer` | `Authorization: Bearer` | `x-api-key` |
| Version | REST v1 | REST v1 | `anthropic-version: 2023-06-01` in examples |
| Core input | `messages[]` | `input` + `instructions` | `messages[]` + top-level `system` |
| Stream terminal | `data: [DONE]` | `response.completed` / `failed` / `incomplete` | `message_stop` |
| Native state | `store` (not conversation state) | `store`, `previous_response_id`, `conversation` | none |
| Native tools | functions + custom | functions + hosted tools | client tools + server tools |

## Request field map

| Concept | Chat Completions | Responses | Messages |
|---|---|---|---|
| System | `system` / `developer` messages | `instructions` | top-level `system` |
| Text | text parts | `input_text` | `text` block |
| Image | `image_url {url, detail}` | `input_image` | `image {source}` |
| File/PDF | `file {file_data, file_id, filename}` | `input_file` | `document` |
| Function tool | `{type:"function", function:{name, parameters, strict}}` | flat `{type:"function", name, parameters, strict}` | `{name, input_schema, strict}` |
| Tool call | assistant `tool_calls[]` | `function_call` item | `tool_use` block |
| Tool result | `tool` message | `function_call_output` item | `tool_result` block |
| Tool choice | `auto/none/required` + named | `auto/none/required` + named/allowed | `auto/any/tool/none` + `disable_parallel_tool_use` |
| Structured output | `response_format` | `text.format` | `output_config.format` |
| Max output | `max_completion_tokens` | `max_output_tokens` | required `max_tokens` |
| Sampling | temperature 0–2, top_p, penalties, seed | temperature 0–2, top_p | temperature 0–1, top_p, top_k (newer models may reject) |
| Streaming | `stream`, `stream_options` | `stream`, terminal usage | `stream` |

## Response field map

| Concept | Chat Completions | Responses | Messages |
|---|---|---|---|
| Object | `chat.completion` | `response` | `message` |
| ID | `chatcmpl-…` | `resp_…` | `msg_…` |
| Text | `choices[].message.content` | message item `output_text` | `text` block |
| Refusal | `message.refusal` | `refusal` content part | `stop_reason: refusal` |
| Tool calls | `message.tool_calls[]` | `output[].function_call` | `content[].tool_use` |
| Reasoning | no standard field | `reasoning` item | `thinking` / `redacted_thinking` |
| Finish | `finish_reason` | `status` + `incomplete_details.reason` | `stop_reason` + `stop_sequence` |
| Usage | prompt/completion/total + details | input/output/total + details | input/output + cache fields |

## Streaming events

| Concept | Chat Completions | Responses | Messages |
|---|---|---|---|
| Framing | `data:` only | `event:` + `data:` | `event:` + `data:` |
| Start | first chunk | `response.created`, `response.in_progress` | `message_start` |
| Text delta | `delta.content` | `response.output_text.delta` | `content_block_delta` with `text_delta` |
| Text end | finish reason | `response.output_text.done` | `content_block_stop` |
| Tool start | `delta.tool_calls[]` | `response.output_item.added` | `content_block_start` with `tool_use` |
| Tool args | `tool_calls[].function.arguments` | `response.function_call_arguments.delta` | `input_json_delta.partial_json` |
| Usage | `include_usage` chunk | terminal response | `message_delta` cumulative usage |
| Terminal | `[DONE]` | `response.completed` | `message_stop` |
| Heartbeat | SSE comments possible | SSE comments possible | `ping` event |
| Error | no standard in-stream event | `error` event / `response.failed` | `error` event |

### Streaming conversion rules

- Same protocol is a byte-level passthrough; do not re-encode.
- Cross protocol: upstream decoder → canonical events → ingress encoder. Ensure item/block `start` precedes deltas and `done/stop` follows them.
- Responses requires monotonic `sequence_number`; Anthropic has block indices; Chat has tool-call indices. Preserve stable IDs.
- Parallel tool arguments may interleave in Chat. Messages requires sequential block lifecycles; buffer tool arguments and flush in order when converting to Messages.
- Chat `[DONE]` is never forwarded to Responses or Messages.
- Wait for usage before a terminal event if the target requires it. Never emit a successful terminal after an error.

## Stop and status mapping

| Chat `finish_reason` | Responses | Messages |
|---|---|---|
| `stop` | `completed` | `end_turn` |
| `length` | `incomplete` / `max_output_tokens` | `max_tokens` |
| `tool_calls` / `function_call` | `completed` | `tool_use` |
| `content_filter` | `incomplete` / `content_filter` | `refusal` |
| null / disconnect | `failed` or error event | `error` event |

Messages `pause_turn` maps to a completed Chat/Responses stop with loss of continuation semantics; `model_context_window_exceeded` maps to `length` (lossy). Stop sequence identity is lost when converting from Chat.

## Usage mapping

```
Chat → Responses:
  input = prompt_tokens, output = completion_tokens, total = total_tokens
  cached = prompt_tokens_details.cached_tokens
  cache_write = prompt_tokens_details.cache_write_tokens
  reasoning = completion_tokens_details.reasoning_tokens

Chat → Messages:
  cache_read = prompt_tokens_details.cached_tokens
  cache_creation = prompt_tokens_details.cache_write_tokens
  input = max(prompt_tokens - cache_read - cache_creation, 0)
  output = completion_tokens
```

Anthropic total input = `input_tokens + cache_creation_input_tokens + cache_read_input_tokens`. Treat this formula as approximate if an upstream reports cache write fields ambiguously.

## Fail-fast boundary

Cross-protocol requests return `unsupported_feature` for:

- Responses state (`store: true`, `previous_response_id`, `conversation`)
- Responses hosted tools and hosted outputs
- Anthropic server tools / MCP / containers
- reasoning/thinking content and signatures
- audio
- Chat `n > 1`
- prompt templates and unsupported native output

Same-protocol passthrough preserves these features because the provider is native for that protocol.

## Official references

- OpenAI Chat Completions reference: https://developers.openai.com/api/reference/resources/chat
- OpenAI Chat streaming events: https://developers.openai.com/api/reference/resources/chat/subresources/completions/streaming-events
- OpenAI Responses create: https://developers.openai.com/api/reference/resources/responses/methods/create
- OpenAI Responses streaming events: https://developers.openai.com/api/reference/resources/responses/streaming-events
- OpenAI streaming guide: https://developers.openai.com/api/docs/guides/streaming-responses
- OpenAI error codes: https://developers.openai.com/api/docs/guides/error-codes
- OpenAI OpenAPI spec: https://github.com/openai/openai-openapi
- Anthropic Create a Message: https://platform.claude.com/docs/en/api/messages/create
- Anthropic streaming: https://platform.claude.com/docs/en/build-with-claude/streaming
- Anthropic errors: https://platform.claude.com/docs/en/api/errors
- Anthropic versioning: https://platform.claude.com/docs/en/api/versioning
