#[macro_use]
extern crate cocoa;
#[macro_use]
extern crate objc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyAccessory, NSBackingStoreBuffered, NSImage, NSMenu, NSMenuItem,
    NSRunningApplication, NSSquareStatusItemLength, NSStatusBar, NSStatusItem, NSWindow,
    NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSData, NSPoint, NSRect, NSSize, NSString};
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::ToVoid;
use core_foundation::boolean::CFBooleanRef;
use core_foundation::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation::number::{kCFNumberIntType, CFBooleanGetValue, CFNumberGetValue, CFNumberRef};
use core_graphics::access::ScreenCaptureAccess;
use core_graphics::display::{
    kCGNullWindowID, kCGWindowImageDefault, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionIncludingWindow, kCGWindowListOptionOnScreenOnly, CGRectNull,
};

use core_graphics::window::{
    kCGWindowIsOnscreen, kCGWindowNumber, kCGWindowSharingNone, kCGWindowSharingState,
    CGWindowListCopyWindowInfo, CGWindowListCreateImage,
};

use objc::runtime::{Object, Sel};
use std::os::raw::c_void;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

mod objc_ffi;
use crate::objc_ffi::{NSBitmapImageFileType, NSBitmapImageRep, NSTextView};

struct State {
    window_count: usize,
    window_open: bool,
}

extern "C" fn should_close(_: &Object, _: Sel, _: id) -> bool {
    return false;
}

extern "C" fn close(this: &Object, _: Sel, _: id) {
    unsafe {
        // Am I bad a person for dropping the Arc? Yes I am.
        let state_ptr = *this.get_ivar::<*mut c_void>("state") as *mut Mutex<State>;
        let mut state = (*(state_ptr as *mut Mutex<State>)).lock().unwrap();
        state.window_open = false;
    }
}

extern "C" fn open(this: &Object, _: Sel, _: id) {
    unsafe {
        // Am I bad a person for dropping the Arc? Yes I am.
        let state_ptr = *this.get_ivar::<*mut c_void>("state") as *mut Mutex<State>;
        let mut state = (*(state_ptr as *mut Mutex<State>)).lock().unwrap();
        let window_delegate: id = *this.get_ivar("window_delegate");
        if !state.window_open {
            open_window(&mut state, window_delegate);
        }
    }
}

extern "C" fn will_terminate(_: &Object, _: Sel, _: id) {
    // Required? Idk
}

fn add_to_status_bar() {
    unsafe {
        let menu = NSMenu::new(nil).autorelease();
        let open = NSMenuItem::new(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Open"),
                sel!(open:),
                NSString::alloc(nil).init_str(""),
            )
            .autorelease();
        menu.addItem_(open);
        let quit = NSMenuItem::new(nil)
            .initWithTitle_action_keyEquivalent_(
                NSString::alloc(nil).init_str("Quit"),
                sel!(terminate:),
                NSString::alloc(nil).init_str(""),
            )
            .autorelease();
        menu.addItem_(quit);

        let icon = include_bytes!("icon.svg");
        let icon_data =
            NSData::dataWithBytes_length_(nil, icon.as_ptr() as *mut c_void, icon.len() as u64);
        let icon_image = NSImage::initWithData_(NSImage::alloc(nil), icon_data);
        let _r: bool = msg_send![icon_image, setTemplate: true];
        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item = status_bar.statusItemWithLength_(NSSquareStatusItemLength);
        let status_button = status_item.button();
        cocoa::appkit::NSButton::setImage_(status_button, icon_image);
        status_item.setMenu_(menu);
    }
}

fn take_screenshots(state: Arc<Mutex<State>>) {
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
        windows
    };

    (*state).lock().unwrap().window_count = windows.len();

    for window in windows {
        let jpeg = unsafe {
            let image = CGWindowListCreateImage(
                CGRectNull,
                kCGWindowListOptionIncludingWindow,
                window,
                kCGWindowImageDefault,
            );
            NSBitmapImageRep::alloc(nil)
                .initWithCGImage_(image)
                .representationUsingType_(NSBitmapImageFileType::NSBitmapImageFileTypeJPEG)
        };
        unsafe {
            NSData::writeToFile_atomically_(
                jpeg,
                NSString::alloc(nil).init_str("/Users/april/test.jpg"),
                NO,
            );
        }
    }
}

fn open_window(state: &mut State, window_delegate: id) {
    unsafe {
        let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(200., 200.)),
            NSWindowStyleMask::NSTitledWindowMask
                | NSWindowStyleMask::NSClosableWindowMask
                | NSWindowStyleMask::NSMiniaturizableWindowMask,
            NSBackingStoreBuffered,
            NO,
        );
        window.cascadeTopLeftFromPoint_(NSPoint::new(20., 20.));
        window.center();
        let title = NSString::alloc(nil).init_str("Hello World!").autorelease();
        window.setTitle_(title);
        window.setDelegate_(window_delegate);

        let count = state.window_count.to_string();
        let content = NSTextView::alloc(nil).initWithFrame_(window.contentView().frame());
        content.setEditable_(NO);
        content.setString_(NSString::alloc(nil).init_str(&count).autorelease());
        window.setContentView_(content);

        window.makeKeyAndOrderFront_(nil);

        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
        state.window_open = true;
    }
}

fn main() {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        if ScreenCaptureAccess::default().preflight() == false {
            println!("Don't have permission to take screenshots, exiting");
            return;
        }

        let state = Arc::new(Mutex::new(State {
            window_count: 0,
            window_open: false,
        }));
        let cloned = Arc::clone(&state);
        thread::spawn(|| {
            take_screenshots(cloned);
        });

        let window_delegate = delegate!("WindowDelegate", {
            state: *mut c_void = Arc::<Mutex<State>>::as_ptr(&state) as *const c_void,
            (windowWillClose:) => close as extern "C" fn(&Object, Sel, id)
        });

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
        app.setDelegate_(delegate!("AppDelegate", {
            state: *mut c_void = Arc::<Mutex<State>>::as_ptr(&state) as *const c_void,
            window_delegate: id = window_delegate,
            (open:) => open as extern "C" fn(&Object, Sel, id),
            (applicationShouldTerminateAfterLastWindowClosed:) => should_close as extern "C" fn(&Object, Sel, id) -> bool,
            (applicationWillTerminate:) => will_terminate as extern "C" fn(&Object, Sel, id)
        }));
        add_to_status_bar();
        app.run();
        pool.drain();
    }
}
