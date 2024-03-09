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
use objc::declare::ClassDecl;
use objc::runtime::{Object, Sel};
use std::os::raw::c_void;

extern "C" fn close(_: &Object, _: Sel, _: id) {
    // Having this prevents the app from crashing even though it doesn't seem to be called
}

extern "C" fn open(_: &Object, _: Sel, _: id) {
    open_window();
}

fn create_delegate() -> id {
    unsafe {
        let mut delegate_decl = ClassDecl::new("AppDelegate", class!(NSObject)).unwrap();
        // Required to prevent the app from crashing when the close button is pressed
        delegate_decl.add_method(sel!(windowWillClose:), close as extern "C" fn(&Object, Sel, id));
        delegate_decl.add_method(sel!(open:), open as extern "C" fn(&Object, Sel, id));
        let delegate_class = delegate_decl.register();
        return msg_send![delegate_class, new];
    }
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
            NSData::dataWithBytes_length_(nil, icon.as_ptr() as *mut c_void, icon.len() as u64)
                .autorelease();
        let icon_image = NSImage::initWithData_(NSImage::alloc(nil), icon_data).autorelease();
        let _r: bool = msg_send![icon_image, setTemplate: true];
        let status_bar = NSStatusBar::systemStatusBar(nil);
        let status_item = status_bar.statusItemWithLength_(NSSquareStatusItemLength);
        let status_button = status_item.button();
        cocoa::appkit::NSButton::setImage_(status_button, icon_image);
        status_item.setMenu_(menu);
    }
}

fn open_window() {
    unsafe {
        let window = NSWindow::alloc(nil)
            .initWithContentRect_styleMask_backing_defer_(
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(200., 200.)),
                NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask,
                NSBackingStoreBuffered,
                NO,
            )
            .autorelease();
        window.cascadeTopLeftFromPoint_(NSPoint::new(20., 20.));
        window.center();
        let title = NSString::alloc(nil).init_str("Hello World!").autorelease();
        window.setTitle_(title);
        window.makeKeyAndOrderFront_(nil);

        let current_app = NSRunningApplication::currentApplication(nil);
        current_app.activateWithOptions_(NSApplicationActivateIgnoringOtherApps);
    }
}

fn main() {
    unsafe {
        NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
        app.setDelegate_(create_delegate());
        add_to_status_bar();
        app.run();
    }
}
