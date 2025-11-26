# Wasm ABI Specification

This document describes the Application Binary Interface (ABI) for WebAssembly modules running on Vortex.

## Overview

Vortex modules are standard WebAssembly modules that export a `handle_request` function and optionally import host functions for logging and HTTP requests.

## Required Exports

### `handle_request() -> i32`

Every Vortex module **must** export this function.

**Signature:**
```wat
(func (export "handle_request") (result i32)
  ;; Your logic here
  i32.const 0  ;; Return offset where response string starts
)
```

**Returns:** Memory offset (i32) where the HTTP response string begins.

**Response Format:**
The response at the returned memory offset should be a valid HTTP response string:
```
HTTP/1.1 200 OK\r\n
Content-Length: 13\r\n
\r\n
Hello, World!
```

### `memory` Export

Modules **must** export their linear memory:
```wat
(memory (export "memory") 1)  ;; At least 1 page (64KB)
```

## Optional Imports

### `env.log(ptr: i32, len: i32)`

Write a log message to the server's stdout.

**Parameters:**
- `ptr`: Memory offset of the UTF-8 string
- `len`: Length of the string in bytes

**Example:**
```wat
(import "env" "log" (func $log (param i32 i32)))

(func (export "handle_request") (result i32)
  ;; Log "Request received"
  (call $log (i32.const 100) (i32.const 16))
  i32.const 0
)

(data (i32.const 100) "Request received")
```

### `env.fetch(url_ptr: i32, url_len: i32) -> i32`

Make an HTTP GET request to an external URL.

**Parameters:**
- `url_ptr`: Memory offset of the URL string
- `url_len`: Length of the URL in bytes

**Returns:** Memory offset where the response body starts, or `-1` on error.

**Example:**
```wat
(import "env" "fetch" (func $fetch (param i32 i32) (result i32)))

(func (export "handle_request") (result i32)
  ;; Fetch from https://api.example.com/data
  (call $fetch (i32.const 200) (i32.const 28))
  drop
  
  ;; Return our response
  i32.const 0
)

(data (i32.const 200) "https://api.example.com/data")
```

## Memory Layout Recommendations

Organize module memory for clarity:

```
0x0000 - 0x1000:  Response buffer
0x1000 - 0x2000:  Logging/fetch result buffers
0x2000 - 0x3000:  Static strings (URLs, messages)
0x3000+   :       Dynamic allocations
```

## Return Codes (Special Behavior)

While `handle_request` returns a memory offset, Vortex recognizes special response patterns:

- **`"403"`**: Return just the string "403" to trigger a 403 Forbidden response
- **`"HTTP/1.1 XXX"`**: Full HTTP response (parsed by server)
- **Error (trap)**: Wasm trap → 500 Internal Server Error

## Complete Example

```wat
(module
  ;; Import host functions
  (import "env" "log" (func $log (param i32 i32)))
  (import "env" "fetch" (func $fetch (param i32 i32) (result i32)))
  
  ;; Export memory and handler
  (memory (export "memory") 1)
  (func (export "handle_request") (result i32)
    ;; Log request
    (call $log (i32.const 500) (i32.const 18))
    
    ;; Fetch external data
    (call $fetch (i32.const 520) (i32.const 24))
    drop
    
    ;; Return response at offset 0
    i32.const 0
  )
  
  ;; Response
  (data (i32.const 0) "HTTP/1.1 200 OK\\r\\nContent-Length: 32\\r\\n\\r\\n{\\"status\\":\\"ok\\",\\"data\\":\\"fetched\\"}")
  
  ;; Log message
  (data (i32.const 500) "Processing request")
  
  ;; Fetch URL
  (data (i32.const 520) "https://api.example.com/")
)
```

## Pipeline Execution

When multiple modules are loaded (pipeline mode):
1. Request flows through modules **sequentially**
2. If any module returns `"403"` or errors, pipeline **stops**
3. The **last successful** module's response is sent to the client

**Example pipeline:**
```bash
vortex-core auth.wat rate_limiter.wat business_logic.wat
```

- `auth.wat`: Checks authentication, returns "403" if unauthorized
- `rate_limiter.wat`: Checks rate limits, returns "429" if exceeded
- `business_logic.wat`: Processes request, returns JSON response

## Limitations

- No filesystem access (WASI is limited to host functions only)
- No network access (except via `env.fetch`)
- Single-threaded execution per request
- Memory is **not shared** between requests (fresh instance per request)

## Best Practices

1. **Keep modules small**: < 1MB compiled size
2. **Minimize allocations**: Use static data when possible
3. **Handle errors gracefully**: Check `fetch` return values
4. **Log sparingly**: Logging has overhead
5. **Use pipelines**: Separate concerns (auth, logic, rate-limiting)

## Testing Your Module

```bash
# Compile WAT to Wasm
wat2wasm my_module.wat

# Run with Vortex
vortex-core my_module.wasm

# Test with curl
curl localhost:8080
```
