# Selena HTTP API

## Gateway endpoints

| Method | Path | Description |
|---|---|---|
| POST | `/v1/chat/completions` | OpenAI Chat Completions ingress |
| POST | `/v1/responses` | OpenAI Responses ingress |
| POST | `/v1/messages` | Anthropic Messages ingress |
| GET | `/v1/models` | `provider/...` entries |
| GET | `/health` | Local liveness |

Authentication accepts `Authorization: Bearer <client-key>` or `x-api-key: <client-key>`. If no client keys exist, loopback requests are allowed for first-run. The admin token is also accepted for the Playground. Client keys with `rate_limit_rpm` are enforced per-key in a one-minute window.

Model selection:

1. `provider/model` forces a provider; the suffix is the upstream model.
2. Otherwise the default provider or first enabled provider is used.

### Example: Responses ingress to a Messages provider

```bash
curl http://127.0.0.1:13697/v1/responses \
  -H "Authorization: Bearer $SELENA_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "my-anthropic-provider/claude-sonnet",
    "input": "Explain protocol translation in one paragraph.",
    "stream": true
  }'
```

Same-protocol requests preserve native SSE semantics. Cross-protocol requests emit the ingress protocol's event lifecycle; native-only features that cannot be represented fail with `unsupported_feature`.

## Admin endpoints

All admin routes except `POST /api/auth/login` require `x-selena-admin: <runtime-token>`.

| Method | Path | Description |
|---|---|---|
| POST | `/api/auth/login` | Validate a token |
| GET | `/api/auth/session` | Validate current admin token |
| GET | `/api/overview` | Counts, recent traces, health summary |
| GET/POST | `/api/providers` | List/create providers |
| PATCH/DELETE | `/api/providers/{id}` | Update/delete provider |
| GET/POST | `/api/providers/{id}/keys` | List/create provider keys |
| DELETE | `/api/providers/{id}/keys/{key_id}` | Delete key |
| PATCH | `/api/providers/{id}/keys/{key_id}/enabled` | Enable/disable key |
| POST | `/api/providers/{id}/test` | Check key availability |
| GET/POST | `/api/client-keys` | List/create client keys |
| DELETE | `/api/client-keys/{id}` | Delete client key |
| PATCH | `/api/client-keys/{id}/enabled` | Enable/disable client key |
| GET | `/api/model-pool?refresh=true` | Models discovered from each provider's `/models` endpoint |
| GET | `/api/requests?limit=100` | Request traces |
| GET | `/api/usage` | Last-seven-day daily usage |
| GET/PUT | `/api/settings` | Default provider, theme, configured port |
| GET | `/api/capabilities` | Cross-protocol capability summary |

## Error shapes

OpenAI-style ingress:

```json
{"error":{"message":"...","type":"invalid_request_error","param":null,"code":"unsupported_feature"}}
```

Anthropic-style ingress:

```json
{"type":"error","error":{"type":"invalid_request_error","message":"..."},"request_id":null}
```

Streaming errors after headers are sent use the ingress protocol's native error event. A stream that fails is never terminated with a successful event.

## Usage notes

- `GET /api/requests` returns metadata only; request bodies are not persisted by default.
- `GET /api/usage` aggregates by UTC day from `request_logs`.
- Provider keys are encrypted at rest; admin APIs return only labels and prefixes.
- Provider models are fetched from `GET {base_url}/models` with the provider's auth headers, cached for 120 seconds, and can be refreshed explicitly. A provider that does not implement `/models` reports an error without affecting other providers.
