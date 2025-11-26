(module
  ;; Simplest possible Vortex module
  ;; Returns "Hello, World!" as HTTP response
  
  (memory (export "memory") 1)
  
  (func (export "handle_request") (result i32)
    ;; Return offset where response is stored
    i32.const 0
  )
  
  ;; HTTP response stored at offset 0
  (data (i32.const 0) "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, World!")
)
