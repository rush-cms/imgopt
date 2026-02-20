# POST /convert

Accepts an image via multipart upload, optionally resizes it, and returns the converted bytes in WebP or AVIF format.

---

## Authentication

All requests to `/convert` must include a `Bearer` token:

```
Authorization: Bearer <API_TOKEN>
```

Requests without the header, or with an incorrect token, receive `401 Unauthorized`.

---

## Request

```
POST /convert
Content-Type: multipart/form-data
Authorization: Bearer <token>
```

### Parameters

| Field | Type | Required | Default | Constraints | Description |
|-------|------|----------|---------|-------------|-------------|
| `file` | file | **yes** | — | ≤ `MAX_UPLOAD_MB` | Source image. Accepted formats: JPEG, PNG, GIF, WebP, BMP, TIFF. |
| `format` | string | no | `webp` | `webp`, `avif` | Output format. |
| `quality` | number | no | `80` | `1–100` | Encoder quality. Lower = smaller file, higher = better quality. |
| `width` | integer | no | — | `1–4096` | Target width in pixels. Aspect ratio is preserved if `height` is omitted. |
| `height` | integer | no | — | `1–4096` | Target height in pixels. Aspect ratio is preserved if `width` is omitted. |

**Resize behaviour:**

| `width` | `height` | Result |
|---------|----------|--------|
| set | omitted | Scales to the given width, preserving aspect ratio |
| omitted | set | Scales to the given height, preserving aspect ratio |
| set | set | Resizes to exact dimensions (may change aspect ratio) |
| omitted | omitted | No resize — only format conversion |

**Source image limits:**

- Max dimension per side: **4096 px**
- Max total pixels: **16 000 000** (~4 K resolution)

Requests that exceed these limits are rejected with `400`.

---

## Response

### Success — `200 OK`

The response body contains the raw converted image bytes.

| Header | Example | Description |
|--------|---------|-------------|
| `Content-Type` | `image/webp` | MIME type of the output (`image/webp` or `image/avif`). |
| `X-Request-Id` | `550e8400-e29b-41d4-a716-446655440000` | Unique ID for this request. Use it to correlate logs. |

### Error codes

| Status | When |
|--------|------|
| `400 Bad Request` | Missing `file` field, invalid parameter value, or source image exceeds size limits. |
| `401 Unauthorized` | Missing or incorrect `Authorization` header. |
| `408 Request Timeout` | Encoding took longer than 30 seconds. |
| `422 Unprocessable Entity` | File is not a valid or supported image. |
| `500 Internal Server Error` | Unexpected server error. |

---

## Examples

### curl

```bash
# Convert to WebP (default)
curl -s -X POST http://localhost:3000/convert \
  -H "Authorization: Bearer your_token" \
  -F "file=@photo.jpg" \
  -F "quality=80" \
  --output photo.webp

# Convert to AVIF with resize
curl -s -X POST http://localhost:3000/convert \
  -H "Authorization: Bearer your_token" \
  -F "file=@photo.jpg" \
  -F "format=avif" \
  -F "quality=70" \
  -F "width=1200" \
  --output photo.avif

# Resize only by width (aspect ratio preserved)
curl -s -X POST http://localhost:3000/convert \
  -H "Authorization: Bearer your_token" \
  -F "file=@photo.png" \
  -F "width=800" \
  --output photo_resized.webp
```

### PHP / Laravel

```php
use Illuminate\Support\Facades\Http;

$response = Http::withToken(config('services.imgopt.token'))
    ->attach('file', file_get_contents($localPath), basename($localPath))
    ->post(config('services.imgopt.url') . '/convert', [
        'format'  => 'webp',
        'quality' => 80,
        'width'   => 1200,
    ]);

if ($response->successful()) {
    $requestId = $response->header('X-Request-Id');
    Storage::put('images/output.webp', $response->body());
}
```

`config/services.php`:

```php
'imgopt' => [
    'url'   => env('IMGOPT_URL', 'http://imgopt:3000'),
    'token' => env('IMGOPT_TOKEN'),
],
```

### Python

```python
import requests

with open("photo.jpg", "rb") as f:
    response = requests.post(
        "http://localhost:3000/convert",
        headers={"Authorization": "Bearer your_token"},
        files={"file": ("photo.jpg", f, "image/jpeg")},
        data={"format": "webp", "quality": "80", "width": "1200"},
    )

response.raise_for_status()
request_id = response.headers.get("X-Request-Id")
with open("photo.webp", "wb") as out:
    out.write(response.content)
```

---

## Quality guidelines

| Use case | Format | Quality |
|----------|--------|---------|
| Thumbnails | `webp` | 60–70 |
| General web images | `webp` | 75–85 |
| Hero / full-quality | `webp` | 85–90 |
| Maximum compression | `avif` | 60–75 |
| High-fidelity archive | `avif` | 80–90 |

AVIF produces smaller files than WebP at equivalent quality, but encoding is slower (~5–10×). Prefer WebP for latency-sensitive paths and AVIF for background jobs or pre-generated assets.

---

## Tracing requests

Every response includes an `X-Request-Id` header. When reporting a bug or investigating an error, include this ID so it can be matched against server logs:

```bash
curl -v -X POST http://localhost:3000/convert \
  -H "Authorization: Bearer your_token" \
  -F "file=@photo.jpg" \
  --output out.webp 2>&1 | grep -i x-request-id
```

Server logs are emitted as structured JSON and include `request_id`, `format`, `file_size`, `output_size`, and `duration_ms` on each conversion.
