use winit::{platform::windows::WindowExtWindows, window::Window};

use windows::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WINDOW_EX_STYLE, WS_EX_LAYERED,
        WS_EX_TRANSPARENT,
    },
};

pub trait SpineWidgetWindowExt: WindowExtWindows {
    /// Make this window clickable or not (clicking passthrough)
    fn set_click_passthrough(&self, passthrough: bool);
}

impl SpineWidgetWindowExt for Window {
    fn set_click_passthrough(&self, passthrough: bool) {
        unsafe {
            let hwnd: HWND = std::mem::transmute(self.hwnd());
            let window_styles: WINDOW_EX_STYLE = match GetWindowLongPtrW(hwnd, GWL_EXSTYLE) {
                0 => panic!("GetWindowLongPtrW failed"),
                n => n.try_into().unwrap(),
            };

            let window_styles = if passthrough {
                window_styles | WS_EX_TRANSPARENT | WS_EX_LAYERED //| WS_EX_TOOLWINDOW
            } else {
                window_styles & !WS_EX_TRANSPARENT | WS_EX_LAYERED //| WS_EX_TOOLWINDOW
            };

            if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, window_styles.try_into().unwrap()) == 0 {
                panic!("SetWindowLongPtrW failed");
            }
        }
    }
}
