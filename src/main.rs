#[macro_use]
extern crate cocoa;
#[macro_use]
extern crate objc;

use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivateIgnoringOtherApps,
    NSApplicationActivationPolicyAccessory, NSBackingStoreBuffered, NSImage, NSMenu, NSMenuItem,
    NSRunningApplication, NSWindow, NSWindowStyleMask,
};
use cocoa::base::{id, nil, NO};
use cocoa::foundation::{NSAutoreleasePool, NSData, NSPoint, NSRect, NSSize, NSString};

use cocoa::appkit::{NSSquareStatusItemLength, NSStatusBar, NSStatusItem};
use objc::runtime::{Object, Sel};
use std::os::raw::c_void;

struct State {
    window_open: bool,
}

extern "C" fn should_close(_: &Object, _: Sel, _: id) -> bool {
    return false;
}

extern "C" fn close(this: &Object, _: Sel, _: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("state");
        let state = &mut *(state_ptr as *mut State);
        state.window_open = false;
    }
}

extern "C" fn open(this: &Object, _: Sel, _: id) {
    unsafe {
        let state_ptr: *mut c_void = *this.get_ivar("state");
        let state = &mut *(state_ptr as *mut State);
        let window_delegate: id = *this.get_ivar("window_delegate");
        if !state.window_open {
            state.window_open = true;
            open_window(window_delegate);
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

fn open_window(window_delegate: id) {
    unsafe {
        let window = NSWindow::alloc(nil)
            .initWithContentRect_styleMask_backing_defer_(
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

        window.makeKeyAndOrderFront_(nil);

        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
    }
}

fn main() {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        let mut state = State {
            window_open: false,
        };

        let window_delegate = delegate!("WindowDelegate", {
            state: *mut c_void = &mut state as *mut State as *mut c_void,
            (windowWillClose:) => close as extern "C" fn(&Object, Sel, id)
        });

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
        app.setDelegate_(delegate!("AppDelegate", {
            state: *mut c_void = &mut state as *mut State as *mut c_void,
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
