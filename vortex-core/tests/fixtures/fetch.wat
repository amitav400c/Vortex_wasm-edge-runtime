(module
  (import "env" "fetch" (func $fetch (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "https://example.com")
  
  (func (export "handle_request") (result i32)
    ;; Call fetch with url at offset 0, length 19
    (call $fetch (i32.const 0) (i32.const 19))
    ;; Ignore result for now, just return 200 status
    drop
    i32.const 100
  )
  (data (i32.const 100) "Fetched\00")
)
