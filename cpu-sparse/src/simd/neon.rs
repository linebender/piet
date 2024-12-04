// Copyright 2024 the Piet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SIMD speedups for Neon

use core::arch::aarch64::*;

use crate::{
    fine::Fine,
    wide_tile::{STRIP_HEIGHT, WIDE_TILE_WIDTH},
};

impl<'a> Fine<'a> {
    pub unsafe fn clear_simd(&mut self, color: [f32; 4]) {
        let v_color = vld1q_f32(color.as_ptr());
        let v_color_4 = float32x4x4_t(v_color, v_color, v_color, v_color);
        for i in 0..WIDE_TILE_WIDTH {
            vst1q_f32_x4(self.scratch.as_mut_ptr().add(i * 16), v_color_4);
        }
    }

    pub fn pack_simd(&mut self, x: usize, y: usize) {
        unsafe fn cvt(v: float32x4_t) -> uint8x16_t {
            let clamped = vminq_f32(vmaxq_f32(v, vdupq_n_f32(0.0)), vdupq_n_f32(1.0));
            let scaled = vmulq_f32(clamped, vdupq_n_f32(255.0));
            vreinterpretq_u8_u32(vcvtnq_u32_f32(scaled))
        }

        unsafe fn cvt2(v0: float32x4_t, v1: float32x4_t) -> uint8x16_t {
            vuzp1q_u8(cvt(v0), cvt(v1))
        }

        unsafe {
            let base_ix = (y * STRIP_HEIGHT * self.width + x * WIDE_TILE_WIDTH) * 4;
            for i in (0..WIDE_TILE_WIDTH).step_by(4) {
                let chunk_ix = base_ix + i * 4;
                let v0 = vld1q_f32_x4(self.scratch.as_ptr().add(i * 16));
                let v1 = vld1q_f32_x4(self.scratch.as_ptr().add((i + 1) * 16));
                let x0 = cvt2(v0.0, v1.0);
                let x1 = cvt2(v0.1, v1.1);
                let x2 = cvt2(v0.2, v1.2);
                let x3 = cvt2(v0.3, v1.3);
                let v2 = vld1q_f32_x4(self.scratch.as_ptr().add((i + 2) * 16));
                let v3 = vld1q_f32_x4(self.scratch.as_ptr().add((i + 3) * 16));
                let x4 = cvt2(v2.0, v3.0);
                let y0 = vuzp1q_u8(x0, x4);
                vst1q_u8(self.out_buf.as_mut_ptr().add(chunk_ix), y0);
                let x5 = cvt2(v2.1, v3.1);
                let y1 = vuzp1q_u8(x1, x5);
                vst1q_u8(self.out_buf.as_mut_ptr().add(chunk_ix + self.width * 4), y1);
                let x6 = cvt2(v2.2, v3.2);
                let y2 = vuzp1q_u8(x2, x6);
                vst1q_u8(self.out_buf.as_mut_ptr().add(chunk_ix + self.width * 8), y2);
                let x7 = cvt2(v2.3, v3.3);
                let y3 = vuzp1q_u8(x3, x7);
                vst1q_u8(
                    self.out_buf.as_mut_ptr().add(chunk_ix + self.width * 12),
                    y3,
                );
            }
        }
    }

    pub unsafe fn fill_simd(&mut self, x: usize, width: usize, color: [f32; 4]) {
        let v_color = vld1q_f32(color.as_ptr());
        let alpha = color[3];
        if alpha == 1.0 {
            let v_color_4 = float32x4x4_t(v_color, v_color, v_color, v_color);
            for i in x..x + width {
                vst1q_f32_x4(self.scratch.as_mut_ptr().add(i * 16), v_color_4);
            }
        } else {
            let one_minus_alpha = vdupq_n_f32(1.0 - alpha);
            for i in x..x + width {
                let ix = (x + i) * 16;
                let mut v = vld1q_f32_x4(self.scratch.as_ptr().add(ix));
                v.0 = vfmaq_f32(v_color, v.0, one_minus_alpha);
                v.1 = vfmaq_f32(v_color, v.1, one_minus_alpha);
                v.2 = vfmaq_f32(v_color, v.2, one_minus_alpha);
                v.3 = vfmaq_f32(v_color, v.3, one_minus_alpha);
                vst1q_f32_x4(self.scratch.as_mut_ptr().add(ix), v);
            }
        }
    }

    pub unsafe fn strip_simd(&mut self, x: usize, width: usize, alphas: &[u32], color: [f32; 4]) {
        debug_assert!(alphas.len() >= width);
        let v_color = vmulq_f32(vld1q_f32(color.as_ptr()), vdupq_n_f32(1.0 / 255.0));
        for i in 0..width {
            let a = *alphas.get_unchecked(i);
            // all this zipping compiles to tbl, we should probably just write that
            let a1 = vreinterpret_u8_u32(vdup_n_u32(a));
            let a2 = vreinterpret_u16_u8(vzip1_u8(a1, vdup_n_u8(0)));
            let a3 = vcombine_u16(a2, vdup_n_u16(0));
            let a4 = vreinterpretq_u32_u16(vzip1q_u16(a3, vdupq_n_u16(0)));
            let alpha = vcvtq_f32_u32(a4);
            let ix = (x + i) * 16;
            let mut v = vld1q_f32_x4(self.scratch.as_ptr().add(ix));
            let one_minus_alpha = vfmsq_laneq_f32(vdupq_n_f32(1.0), alpha, v_color, 3);
            v.0 = vfmaq_laneq_f32(vmulq_laneq_f32(v_color, alpha, 0), v.0, one_minus_alpha, 0);
            v.1 = vfmaq_laneq_f32(vmulq_laneq_f32(v_color, alpha, 1), v.1, one_minus_alpha, 1);
            v.2 = vfmaq_laneq_f32(vmulq_laneq_f32(v_color, alpha, 2), v.2, one_minus_alpha, 2);
            v.3 = vfmaq_laneq_f32(vmulq_laneq_f32(v_color, alpha, 3), v.3, one_minus_alpha, 3);
            vst1q_f32_x4(self.scratch.as_mut_ptr().add(ix), v);
        }
    }
}
