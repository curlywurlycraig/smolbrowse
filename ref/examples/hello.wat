(module
  (import "smolbrowse" "pixel" (func $pixel (param i32 i32 i32)))
  (import "smolbrowse" "text" (func $text (param i32 i32 i32 i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "smolbrowse\nA small corner of the internet.\n\nPixels, words, and possibility.")
  (func (export "render")
    (local $x i32) (local $y i32)
    ;; Paint a dark blue canvas with a mountain silhouette.
    (loop $rows
      (local.set $x (i32.const 0))
      (loop $columns
        (call $pixel (local.get $x) (local.get $y)
          (if (result i32)
            (i32.gt_s (local.get $y)
              (i32.add (i32.const 140)
                (i32.div_s
                  (if (result i32) (i32.lt_s (local.get $x) (i32.const 160))
                    (then (i32.sub (i32.const 160) (local.get $x)))
                    (else (i32.sub (local.get $x) (i32.const 160))))
                  (i32.const 2))))
            (then (i32.const 0x526f83))
            (else (i32.const 0x172438))))
        (local.set $x (i32.add (local.get $x) (i32.const 1)))
        (br_if $columns (i32.lt_s (local.get $x) (i32.const 320))))
      (local.set $y (i32.add (local.get $y) (i32.const 1)))
      (br_if $rows (i32.lt_s (local.get $y) (i32.const 240))))
    (call $text (i32.const 16) (i32.const 24) (i32.const 0) (i32.const 75) (i32.const 0xf5dbb4))))
