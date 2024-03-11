use anyhow::{anyhow, Result};
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSData};
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::{CFRelease, ToVoid};
use core_foundation::boolean::CFBooleanRef;
use core_foundation::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation::number::{kCFNumberIntType, CFBooleanGetValue, CFNumberGetValue, CFNumberRef};
use core_graphics::image::CGImage;
use foreign_types_shared::ForeignType;
use image::RgbaImage;
use std::os::raw::c_void;

use core_graphics::display::{
    kCGNullWindowID, kCGWindowImageDefault, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionIncludingWindow, kCGWindowListOptionOnScreenOnly, CGRectNull,
};
use core_graphics::window::{
    create_image, kCGWindowIsOnscreen, kCGWindowNumber, kCGWindowSharingNone,
    kCGWindowSharingState, CGWindowListCopyWindowInfo,
};

use crate::objc_ffi::{NSBitmapImageRep, NSBitmapImageFileType};

pub struct Screenshot {
    pub image: RgbaImage,
    pub jpeg: Vec<u8>,
}

pub fn take_screenshots() -> Result<Vec<Screenshot>> {
    let windows = unsafe {
        let window_infos = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );
        let mut windows: Vec<u32> = Vec::new();
        for i in 0..CFArrayGetCount(window_infos) {
            let info = CFArrayGetValueAtIndex(window_infos, i) as CFDictionaryRef;
            if info.is_null() {
                continue;
            }

            let raw_sharing_state = CFDictionaryGetValue(info, kCGWindowSharingState.to_void());
            let mut sharing_state: u32 = 0;
            CFNumberGetValue(
                raw_sharing_state as CFNumberRef,
                kCFNumberIntType,
                &mut sharing_state as *mut _ as *mut c_void,
            );
            if sharing_state == kCGWindowSharingNone {
                continue;
            }

            let raw_onscreen = CFDictionaryGetValue(info, kCGWindowIsOnscreen.to_void());
            if !CFBooleanGetValue(raw_onscreen as CFBooleanRef) {
                continue;
            }

            let raw_id = CFDictionaryGetValue(info, kCGWindowNumber.to_void());
            let mut id: u32 = 0;
            CFNumberGetValue(
                raw_id as CFNumberRef,
                kCFNumberIntType,
                &mut id as *mut _ as *mut c_void,
            );
            windows.push(id);
        }
        CFRelease(window_infos as *const c_void);
        windows
    };

    let mut screenshots = Vec::new();
    for window in windows {
        unsafe {
            let image = create_image(
                CGRectNull,
                kCGWindowListOptionIncludingWindow,
                window,
                kCGWindowImageDefault,
            ).ok_or_else(|| anyhow!("Null image"))?;
            screenshots.push(Screenshot {
                image: cgimage_to_image(image.clone())?,
                jpeg: cgimage_to_jpeg(image.clone())?,
            });
        };
    }

    Ok(screenshots)
}

unsafe fn cgimage_to_image(image: CGImage) -> Result<RgbaImage> {
    let bytes = image.data();
    let raw = bytes.bytes();
    let width = image.width();
    let height = image.height();
    let mut copy = Vec::with_capacity(width * height * 4);
    for row in raw.chunks_exact(image.bytes_per_row()) {
        copy.extend_from_slice(&row[..width * 4]);
    }
    for bgra in copy.chunks_exact_mut(4) {
        bgra.swap(0, 2);
    }
    let image = RgbaImage::from_raw(width.try_into()?, height.try_into()?, copy);
    image.ok_or_else(|| anyhow!("Unable to convert image"))
}

unsafe fn cgimage_to_jpeg(image: CGImage) -> Result<Vec<u8>> {
    let raw = NSBitmapImageRep::alloc(nil)
        .initWithCGImage_(image.as_ptr())
        .autorelease()
        .representationUsingType_(NSBitmapImageFileType::NSBitmapImageFileTypeJPEG);
    let slice = std::slice::from_raw_parts(raw.bytes() as *const u8, raw.length().try_into()?);
    Ok(Vec::from(slice))
}