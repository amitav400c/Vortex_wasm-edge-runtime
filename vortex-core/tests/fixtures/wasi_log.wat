(module
  (import "env" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Hello from WASI Log!")
  
  (func (export "handle_request") (result i32)
    (call $log (i32.const 0) (i32.const 20))
    i32.const 100
  )
  (data (i32.const 100) "Logged\00")
)
