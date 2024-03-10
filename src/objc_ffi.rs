#![allow(non_snake_case)]

use cocoa::base::{id, nil, BOOL};
use cocoa::foundation::{NSDictionary, NSRect};
use core_graphics::sys::CGImageRef;
use std::os::raw::c_void;

#[allow(dead_code)]
pub enum NSBitmapImageFileType {
    NSBitmapImageFileTypeTIFF = 0,
    NSBitmapImageFileTypeBMP = 1,
    NSBitmapImageFileTypeGIF = 2,
    NSBitmapImageFileTypeJPEG = 3,
    NSBitmapImageFileTypePNG = 4,
    NSBitmapImageFileTypeJPEG2000 = 5,
}

pub trait NSBitmapImageRep: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSBitmapImageRep), alloc]
    }

    unsafe fn initWithCGImage_(self, cgImage: CGImageRef) -> id;
    unsafe fn representationUsingType_(self, storageType: NSBitmapImageFileType) -> id /* (NSData) */;
}

impl NSBitmapImageRep for id {
    unsafe fn initWithCGImage_(self, cgImage: CGImageRef) -> id {
        msg_send![self, initWithCGImage: cgImage as *const c_void]
    }

    unsafe fn representationUsingType_(self, storageType: NSBitmapImageFileType) -> id /* (NSData) */
    {
        msg_send![
            self,
            representationUsingType: storageType
            properties: NSDictionary::dictionary(nil)
        ]
    }
}

pub trait NSTextView: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSTextView), alloc]
    }

    unsafe fn initWithFrame_(self, frameRect: NSRect) -> id;
    unsafe fn setEditable_(self, editable: BOOL);
    unsafe fn setString_(self, string: id /* NSString */);
}

impl NSTextView for id {
    unsafe fn initWithFrame_(self, frameRect: NSRect) -> id {
        msg_send![self, initWithFrame: frameRect]
    }

    unsafe fn setEditable_(self, editable: BOOL) {
        msg_send![self, setEditable: editable]
    }

    unsafe fn setString_(self, string: id /* NSString */) {
        msg_send![self, setString: string]
    }
}
