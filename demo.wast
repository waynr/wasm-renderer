(module
 (type $t0 (func))

 (memory $mem 1)

 (func $tick (type $t0)
    (memory.fill $mem
        (i32.const 0)
        (i32.const 0xcfdf)
        (i32.const 0x10000)
    )
 )

 (export "tick" (func $tick))
 (export "image_buffer" (memory $mem))
)
