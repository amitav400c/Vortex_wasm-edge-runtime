(module
  (func (export "handle_request") (result i32)
    ;; Return pointer to "403"
    i32.const 100
  )
  (memory (export "memory") 1)
  (data (i32.const 100) "403\00")
)
