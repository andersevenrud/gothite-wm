/**
 * gothite wn
 * Author: Anders Evenrud <andersevenrud@gmail.com>
 */

#[macro_use]
extern crate log;
extern crate x11;
extern crate vector2d;

use std::ffi::CString;
use std::collections::HashMap;
use std::mem::{zeroed, uninitialized};
use std::os::raw::{c_void};
use std::cmp::{max};
use std::ptr;
use x11::xlib;
use x11::keysym;
use vector2d::Vector2D;

const DECORATION_PADDING: i32 = 10;

/**
 * The catch-all error reporter
 */
extern "C" fn error_handler(_display: *mut xlib::Display, _ev: *mut xlib::XErrorEvent) -> i32 {
    // TODO: Get real error message
    unsafe {
        error!("X11 Error (request code): {}", (*_ev).request_code);
    }

    0
}

/**
 * Window structure
 */
struct Window {
    frame: xlib::Window,
    decoration: xlib::Window,
    drag_start: Vector2D<i32>,
    drag_start_size: Vector2D<u32>
}

/**
 * Window Manager structure
 */
struct WindowManager {
    display: *mut xlib::Display,
    root: xlib::Window,
    windows: HashMap<xlib::Window, Window>,
    drag_start: Vector2D<i32>
}

/**
 * Re-frames any windows that was spawned before the WM started up
 */
fn reparent_initial_windows(_wm: &mut WindowManager) {
    unsafe {
        xlib::XGrabServer(_wm.display);

        let mut root: xlib::Window = uninitialized();
        let mut parent: xlib::Window = uninitialized();
        let mut windows: *mut xlib::Window = uninitialized();
        let mut count: u32 = 0;

        xlib::XQueryTree(_wm.display, _wm.root, &mut root, &mut parent, &mut windows, &mut count);

        if root == _wm.root {
            debug!("Reparenting {} windows", count);

            for _i in 0..count {
                create_window_frame(_wm, *windows.offset(_i as isize), true);
            }
        }

        xlib::XFree(windows as *mut c_void);
        xlib::XUngrabServer(_wm.display);
    }
}

/**
 * Binds a input button to a window
 */
fn bind_window_button(_wm: &WindowManager, _w: xlib::Window, _b: u32, _m: u32) {
    unsafe {
        xlib::XGrabButton(
            _wm.display,
            _b,
            _m,
            _w,
            0,
            xlib::ButtonPressMask as u32 | xlib::ButtonReleaseMask as u32 | xlib::ButtonMotionMask as u32,
            xlib::GrabModeAsync,
            xlib::GrabModeAsync,
            0,
            0);
    }
}

/**
 * Binds a input key to a window
 */
fn bind_window_key(_wm: &WindowManager, _w: xlib::Window, _k: u32, _m: u32) {
    unsafe {
        xlib::XGrabKey(
            _wm.display,
            xlib::XKeysymToKeycode(_wm.display, _k as u64) as i32,
            _m,
            _w,
            0,
            xlib::GrabModeAsync,
            xlib::GrabModeAsync);
    }
}

/**
 * Resizes a window
 */
fn resize_window(_wm: &WindowManager, _w: xlib::Window, _win: &Window, delta: Vector2D<i32>) {
    let new_dimension = _win.drag_start_size.as_i32s() + delta;
    let new_dimension = Vector2D::new(
        max(10, new_dimension.x),
        max(10, new_dimension.y)
    ).as_u32s();

    unsafe {
        xlib::XResizeWindow(_wm.display, _win.decoration,
                            new_dimension.x + (DECORATION_PADDING * 2) as u32,
                            new_dimension.y + (DECORATION_PADDING * 2) as u32);
        xlib::XResizeWindow(_wm.display, _win.frame, new_dimension.x, new_dimension.y);
        xlib::XResizeWindow(_wm.display, _w, new_dimension.x, new_dimension.y);
    }
}

/**
 * Moves a window
 */
fn move_window(_wm: &WindowManager, _w: xlib::Window, _win: &Window, delta: Vector2D<i32>) {
    let new_position = _win.drag_start + delta;
    let decoration_position = new_position - Vector2D::new(DECORATION_PADDING, DECORATION_PADDING);

    unsafe {
        xlib::XMoveWindow(_wm.display, _win.frame, new_position.x, new_position.y);
        xlib::XMoveWindow(_wm.display, _win.decoration, decoration_position.x, decoration_position.y);
    }
}

/**
 * Checks if a window can be gracefully killed
 */
fn can_kill_window_gracefully(_wm: &mut WindowManager, _w: xlib::Window) -> bool {
    let mut atoms: *mut xlib::Atom = unsafe { uninitialized() };
    let mut atom_count: i32 = 0;

    let result = unsafe {
        xlib::XGetWMProtocols(_wm.display,
                              _w,
                              &mut atoms,
                              &mut atom_count)
    };

    if result == 0 {
        return false;
    }

    let wm_delete_window_str = CString::new("WM_DELETE_WINDOW").unwrap();
    let delete_atom = unsafe { xlib::XInternAtom(_wm.display, wm_delete_window_str.as_ptr(), xlib::False) };

    // FIXME There must me an alternative for a loop
    for _i in 0..atom_count {
        let v = unsafe { atoms.offset(_i as isize) };
        if unsafe { *v == delete_atom } {
            return true;
        }
    }

    return false;
}

/**
 * Kills a window
 */
fn kill_window(_wm: &mut WindowManager, _w: xlib::Window) {
    if can_kill_window_gracefully(_wm, _w) {
        let mut ev: xlib::XEvent = unsafe { uninitialized() };
        let wm_protocols_str = CString::new("WM_PROTOCOLS").unwrap();
        let wm_delete_window_str = CString::new("WM_DELETE_WINDOW").unwrap();

        unsafe {
            let wm_protocols = xlib::XInternAtom(_wm.display, wm_protocols_str.as_ptr(), xlib::False);
            let wm_delete_window = xlib::XInternAtom(_wm.display, wm_delete_window_str.as_ptr(), xlib::False);

            ev.client_message.type_ = xlib::ClientMessage;
            ev.client_message.message_type = wm_protocols;
            ev.client_message.window = _w;
            ev.client_message.format = 32;
            ev.client_message.data.set_long(0, wm_delete_window as i64);

            xlib::XSendEvent(_wm.display, _w, xlib::False, 0, &mut ev);
        }

        debug!("Gracefully killed window");
    } else {
        unsafe {
            xlib::XKillClient(_wm.display, _w);
        }

        debug!("Killed window");
    }
}

/**
 * Removes a window frame
 */
fn remove_window_frame(_wm: &mut WindowManager, _w: xlib::Window) {
    if !_wm.windows.contains_key(&_w) {
        return;
    }

    let win = _wm.windows.get(&_w).unwrap();
    unsafe {
        xlib::XUnmapWindow(_wm.display, win.decoration);
        xlib::XUnmapWindow(_wm.display, win.frame);
        xlib::XReparentWindow(_wm.display, _w, _wm.root, 0, 0);
        xlib::XRemoveFromSaveSet(_wm.display, _w);
        xlib::XDestroyWindow(_wm.display, win.frame);
    }

    _wm.windows.remove(&_w);
}

/**
 * Creates a window frame
 */
fn create_window_frame(_wm: &mut WindowManager, _w: xlib::Window, early: bool) {
    unsafe {
        let mut attrs: xlib::XWindowAttributes = uninitialized();

        xlib::XGetWindowAttributes(_wm.display, _w, &mut attrs);

        if early && (attrs.override_redirect > 0 || attrs.map_state != xlib::IsViewable) {
            return;
        }

        let decoration = xlib::XCreateSimpleWindow(
            _wm.display,
            _wm.root,
            attrs.x - DECORATION_PADDING,
            attrs.y - DECORATION_PADDING,
            (attrs.width + (DECORATION_PADDING * 2)) as u32,
            (attrs.height + (DECORATION_PADDING * 2)) as u32,
            0,
            0xfff000,
            0xfff000);

        let frame = xlib::XCreateSimpleWindow(
            _wm.display,
            _wm.root,
            attrs.x,
            attrs.y,
            attrs.width as u32,
            attrs.height as u32,
            0,
            0xffffff,
            0x000000);

        xlib::XSelectInput(_wm.display, frame, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask);

        bind_window_button(_wm, _w, xlib::Button1, xlib::Mod1Mask);
        bind_window_button(_wm, _w, xlib::Button3, xlib::Mod1Mask);
        bind_window_key(_wm, _w, keysym::XK_F4, xlib::Mod1Mask);

        xlib::XAddToSaveSet(_wm.display, _w);
        xlib::XReparentWindow(_wm.display, _w, frame, 0, 0);
        xlib::XMapWindow(_wm.display, decoration);
        xlib::XMapWindow(_wm.display, frame);

        _wm.windows.insert(_w, Window {
            frame: frame,
            decoration: decoration,
            drag_start: Vector2D::new(0, 0),
            drag_start_size: Vector2D::new(0, 0)
        });
    }
}

/**
 * Handle reparent notification event
 */
fn on_reparent_notify(_wm: &WindowManager, _e: xlib::XReparentEvent) {
    // Ignore for now
}

/**
 * Handle unmap notification event
 */
fn on_unmap_notify(_wm: &mut WindowManager, _e: xlib::XUnmapEvent) {
    if !_wm.windows.contains_key(&_e.window) {
        warn!("Ignoring UnmapNotify for {}", _e.window);
        return;
    }

    if _e.event == _wm.root {
        debug!("Ignoring UnmapNotify for root");
        return;
    }

    let win = _wm.windows.get(&_e.window).unwrap();

    unsafe {
        // FIXME This triggers an error
        xlib::XUnmapWindow(_wm.display, win.frame);
        xlib::XReparentWindow(_wm.display, _e.window, _wm.root, 0, 0);
        xlib::XRemoveFromSaveSet(_wm.display, _e.window);
        xlib::XDestroyWindow(_wm.display, win.frame);
    }

    remove_window_frame(_wm, _e.window);
}

/**
 * Handle map notification event
 */
fn on_map_notify(_wm: &WindowManager, _e: xlib::XMapEvent) {
    // Ignore for now
}

/**
 * Handle map request event
 */
fn on_map_request(_wm: &mut WindowManager, _e: xlib::XMapRequestEvent) {
    create_window_frame(_wm, _e.window, false);

    unsafe {
        xlib::XMapWindow(_wm.display, _e.window);
    }
}

/**
 * Handle motion notification event
 */
fn on_motion_notify(_wm: &WindowManager, _e: xlib::XMotionEvent) {
    if !_wm.windows.contains_key(&_e.window) {
        return;
    }

    let win = _wm.windows.get(&_e.window).unwrap();
    let position = Vector2D::new(_e.x_root, _e.y_root);
    let delta =  position - _wm.drag_start;

    if _e.state & xlib::Button1Mask != 0 {
        move_window(_wm, _e.window, win, delta);
    } else if _e.state & xlib::Button3Mask != 0 {
        resize_window(_wm, _e.window, win, delta);
    }
}

/**
 * Handle configuration notification event
 */
fn on_configure_notify(_wm: &WindowManager, _e: xlib::XConfigureEvent) {
    // Ignore for now
}

/**
 * Handle configuration request event
 */
fn on_configure_request(_wm: &WindowManager, _e: xlib::XConfigureRequestEvent) {
    let mut changes: xlib::XWindowChanges = unsafe { zeroed() };
    changes.x = _e.x;
    changes.y = _e.y;
    changes.width = _e.width;
    changes.height = _e.height;
    changes.border_width = _e.border_width;
    changes.sibling = _e.above;
    changes.stack_mode = _e.detail;

    unsafe {
        if _wm.windows.contains_key(&_e.window) {
            let win = _wm.windows.get(&_e.window).unwrap();
            xlib::XConfigureWindow(_wm.display, win.frame, _e.value_mask as u32, &mut changes);
        }

        xlib::XConfigureWindow(_wm.display, _e.window, _e.value_mask as u32, &mut changes);
    }
}

/**
 * Handle destruction notification event
 */
fn on_destroy_notify(_wm: &WindowManager, _e: xlib::XDestroyWindowEvent) {
    // Ignore for now
}

/**
 * Handle creation notification event
 */
fn on_create_notify(_wm: &WindowManager, _e: xlib::XCreateWindowEvent) {
    // Ignore for now
}

/**
 * Handle button press event
 */
fn on_button_press(_wm: &mut WindowManager, _e: xlib::XButtonEvent) {
    if !_wm.windows.contains_key(&_e.window) {
        return;
    }

    let win = _wm.windows.get_mut(&_e.window).unwrap();
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut w: u32 = 0;
    let mut h: u32 = 0;
    let mut border: u32 = 0;
    let mut depth: u32 = 0;

    unsafe {
        let mut root: xlib::Window = uninitialized();
        xlib::XGetGeometry(_wm.display, win.frame, &mut root, &mut x, &mut y, &mut w, &mut h, &mut border, &mut depth);
        xlib::XRaiseWindow(_wm.display, win.frame);
    }

    _wm.drag_start = Vector2D { x: _e.x_root, y: _e.y_root };
    win.drag_start = Vector2D { x: x, y: y };
    win.drag_start_size = Vector2D { x: w, y: h };
}

/**
 * Handle button release event
 */
fn on_button_release(_wm: &WindowManager, _e: xlib::XButtonEvent) {
    // Ignore for now
}

/**
 * Handle key press event
 */
fn on_key_press(_wm: &mut WindowManager, _e: xlib::XKeyEvent) {
    if _e.state & xlib::Mod1Mask > 0 {
        if _e.keycode == unsafe { xlib::XKeysymToKeycode(_wm.display, keysym::XK_F4 as u64) as u32 } {
            kill_window(_wm, _e.window);
        }
    }
}

/**
 * Handle key release event
 */
fn on_key_release(_wm: &WindowManager, _e: xlib::XKeyEvent) {
    // Ignore for now
}

/**
 * Program
 */
fn main() {
    env_logger::init();

    unsafe {
        xlib::XInitThreads();
    }

    let display = unsafe { xlib::XOpenDisplay(ptr::null()) };
    if display.is_null() {
        panic!("Failed to open display");
    }

    info!("Opened display");

    unsafe {
        xlib::XSetErrorHandler(Some(error_handler));
    }

    let screen = unsafe { xlib::XDefaultScreenOfDisplay(display) };
    let root = unsafe { xlib::XRootWindowOfScreen(screen) };

    unsafe {
        xlib::XSelectInput(display, root, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask);
        xlib::XSync(display, 0);
    }

    let mut wm = WindowManager {
        display: display,
        root: root,
        windows: HashMap::new(),
        drag_start: Vector2D { x: 0, y: 0 }
    };

    reparent_initial_windows(&mut wm);

    info!("Starting event loop");

    loop {
        let mut ev: xlib::XEvent = unsafe { uninitialized() };

        unsafe {
            xlib::XNextEvent(display, &mut ev);

            match ev.get_type() {
                xlib::ConfigureRequest => on_configure_request(&wm, ev.configure_request),
                xlib::ConfigureNotify => on_configure_notify(&wm, ev.configure),
                xlib::CreateNotify => on_create_notify(&wm, ev.create_window),
                xlib::DestroyNotify => on_destroy_notify(&wm, ev.destroy_window),
                xlib::ReparentNotify => on_reparent_notify(&wm, ev.reparent),
                xlib::MapNotify => on_map_notify(&wm, ev.map),
                xlib::MapRequest => on_map_request(&mut wm, ev.map_request),
                xlib::UnmapNotify => on_unmap_notify(&mut wm, ev.unmap),
                xlib::ButtonPress => on_button_press(&mut wm, ev.button),
                xlib::ButtonRelease => on_button_release(&wm, ev.button),
                xlib::KeyPress => on_key_press(&mut wm, ev.key),
                xlib::KeyRelease => on_key_release(&wm, ev.key),

                xlib::MotionNotify => {
                    while xlib::XCheckTypedWindowEvent(display, ev.motion.window, xlib::MotionNotify, &mut ev) > 0 {
                        // Skip pending motion evets
                    }

                    on_motion_notify(&wm, ev.motion);
                },

                _ => {
                    info!("Did not handle event of type {}", ev.get_type());
                    // void
                }
            }
        }
    }

    unsafe {
        xlib::XCloseDisplay(display);
    }
}
