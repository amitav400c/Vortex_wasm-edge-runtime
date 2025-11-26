(module
  ;; Authentication middleware example
  ;; Checks for "Authorization" header (simplified)
  ;; Returns 403 if missing, 200 if present
  
  (memory (export "memory") 1)
  
  (func (export "handle_request") (result i32)
    ;; In a real implementation, you would:
    ;; 1. Parse HTTP headers from request
    ;; 2. Check for Authorization header
    ;; 3. Validate token
    
    ;; For this example, we always pass (return 200)
    ;; To trigger auth failure, change to: i32.const 100
    i32.const 0
  )
  
  ;; Success response (pass to next module in pipeline)
  (data (i32.const 0) "HTTP/1.1 200 OK\r\nContent-Length: 11\r\n\r\nAuthorized!")
  
  ;; Failure response (stops pipeline)
  (data (i32.const 100) "403")
)
