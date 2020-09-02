//! Iteration over glyph runs.

#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use winapi::shared::guiddef::{IsEqualGUID, REFIID};
use winapi::shared::minwindef::{BOOL, FALSE, FLOAT, ULONG};
use winapi::shared::ntdef::HRESULT;
use winapi::shared::winerror::{E_NOINTERFACE, S_OK};
use winapi::um::dcommon::DWRITE_MEASURING_MODE;
use winapi::um::dwrite::{
    IDWriteInlineObject, IDWritePixelSnapping, IDWritePixelSnappingVtbl, IDWriteTextRenderer,
    IDWriteTextRendererVtbl, DWRITE_GLYPH_RUN, DWRITE_GLYPH_RUN_DESCRIPTION, DWRITE_MATRIX,
    DWRITE_STRIKETHROUGH, DWRITE_UNDERLINE,
};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::Interface;

use wio::com::ComPtr;

/// A custom renderer which will be used as the callback for
/// internal iteration over glyph runs.
///
/// This implementation is very much by hand. When com-rs is
/// ready, we should switch to that.
#[repr(C)]
pub(crate) struct CustomRenderer {
    refcount: AtomicUsize,
}

#[repr(C)]
struct CustomRendererRepr(*const IDWriteTextRendererVtbl, CustomRenderer);

impl CustomRenderer {
    pub(crate) fn new() -> CustomRenderer {
        CustomRenderer {
            refcount: AtomicUsize::new(1),
        }
    }

    fn into_interface(self) -> *mut IDWriteTextRenderer {
        let com = Box::new(CustomRendererRepr(&CUSTOM_RENDERER_VTBL, self));
        Box::into_raw(com) as *mut _
    }

    pub(crate) fn into_comptr(self) -> ComPtr<IDWriteTextRenderer> {
        unsafe { ComPtr::from_raw(self.into_interface()) }
    }

    unsafe fn from_interface<'a>(thing: *mut IDWriteTextRenderer) -> &'a mut CustomRenderer {
        &mut (*(thing as *mut CustomRendererRepr)).1
    }

    unsafe fn destroy(thing: *mut IDWriteTextRendererVtbl) {
        Box::from_raw(thing as *mut CustomRendererRepr);
    }
}

static CUSTOM_RENDERER_VTBL: IDWriteTextRendererVtbl = IDWriteTextRendererVtbl {
    parent: IDWritePixelSnappingVtbl {
        parent: IUnknownVtbl {
            QueryInterface: CustomRenderer_QueryInterface,
            AddRef: CustomRenderer_AddRef,
            Release: CustomRenderer_Release,
        },
        IsPixelSnappingDisabled: CustomRenderer_IsPixelSnappingDisabled,
        GetCurrentTransform: CustomRenderer_GetCurrentTransform,
        GetPixelsPerDip: CustomRenderer_GetPixelsPerDip,
    },
    DrawGlyphRun: CustomRenderer_DrawGlyphRun,
    DrawInlineObject: CustomRenderer_DrawInlineObject,
    DrawStrikethrough: CustomRenderer_DrawStrikethrough,
    DrawUnderline: CustomRenderer_DrawUnderline,
};

unsafe extern "system" fn CustomRenderer_AddRef(unknown_this: *mut IUnknown) -> ULONG {
    let this = CustomRenderer::from_interface(unknown_this as *mut _);
    (this.refcount.fetch_add(1, Ordering::Relaxed) + 1) as ULONG
}

unsafe extern "system" fn CustomRenderer_Release(unknown_this: *mut IUnknown) -> ULONG {
    let count = {
        let this = CustomRenderer::from_interface(unknown_this as *mut _);
        this.refcount.fetch_sub(1, Ordering::Release) - 1
    };
    if count == 0 {
        // Atomic patterns adapted from Rust's Arc.
        std::sync::atomic::fence(Ordering::Acquire);
        CustomRenderer::destroy(unknown_this as *mut _);
    }
    count as ULONG
}

unsafe extern "system" fn CustomRenderer_QueryInterface(
    unknown_this: *mut IUnknown,
    riid: REFIID,
    ppv_object: *mut *mut c_void,
) -> HRESULT {
    if IsEqualGUID(&*riid, &IUnknown::uuidof())
        || IsEqualGUID(&*riid, &IDWritePixelSnapping::uuidof())
        || IsEqualGUID(&*riid, &IDWriteTextRenderer::uuidof())
    {
        CustomRenderer_AddRef(unknown_this);
        *ppv_object = unknown_this as *mut _;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn CustomRenderer_IsPixelSnappingDisabled(
    this: *mut IDWritePixelSnapping,
    client_drawing_context: *mut c_void,
    is_disabled: *mut BOOL,
) -> HRESULT {
    *is_disabled = FALSE;
    0
}

unsafe extern "system" fn CustomRenderer_GetCurrentTransform(
    this: *mut IDWritePixelSnapping,
    client_drawing_context: *mut c_void,
    transform: *mut DWRITE_MATRIX,
) -> HRESULT {
    *transform = DWRITE_MATRIX {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        dx: 0.0,
        dy: 0.0,
    };
    0
}

unsafe extern "system" fn CustomRenderer_GetPixelsPerDip(
    this: *mut IDWritePixelSnapping,
    client_drawing_context: *mut c_void,
    pixels_per_dip: *mut FLOAT,
) -> HRESULT {
    *pixels_per_dip = 1.0;
    0
}

unsafe extern "system" fn CustomRenderer_DrawGlyphRun(
    this: *mut IDWriteTextRenderer,
    client_drawing_context: *mut c_void,
    baseline_origin_x: FLOAT,
    baseline_origin_y: FLOAT,
    measuring_mode: DWRITE_MEASURING_MODE,
    glyph_run: *const DWRITE_GLYPH_RUN,
    glyph_run_description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
    client_drawing_effect: *mut IUnknown,
) -> HRESULT {
    let glyph_run = &*glyph_run;
    let descr = &*glyph_run_description;
    let len = glyph_run.glyphCount as usize;
    println!(
        "draw glyph run: size {} @{}",
        glyph_run.fontEmSize, descr.textPosition
    );
    let indices = std::slice::from_raw_parts(glyph_run.glyphIndices, len);
    let advances = std::slice::from_raw_parts(glyph_run.glyphAdvances, len);
    let offsets = std::slice::from_raw_parts(glyph_run.glyphOffsets, len);
    for i in 0..glyph_run.glyphCount {
        let offset = &offsets[i as usize];
        println!(
            "  glyph ix {} @ ({}, {}), -> {}",
            indices[i as usize], offset.advanceOffset, offset.ascenderOffset, advances[i as usize]
        );
    }
    let cluster_map = std::slice::from_raw_parts(descr.clusterMap, descr.stringLength as usize);
    for i in 0..descr.stringLength {
        println!("  cluster map {}", cluster_map[i as usize]);
    }
    0
}

unsafe extern "system" fn CustomRenderer_DrawInlineObject(
    this: *mut IDWriteTextRenderer,
    client_drawing_context: *mut c_void,
    origin_x: FLOAT,
    origin_y: FLOAT,
    inline_object: *mut IDWriteInlineObject,
    is_sideways: BOOL,
    is_right_to_left: BOOL,
    client_drawing_effect: *mut IUnknown,
) -> HRESULT {
    println!("draw inline object");
    0
}

unsafe extern "system" fn CustomRenderer_DrawStrikethrough(
    this: *mut IDWriteTextRenderer,
    client_drawing_context: *mut c_void,
    baseline_origin_x: FLOAT,
    baseline_origin_y: FLOAT,
    strikethrough: *const DWRITE_STRIKETHROUGH,
    client_drawing_effect: *mut IUnknown,
) -> HRESULT {
    println!("draw strikethrough");
    0
}

unsafe extern "system" fn CustomRenderer_DrawUnderline(
    this: *mut IDWriteTextRenderer,
    client_drawing_context: *mut c_void,
    baseline_origin_x: FLOAT,
    baseline_origin_y: FLOAT,
    underline: *const DWRITE_UNDERLINE,
    client_drawing_effect: *mut IUnknown,
) -> HRESULT {
    println!("draw underline");
    0
}
