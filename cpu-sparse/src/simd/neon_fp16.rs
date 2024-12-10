// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SIMD speedups for Neon with the FP16 extension

use core::arch::aarch64::*;
use std::arch::asm;

use crate::{
    fine::Fine,
    strip::{Strip, Tile},
    tiling::Vec2,
    wide_tile::{STRIP_HEIGHT, WIDE_TILE_WIDTH},
};

impl<'a> Fine<'a> {
    #[inline(never)]
    pub unsafe fn clear_simd_f16(&mut self, color: [f32; 4]) {
        let v_color = vld1q_f32(color.as_ptr());
        asm!(
            "fcvtn v0.4h, v0.4s",
            "trn1.2d v0, v0, v0",
            "mov.16b v1, v0",
            "mov.16b v2, v0",
            "mov.16b v3, v0",
            "2:",
            "st1.4s {{ v0, v1, v2, v3 }}, [x9], #64",
            "subs x2, x2, #2",
            "b.ne 2b",
            inout("x9") &mut self.scratch => _,
            in("q0") v_color,
            inout("x2") WIDE_TILE_WIDTH => _,
            options(nostack),
        )
    }

    // only opaque fills
    #[inline(never)]
    pub unsafe fn fill_simd_f16(&mut self, x: usize, width: usize, color: [f32; 4]) {
        let v_color = vld1q_f32(color.as_ptr());
        asm!(
            "add x9, x9, x1, lsl #5",
            "fcvtn v0.4h, v0.4s",
            "trn1.2d v0, v0, v0",
            "mov.16b v1, v0",
            "tbz w2, #0, 2f",
            "st1.4s {{ v0, v1 }}, [x9], #32",
            "subs x2, x2, #1",
            "b.eq 4f",
            "2:",
            "mov.16b v2, v0",
            "mov.16b v3, v0",
            "3:",
            "st1.4s {{ v0, v1, v2, v3 }}, [x9], #64",
            "subs x2, x2, #2",
            "b.ne 3b",
            "4:",
            in("x1") x,
            inout("x2") width => _,
            inout("x9") &mut self.scratch => _,
            in("q0") v_color,
            options(nostack),
        )
    }

    // Note: saturation isn't needed, it's taken care of by fcvtnu
    pub unsafe fn pack_simd_f16(&mut self, x: usize, y: usize) {
        let base_ix = (y * STRIP_HEIGHT * self.width + x * WIDE_TILE_WIDTH) * 4;
        let out_base = self.out_buf.as_mut_ptr().add(base_ix);
        asm!(
            "add x10, x8, x9, lsl #3",
            "lsl x9, x9, #2",
            "2:",
            "ld4.2d {{ v4, v5, v6, v7 }}, [x4], #64",
            "ld4.2d {{ v8, v9, v10, v11 }}, [x4], #64",
            "fmax.8h v4, v4, v0",
            "fmin.8h v4, v4, v1",
            "fmul.8h v4, v4, v2",
            "fcvtnu.8h v4, v4",
            "fmax.8h v5, v5, v0",
            "fmin.8h v5, v5, v1",
            "fmul.8h v5, v5, v2",
            "fcvtnu.8h v5, v5",
            "fmax.8h v6, v6, v0",
            "fmin.8h v6, v6, v1",
            "fmul.8h v6, v6, v2",
            "fcvtnu.8h v6, v6",
            "fmax.8h v7, v7, v0",
            "fmin.8h v7, v7, v1",
            "fmul.8h v7, v7, v2",
            "fcvtnu.8h v7, v7",
            "fmax.8h v8, v8, v0",
            "fmin.8h v8, v8, v1",
            "fmul.8h v8, v8, v2",
            "fcvtnu.8h v8, v8",
            "fmax.8h v9, v9, v0",
            "fmin.8h v9, v9, v1",
            "fmul.8h v9, v9, v2",
            "fcvtnu.8h v9, v9",
            "fmax.8h v10, v10, v0",
            "fmin.8h v10, v10, v1",
            "fmul.8h v10, v10, v2",
            "fcvtnu.8h v10, v10",
            "fmax.8h v11, v11, v0",
            "fmin.8h v11, v11, v1",
            "fmul.8h v11, v11, v2",
            "fcvtnu.8h v11, v11",
            "uzp1.16b v4, v4, v8",
            "uzp1.16b v5, v5, v9",
            "uzp1.16b v6, v6, v10",
            "uzp1.16b v7, v7, v11",
            "str q5, [x8, x9]",
            "st1.4s {{ v4 }}, [x8], #16",
            "str q7, [x10, x9]",
            "st1.4s {{ v6 }}, [x10], #16",
            "subs x3, x3, #4",
            "b.ne 2b",
            inout("x3") WIDE_TILE_WIDTH => _,
            inout("x4") &self.scratch => _,
            in("q2") vdupq_n_u16(0x5bf8), // 255.0f16
            inout("x8") out_base => _,
            inout("x9") self.width => _,
            options(nostack),
        )
    }

    #[inline(never)]
    pub unsafe fn strip_simd_f16(&mut self, x: usize, width: usize, alphas: &[u32], color: [f32; 4]) {
        let v_color = vmulq_f32(vld1q_f32(color.as_ptr()), vdupq_n_f32(1.0 / 255.0));
        const PERM: [u8; 16] = [0, 16, 0, 16, 1, 16, 1, 16,
        2, 16, 2, 16, 3, 16, 3, 16];
        asm!(
            "fcvtn v0.4h, v0.4s",
            "trn1.2d v0, v0, v0",
            "ld1.16b {{ v6 }}, [x10]",
            "fmov.8h v2, #1.0",
            "2:",
            "ldr s3, [x8], #4",
            "ld1.8h {{ v4, v5 }}, [x9]",
            "tbl.16b v8, {{ v3 }}, v6",
            "ucvtf.8h v8, v8",
            "mov.16b v16, v2",
            "fmls.8h v16, v8, v0[3]",
            "zip1.8h v9, v8, v8",
            "fmul.8h v17, v0, v9",
            "zip1.8h v9, v16, v16",
            "fmla.8h v17, v4, v9",
            "zip2.8h v9, v8, v8",
            "fmul.8h v18, v0, v9",
            "zip2.8h v9, v16, v16",
            "fmla.8h v18, v5, v9",
            "st1.16b {{ v17, v18 }}, [x9], #32",
            "subs x2, x2, #1",
            "b.ne 2b",
            in("q0") v_color,
            inout("x8") alphas.as_ptr() => _,
            inout("x9") self.scratch.as_mut_ptr().add(x * 8) => _,
            in("x10") PERM.as_ptr(),
            inout("x2") width => _,
            options(nostack)
        )
    }
}
