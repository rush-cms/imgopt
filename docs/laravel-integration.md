# Laravel Integration

How to integrate imgopt into a Laravel application.

---

## Configuration

### `config/services.php`

```php
'imgopt' => [
    'url'           => env('IMGOPT_URL', 'http://imgopt:3000'),
    'token'         => env('IMGOPT_TOKEN'),
    'timeout'       => env('IMGOPT_TIMEOUT', 35), // slightly above the server-side 30s encoding timeout
    'max_upload_mb' => env('IMGOPT_MAX_UPLOAD_MB', 10),
],
```

### `.env`

```env
# On the same Coolify network, use the internal service name
IMGOPT_URL=http://imgopt:3000
IMGOPT_TOKEN=your_secret_token_here

# For local development (cargo run or docker compose up)
# IMGOPT_URL=http://localhost:3000
```

---

## Service class

`app/Services/ImgOptService.php`

```php
<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use RuntimeException;

class ImgOptService
{
    private string $url;
    private string $token;
    private int $timeout;

    public function __construct()
    {
        // config() returns mixed — cast explicitly so PHPStan is satisfied
        $this->url     = rtrim((string) config('services.imgopt.url', ''), '/');
        $this->token   = (string) config('services.imgopt.token', '');
        $this->timeout = (int) config('services.imgopt.timeout', 35);
    }

    /**
     * Convert an image and return the raw bytes.
     *
     * @param  string $filePath Absolute path to the source image
     * @param  array{
     *   format?:  'webp'|'avif',
     *   quality?: int,
     *   width?:   int,
     *   height?:  int,
     * } $options
     * @return array{ bytes: string, mime: string, request_id: string }
     *
     * @throws RuntimeException
     */
    public function convert(string $filePath, array $options = []): array
    {
        $options = array_merge([
            'format'  => 'webp',
            'quality' => 80,
        ], $options);

        // file_get_contents() returns string|false — must check before use
        $fileContents = file_get_contents($filePath);

        if ($fileContents === false) {
            throw new RuntimeException("Cannot read file at path: {$filePath}");
        }

        $request = Http::withToken($this->token)
            ->timeout($this->timeout)
            ->attach('file', $fileContents, basename($filePath));

        foreach (['format', 'quality', 'width', 'height'] as $field) {
            if (isset($options[$field])) {
                $request = $request->attach($field, (string) $options[$field], $field);
            }
        }

        $response = $request->post("{$this->url}/convert");

        if ($response->failed()) {
            // header() returns string (empty string when absent) — no second argument
            $requestId = $response->header('X-Request-Id') ?: 'unknown';

            Log::error('imgopt conversion failed', [
                'status'     => $response->status(),
                'body'       => $response->body(),
                'file'       => $filePath,
                'options'    => $options,
                'request_id' => $requestId,
            ]);

            throw new RuntimeException(
                "imgopt returned {$response->status()} (request-id: {$requestId})"
            );
        }

        return [
            'bytes'      => $response->body(),
            'mime'       => $response->header('Content-Type'),
            'request_id' => $response->header('X-Request-Id'),
        ];
    }

    /**
     * Convert and save directly to a path.
     *
     * @param  string $sourcePath
     * @param  string $destinationPath
     * @param  array{
     *   format?:  'webp'|'avif',
     *   quality?: int,
     *   width?:   int,
     *   height?:  int,
     * } $options
     * @return array{ path: string, mime: string, size: int, request_id: string }
     *
     * @throws RuntimeException
     */
    public function convertAndSave(string $sourcePath, string $destinationPath, array $options = []): array
    {
        $result = $this->convert($sourcePath, $options);

        // file_put_contents() returns int|false — must check for failure
        $written = file_put_contents($destinationPath, $result['bytes']);

        if ($written === false) {
            throw new RuntimeException("Failed to write converted image to: {$destinationPath}");
        }

        return [
            'path'       => $destinationPath,
            'mime'       => $result['mime'],
            'size'       => $written,
            'request_id' => $result['request_id'],
        ];
    }

    /**
     * Convert from a URL (downloads first, then converts).
     *
     * @param  string $imageUrl
     * @param  array{
     *   format?:  'webp'|'avif',
     *   quality?: int,
     *   width?:   int,
     *   height?:  int,
     * } $options
     * @return array{ bytes: string, mime: string, request_id: string }
     *
     * @throws RuntimeException
     */
    public function convertFromUrl(string $imageUrl, array $options = []): array
    {
        // tempnam() returns string|false — must check before use
        $tempPath = tempnam(sys_get_temp_dir(), 'imgopt_');

        if ($tempPath === false) {
            throw new RuntimeException('Failed to create temporary file for image download');
        }

        try {
            $response = Http::timeout(60)->get($imageUrl);

            if ($response->failed()) {
                throw new RuntimeException("Failed to download image from: {$imageUrl}");
            }

            $written = file_put_contents($tempPath, $response->body());

            if ($written === false) {
                throw new RuntimeException("Failed to write downloaded image to temp file");
            }

            return $this->convert($tempPath, $options);
        } finally {
            @unlink($tempPath);
        }
    }

    /**
     * Check if the imgopt service is reachable.
     */
    public function isHealthy(): bool
    {
        try {
            return Http::timeout(5)
                ->get("{$this->url}/health")
                ->successful();
        } catch (\Throwable) {
            return false;
        }
    }
}
```

---

## Binding

Register the service as a singleton so the config is only read once.

`app/Providers/AppServiceProvider.php`

```php
use App\Services\ImgOptService;

public function register(): void
{
    $this->app->singleton(ImgOptService::class);
}
```

---

## Usage examples

### In a controller

```php
use App\Services\ImgOptService;
use Illuminate\Http\UploadedFile;

class MediaController extends Controller
{
    public function store(Request $request, ImgOptService $imgopt): JsonResponse
    {
        $request->validate(['image' => 'required|image|max:10240']);

        // file() returns UploadedFile|UploadedFile[]|null — assert the expected type
        $uploaded = $request->file('image');
        assert($uploaded instanceof UploadedFile);

        $result = $imgopt->convert($uploaded->getRealPath(), [
            'format'  => 'webp',
            'quality' => 82,
            'width'   => 1200,
        ]);

        $path = 'media/' . uniqid() . '.webp';
        Storage::put($path, $result['bytes']);

        return response()->json([
            'path'       => $path,
            'mime'       => $result['mime'],
            'request_id' => $result['request_id'],
        ]);
    }
}
```

### In a queued job

`app/Jobs/OptimizeImageJob.php`

```php
<?php

namespace App\Jobs;

use App\Models\Media;
use App\Services\ImgOptService;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Illuminate\Support\Facades\Log;

class OptimizeImageJob implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public int $tries   = 3;
    public int $timeout = 60;

    /**
     * @param int    $mediaId
     * @param string $sourcePath
     * @param array{
     *   format?:  'webp'|'avif',
     *   quality?: int,
     *   width?:   int,
     *   height?:  int,
     * } $options
     */
    public function __construct(
        private readonly int    $mediaId,
        private readonly string $sourcePath,
        private readonly array  $options = [],
    ) {}

    public function handle(ImgOptService $imgopt): void
    {
        $media = Media::findOrFail($this->mediaId);

        $result = $imgopt->convertAndSave(
            sourcePath:      storage_path("app/{$this->sourcePath}"),
            destinationPath: storage_path("app/optimized/{$media->id}.webp"),
            options:         array_merge(['format' => 'webp', 'quality' => 82], $this->options),
        );

        $media->update([
            'optimized_path' => "optimized/{$media->id}.webp",
            'optimized_size' => $result['size'],
            'optimized_at'   => now(),
        ]);

        Log::info('Image optimized', [
            'media_id'   => $this->mediaId,
            'size'       => $result['size'],
            'request_id' => $result['request_id'],
        ]);
    }

    public function failed(\Throwable $e): void
    {
        Log::error('OptimizeImageJob failed', [
            'media_id' => $this->mediaId,
            'error'    => $e->getMessage(),
        ]);
    }
}
```

Dispatching:

```php
OptimizeImageJob::dispatch($media->id, $media->path)
    ->onQueue('images');
```

### In the WordPress importer context

```php
// After parsing a post from the XML and saving it:
foreach ($post->attachments as $attachment) {
    OptimizeImageJob::dispatch(
        mediaId:    $attachment->id,
        sourcePath: $attachment->local_path,
        options:    ['format' => 'webp', 'quality' => 80, 'width' => 1920],
    )->onQueue('images');
}
```

---

## Handling failures gracefully

imgopt is a non-critical dependency — content should still be saved even if optimization fails.

```php
public function handle(ImgOptService $imgopt): void
{
    try {
        $result = $imgopt->convert($this->sourcePath, $this->options);
        // save optimized version...
    } catch (\RuntimeException $e) {
        // Log and fall back to the original file
        Log::warning('imgopt unavailable, keeping original', [
            'error' => $e->getMessage(),
            'file'  => $this->sourcePath,
        ]);
        // optionally: re-queue with a delay
        // $this->release(60);
    }
}
```

---

## Testing

Use `Http::fake()` so tests never hit the real service.

```php
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Http;
use App\Services\ImgOptService;

class ImgOptServiceTest extends TestCase
{
    public function test_convert_returns_webp_bytes(): void
    {
        // file_get_contents() returns string|false — assert before passing to Http::fake()
        $fixture = file_get_contents(base_path('tests/fixtures/sample.webp'));
        assert($fixture !== false, 'Test fixture file not found');

        Http::fake([
            '*/convert' => Http::response(
                body:    $fixture,
                status:  200,
                headers: [
                    'Content-Type' => 'image/webp',
                    'X-Request-Id' => 'test-uuid-1234',
                ],
            ),
        ]);

        $result = app(ImgOptService::class)->convert(
            base_path('tests/fixtures/sample.jpg'),
            ['format' => 'webp', 'quality' => 80],
        );

        $this->assertSame('image/webp', $result['mime']);
        $this->assertSame('test-uuid-1234', $result['request_id']);
        $this->assertNotEmpty($result['bytes']);
    }

    public function test_convert_throws_on_server_error(): void
    {
        Http::fake(['*/convert' => Http::response('Processing failed', 422)]);

        $this->expectException(\RuntimeException::class);

        app(ImgOptService::class)->convert(
            base_path('tests/fixtures/sample.jpg'),
        );
    }

    public function test_is_healthy_returns_false_when_unreachable(): void
    {
        // body must be string, not null
        Http::fake(['*/health' => Http::response('', 500)]);

        $this->assertFalse(app(ImgOptService::class)->isHealthy());
    }
}
```

---

## Queue configuration

Separate queues keep imports from blocking image optimization and vice versa.

`config/horizon.php` (or `config/queue.php` with supervisor):

```php
'environments' => [
    'production' => [
        'supervisor-imports' => [
            'queue'     => ['imports'],
            'processes' => 2,   // heavy, few workers
            'timeout'   => 300,
        ],
        'supervisor-images' => [
            'queue'     => ['images'],
            'processes' => 5,   // lighter, more workers
            'timeout'   => 90,
        ],
        'supervisor-default' => [
            'queue'     => ['default'],
            'processes' => 3,
            'timeout'   => 60,
        ],
    ],
],
```
