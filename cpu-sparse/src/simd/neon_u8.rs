// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SIMD speedups for Neon with u8 format

use core::arch::aarch64::*;

use crate::{
    fine::Fine,
    wide_tile::{STRIP_HEIGHT, WIDE_TILE_WIDTH},
};

unsafe fn div255q(x: uint16x8_t) -> uint8x8_t {
    vrshrn_n_u16(vsraq_n_u16(x, x, 8), 8)
}

fn color_to_u32(color: [f32; 4]) -> u32 {
    let bytes = color.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8);
    u32::from_le_bytes(bytes)
}

impl<'a> Fine<'a> {
    pub unsafe fn clear_simd_u8(&mut self, color: [f32; 4]) {
        self.fill_simd_u8(0, WIDE_TILE_WIDTH, color);
    }

    pub unsafe fn fill_simd_u8(&mut self, x: usize, width: usize, color: [f32; 4]) {
        let v_color = vdupq_n_u32(color_to_u32(color));
        let v_color2 = uint32x4x2_t(v_color, v_color);
        for i in 0..width / 2 {
            vst1q_u32_x2(self.scratch.as_mut_ptr().add(x * 4 + i * 8) as *mut u32, v_color2);
        }
        if width % 2 != 0 {
            vst1q_u32(self.scratch.as_mut_ptr().add((x + width - 1) * 4) as *mut u32, v_color);
        }
    }

    pub unsafe fn pack_simd_u8(&mut self, x: usize, y: usize) {
        let base_ix = y * STRIP_HEIGHT * self.width + x * WIDE_TILE_WIDTH;
        let out_ptr = self.out_buf.as_mut_ptr() as *mut u32;
        for i in (0..WIDE_TILE_WIDTH).step_by(4) {
            let chunk_ix = base_ix + i;
            let v = vld4q_u32((self.scratch.as_ptr() as *const u32).add(i * 4));
            vst1q_u32(out_ptr.add(chunk_ix), v.0);
            vst1q_u32(out_ptr.add(chunk_ix + self.width), v.1);
            vst1q_u32(out_ptr.add(chunk_ix + self.width * 2), v.2);
            vst1q_u32(out_ptr.add(chunk_ix + self.width * 3), v.3);
        }
    }

    #[inline(never)]
    pub unsafe fn strip_simd_u8(&mut self, x: usize, width: usize, alphas: &[u32], color: [f32; 4]) {
        let permlow = vcreate_u8(0x01010101_00000000);
        let permhigh = vcreate_u8(0x03030303_02020202);
        let color_splat = vreinterpret_u8_u32(vdup_n_u32(color_to_u32(color)));
        let alpha_splat = vdup_lane_u8(color_splat, 3);

        for i in 0..width {
            let ix = (x + i) * 4;
            // Maybe this should be ldr to an s register as in the f16 case
            let a = *alphas.get_unchecked(i);
            let a1 = vreinterpret_u8_u32(vdup_n_u32(a));
            let alphas = vmull_u8(alpha_splat, a1);
            let one_minus_alphas = vmvn_u8(div255q(alphas));
            let v = vld1_u8_x2(self.scratch.as_ptr().add(ix) as *const u8);
            let oml_low = vtbl1_u8(one_minus_alphas, permlow);
            let x0 = vmull_u8(v.0, oml_low);
            let y0 = vmlal_u8(x0, vtbl1_u8(a1, permlow), color_splat);
            let oml_high = vtbl1_u8(one_minus_alphas, permhigh);
            let x1 = vmull_u8(v.1, oml_high);
            let y1 = vmlal_u8(x1, vtbl1_u8(a1, permhigh), color_splat);
            let y = uint8x8x2_t(div255q(y0), div255q(y1));
            vst1_u8_x2(self.scratch.as_mut_ptr().add(ix) as *mut u8, y);
        }
    }
}
