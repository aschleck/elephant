use anyhow::{anyhow, Result};
use cocoa::base::nil;
use cocoa::foundation::{NSArray, NSAutoreleasePool, NSData, NSString};
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::{CFRelease, FromVoid, ToVoid};
use core_foundation::boolean::CFBooleanRef;
use core_foundation::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation::number::{kCFNumberIntType, CFBooleanGetValue, CFNumberGetValue, CFNumberRef};
use core_foundation::string::CFString;
use core_graphics::image::CGImage;
use foreign_types_shared::ForeignType;
use metrohash::MetroHash64;
use std::ffi::CStr;
use std::hash::{Hash, Hasher};
use std::os::raw::c_void;

use core_graphics::base::{
    kCGBitmapByteOrder32Little, kCGImageAlphaLast, kCGImageAlphaPremultipliedLast,
};
use core_graphics::context::{CGContext, CGInterpolationQuality};
use core_graphics::display::{
    kCGNullWindowID, kCGWindowImageDefault, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionIncludingWindow, kCGWindowListOptionOnScreenOnly, CGRectNull,
};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::window::{
    create_image, kCGWindowIsOnscreen, kCGWindowLayer, kCGWindowName, kCGWindowNumber,
    kCGWindowSharingNone, kCGWindowSharingState, CGWindowListCopyWindowInfo,
};

use crate::objc_ffi::{
    NSBitmapImageFileType, NSBitmapImageRep, VNImageRequestHandler, VNRecognizeTextRequest,
    VNRecognizedText, VNRecognizedTextObservation,
};
use crate::types::Window;

struct WindowHandle {
    id: u32,
    title: String,
}

pub fn get_windows() -> Result<Vec<Window>> {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        let window_infos = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );
        let mut windows = Vec::new();
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

            let raw_layer = CFDictionaryGetValue(info, kCGWindowLayer.to_void());
            let mut layer: u32 = 0;
            CFNumberGetValue(
                raw_layer as CFNumberRef,
                kCFNumberIntType,
                &mut layer as *mut _ as *mut c_void,
            );
            if layer != 0 {
                continue;
            }

            let raw_id = CFDictionaryGetValue(info, kCGWindowNumber.to_void());
            let mut id: u32 = 0;
            CFNumberGetValue(
                raw_id as CFNumberRef,
                kCFNumberIntType,
                &mut id as *mut _ as *mut c_void,
            );

            let raw_title = CFDictionaryGetValue(info, kCGWindowName.to_void());
            if raw_title == std::ptr::null() {
                continue;
            }
            let title = CFString::from_void(raw_title).to_string();
            // There's probably a better way to avoid raised search boxes and what not, alas I
            // don't know it.
            // Maybe some info here
            // https://stackoverflow.com/q/5286274
            if title == "" {
                continue;
            }

            windows.push(WindowHandle {
                id: id,
                title: title,
            });
        }
        CFRelease(window_infos as *const c_void);

        let mut screenshots = Vec::new();
        for i in 0..windows.len() {
            let window = &windows[i];
            let z = windows.len() - 1 - i;
            let image = create_image(
                CGRectNull,
                kCGWindowListOptionIncludingWindow,
                window.id,
                kCGWindowImageDefault,
            )
            .ok_or_else(|| anyhow!("Unable to take screenshot"))?;
            //let small = resize_cgimage(&image, image.width() / 8, image.height() / 8)?;
            let jpeg = cgimage_to_jpeg(image.clone())?;
            let mut hasher = MetroHash64::new();
            jpeg.hash(&mut hasher);
            let hash = hasher.finish();
            screenshots.push(Window {
                id: window.id,
                title: window.title.clone(),
                jpeg: jpeg,
                //jpeg_small: cgimage_to_jpeg(small.clone())?,
                jpeg_metrohash: hash,
                z: z,
            });
        }

        pool.drain();
        Ok(screenshots)
    }
}

unsafe fn cgimage_to_jpeg(image: CGImage) -> Result<Vec<u8>> {
    let raw = NSBitmapImageRep::alloc(nil)
        .initWithCGImage_(image.as_ptr())
        .autorelease()
        .representationUsingType_(NSBitmapImageFileType::NSBitmapImageFileTypeJPEG);
    let slice = std::slice::from_raw_parts(raw.bytes() as *const u8, raw.length().try_into()?);
    Ok(Vec::from(slice))
}

//unsafe fn ocr_cgimage(image: &CGImage) -> Result<()> {
//    let handler =
//        VNImageRequestHandler::alloc(nil)
//        .initWithCGImage(image.as_ptr())
//        .autorelease();
//    let request = VNRecognizeTextRequest::alloc(nil);
//    VNRecognizeTextRequest::init(request).autorelease();
//    handler.performRequests(&[request], None);
//    let results = request.results();
//    for i in 0..results.count() {
//        let candidates = results.objectAtIndex(i).topCandidates(1);
//        for j in 0..candidates.count() {
//            let raw_string = candidates.objectAtIndex(j).string();
//            println!("{}", CStr::from_ptr(raw_string.UTF8String()).to_str()?);
//        }
//    }
//
//    Ok(())
//}

fn resize_cgimage(image: &CGImage, width: usize, height: usize) -> Result<CGImage> {
    let context = CGContext::create_bitmap_context(
        None,
        width,
        height,
        image.bits_per_component(),
        image.bytes_per_row(),
        &image.color_space(),
        kCGImageAlphaPremultipliedLast,
    );
    context.set_interpolation_quality(CGInterpolationQuality::CGInterpolationQualityHigh);
    context.draw_image(
        CGRect {
            origin: CGPoint { x: 0.0, y: 0.0 },
            size: CGSize {
                width: width as f64,
                height: height as f64,
            },
        },
        &image,
    );
    context
        .create_image()
        .ok_or_else(|| anyhow!("Couldn't make resized image"))
}

//fn cgimage_to_pixels(cg: CGImage) -> Result<Vec<u8>> {
//    if cg.bits_per_pixel() != 32 {
//        return Err(anyhow!("Expected screenshot to be rgba32"));
//    }
//
//    let data = cg.data();
//    let raw = data.bytes();
//    let width = cg.width();
//    let height = cg.height();
//    let mut copy = Vec::with_capacity(width * height * 4);
//    for row in raw.chunks_exact(cg.bytes_per_row()) {
//        copy.extend_from_slice(&row[..width * 4]);
//    }
//    for bgra in copy.chunks_exact_mut(4) {
//        bgra.swap(0, 2);
//    }
//    Ok(copy)
//}
