(module
  (import "env" "log" (func $log (param i32 i32)))
  (import "env" "fetch" (func $fetch (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  
  (data (i32.const 0) "Processing request...")
  (data (i32.const 100) "https://api.example.com/data")
  (data (i32.const 200) "{\"status\":\"ok\",\"data\":\"fetched\"}")
  
  (func (export "handle_request") (result i32)
    ;; 1. Log
    (call $log (i32.const 0) (i32.const 21))
    
    ;; 2. Fetch
    (call $fetch (i32.const 100) (i32.const 28))
    drop ;; Ignore result
    
    ;; 3. Return JSON
    i32.const 200
  )
)
