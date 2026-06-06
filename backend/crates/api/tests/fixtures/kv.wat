;; Guest exercising HOST FUNCTIONS: `set` forwards its input JSON to the
;; `plugin_kv_set` host fn, `get` to `plugin_kv_get`, and each outputs the
;; host fn's result verbatim. Both are imported from "extism:host/user" — so
;; instantiating this module under a manifest that does NOT grant them fails,
;; which is exactly the structural capability containment under test.
(module
  (import "extism:host/env" "input_length" (func $input_length (result i64)))
  (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
  (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
  (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))
  (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))
  (import "extism:host/env" "length" (func $length (param i64) (result i64)))
  (import "extism:host/user" "plugin_kv_set" (func $kv_set (param i64) (result i64)))
  (import "extism:host/user" "plugin_kv_get" (func $kv_get (param i64) (result i64)))

  (func $copy_input (result i64)
    (local $len i64) (local $buf i64) (local $i i64)
    (local.set $len (call $input_length))
    (local.set $buf (call $alloc (local.get $len)))
    (block $done
      (loop $copy
        (br_if $done (i64.ge_u (local.get $i) (local.get $len)))
        (call $store_u8
          (i64.add (local.get $buf) (local.get $i))
          (call $input_load_u8 (local.get $i)))
        (local.set $i (i64.add (local.get $i) (i64.const 1)))
        (br $copy)))
    (local.get $buf))

  (func $forward (param $host_result i64)
    (call $output_set (local.get $host_result) (call $length (local.get $host_result))))

  (func (export "set") (result i32)
    (call $forward (call $kv_set (call $copy_input)))
    (i32.const 0))

  (func (export "get") (result i32)
    (call $forward (call $kv_get (call $copy_input)))
    (i32.const 0))
)
