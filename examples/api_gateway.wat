(module
  ;; API Gateway example with routing and external fetch
  
  (import "env" "log" (func $log (param i32 i32)))
  (import "env" "fetch" (func $fetch (param i32 i32) (result i32)))
  
  (memory (export "memory") 1)
  
  (func (export "handle_request") (result i32)
    ;; Log incoming request
    (call $log (i32.const 500) (i32.const 22))
    
    ;; In a real implementation, parse request path
    ;; For this example, always fetch from external API
    (call $fetch (i32.const 600) (i32.const 28))
    drop
    
    ;; Return aggregated response
    i32.const 0
  )
  
  ;; HTTP response
  (data (i32.const 0) "HTTP/1.1 200 OK\r\nContent-Length: 47\r\n\r\n{\"status\":\"ok\",\"message\":\"API Gateway Response\"}")
  
  ;; Log message
  (data (i32.const 500) "API Gateway: Processing")
  
  ;; External API URL
  (data (i32.const 600) "https://api.example.com/data")
)
