(module
  (memory (export "memory") 1)
  
  ;; Mock Auth: Always returns 200 OK (pass)
  ;; In a real scenario, it would check headers.
  (func (export "handle_request") (result i32)
    ;; Return pointer to "200"
    i32.const 0
  )
  (data (i32.const 0) "200")
)
