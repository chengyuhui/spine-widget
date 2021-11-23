use std::os::raw::c_int;

use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{
            GetKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RCONTROL,
            VK_RMENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
            WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};
use winit::{
    event::{ElementState, ModifiersState},
    event_loop::EventLoopProxy,
};

use crate::UserEvent;

static mut EVENT_PROXY: Option<EventLoopProxyWrapper> = None;

struct EventLoopProxyWrapper {
    inner: EventLoopProxy<UserEvent>,
}

/// Screw you, Rust.
/// # Safety
/// This is safe because we're only using it in the keyboard hook callback.
unsafe impl Sync for EventLoopProxyWrapper {}

const VK_SHIFT_VAL: u16 = VK_SHIFT.0;
const VK_LCONTROL_VAL: u16 = VK_LCONTROL.0;
const VK_RCONTROL_VAL: u16 = VK_RCONTROL.0;
const VK_LMENU_VAL: u16 = VK_LMENU.0;
const VK_RMENU_VAL: u16 = VK_RMENU.0;
const VK_LWIN_VAL: u16 = VK_LWIN.0;
const VK_RWIN_VAL: u16 = VK_RWIN.0;

unsafe extern "system" fn keyboard_proc(
    n_code: c_int,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    let vk = (*(std::mem::transmute::<_, *const KBDLLHOOKSTRUCT>(l_param))).vkCode;
    if n_code < 0
        || matches!(
            vk as u16,
            VK_SHIFT_VAL
                | VK_LCONTROL_VAL
                | VK_RCONTROL_VAL
                | VK_LMENU_VAL
                | VK_RMENU_VAL
                | VK_LWIN_VAL
                | VK_RWIN_VAL
        )
    {
        // Do not process message
        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
    }

    let mut modifiers_state = ModifiersState::default();

    for (vk, mask) in [
        (VK_SHIFT, ModifiersState::SHIFT),
        (VK_CONTROL, ModifiersState::CTRL),
        (VK_MENU, ModifiersState::ALT),
        (VK_LWIN, ModifiersState::LOGO),
        (VK_RWIN, ModifiersState::LOGO),
    ] {
        let status = GetKeyState(vk.0 as _) < 0;
        if status {
            modifiers_state |= mask;
        }
    }

    match w_param.0 as u32 {
        code if code == WM_KEYDOWN || code == WM_SYSKEYDOWN => {
            let _ = EVENT_PROXY
                .as_ref()
                .unwrap()
                .inner
                .send_event(UserEvent::GlobalKey {
                    state: ElementState::Pressed,
                    vk_code: vk,
                    modifiers: modifiers_state,
                });
        }
        code if code == WM_KEYUP || code == WM_SYSKEYUP => {
            let _ = EVENT_PROXY
                .as_ref()
                .unwrap()
                .inner
                .send_event(UserEvent::GlobalKey {
                    state: ElementState::Released,
                    vk_code: vk,
                    modifiers: modifiers_state,
                });
        }
        _ => {}
    }

    return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
}

pub struct KeyboardHook {
    hhk: HHOOK,
}

impl KeyboardHook {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        unsafe {
            EVENT_PROXY = Some(EventLoopProxyWrapper { inner: proxy });
        }
        let hhk = unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HINSTANCE::default(), 0)
        };
        Self { hhk }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        unsafe {
            UnhookWindowsHookEx(self.hhk);
        }
    }
}
