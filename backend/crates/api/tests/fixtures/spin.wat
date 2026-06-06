;; Hostile guest: `spin` never returns. Proves the manifest `timeout_ms`
;; epoch-interruption kill actually fires (the invocation must come back as a
;; resource-limit error, not hang the server).
(module
  (func (export "spin") (result i32)
    (loop $forever (br $forever))
    (i32.const 0))
)
