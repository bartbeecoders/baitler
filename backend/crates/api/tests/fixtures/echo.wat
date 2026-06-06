;; Minimal Extism guest, hand-written (no PDK/toolchain): `echo` copies the
;; call input verbatim to the output. Exercises the kernel ABI end to end:
;; input_length/input_load_u8 → alloc/store_u8 → output_set.
(module
  (import "extism:host/env" "input_length" (func $input_length (result i64)))
  (import "extism:host/env" "input_load_u8" (func $input_load_u8 (param i64) (result i32)))
  (import "extism:host/env" "alloc" (func $alloc (param i64) (result i64)))
  (import "extism:host/env" "store_u8" (func $store_u8 (param i64 i32)))
  (import "extism:host/env" "output_set" (func $output_set (param i64 i64)))

  ;; Copy the call input into a fresh kernel-memory block; returns its offset.
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

  (func (export "echo") (result i32)
    (call $output_set (call $copy_input) (call $input_length))
    (i32.const 0))
)
