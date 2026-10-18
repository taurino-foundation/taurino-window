// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::MonitorExt;
use anyhow::Result;
#[cfg_attr(not(windows), allow(unused_imports))]
pub use imp::*;
use parking_lot::{Mutex, RwLock};
use std::{collections::HashMap, sync::Arc};
use tao::event::DeviceId;

#[cfg(not(windows))]
mod imp {}

#[cfg(windows)]
mod imp {
    use {
        once_cell::sync::Lazy,
        std::{iter::once, os::windows::ffi::OsStrExt},
        windows::{
            Win32::{
                Foundation::*,
                Graphics::Gdi::*,
                System::LibraryLoader::{GetProcAddress, LoadLibraryW},
                UI::{HiDpi::*, WindowsAndMessaging::*},
            },
            core::{HRESULT, PCSTR, PCWSTR},
        },
    };

    pub fn encode_wide(string: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
        string.as_ref().encode_wide().chain(once(0)).collect()
    }

    // Helper function to dynamically load function pointer.
    // `library` and `function` must be zero-terminated.
    pub(super) fn get_function_impl(library: &str, function: &str) -> FARPROC {
        let library = encode_wide(library);
        assert_eq!(function.chars().last(), Some('\0'));

        // Library names we will use are ASCII so we can use the A version to avoid string conversion.
        let module = unsafe { LoadLibraryW(PCWSTR::from_raw(library.as_ptr())) }.unwrap_or_default();
        if module.is_invalid() {
            return None;
        }

        unsafe { GetProcAddress(module, PCSTR::from_raw(function.as_ptr())) }
    }

    macro_rules! get_function {
        ($lib:expr, $func:ident) => {
            $crate::utils::get_function_impl($lib, concat!(stringify!($func), '\0'))
                .map(|f| unsafe { std::mem::transmute::<_, $func>(f) })
        };
    }

    type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> u32;
    type GetDpiForMonitor = unsafe extern "system" fn(
        hmonitor: HMONITOR,
        dpi_type: MONITOR_DPI_TYPE,
        dpi_x: *mut u32,
        dpi_y: *mut u32,
    ) -> HRESULT;
    type GetSystemMetricsForDpi = unsafe extern "system" fn(nindex: SYSTEM_METRICS_INDEX, dpi: u32) -> i32;

    static GET_DPI_FOR_WINDOW: Lazy<Option<GetDpiForWindow>> =
        Lazy::new(|| get_function!("user32.dll", GetDpiForWindow));
    static GET_DPI_FOR_MONITOR: Lazy<Option<GetDpiForMonitor>> =
        Lazy::new(|| get_function!("shcore.dll", GetDpiForMonitor));
    static GET_SYSTEM_METRICS_FOR_DPI: Lazy<Option<GetSystemMetricsForDpi>> =
        Lazy::new(|| get_function!("user32.dll", GetSystemMetricsForDpi));

    #[allow(non_snake_case)]
    pub unsafe fn hwnd_dpi(hwnd: HWND) -> u32 {
        unsafe {
            if let Some(GetDpiForWindow) = *GET_DPI_FOR_WINDOW {
                // We are on Windows 10 Anniversary Update (1607) or later.
                match GetDpiForWindow(hwnd) {
                    0 => USER_DEFAULT_SCREEN_DPI, // 0 is returned if hwnd is invalid
                    dpi => dpi,
                }
            } else if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
                // We are on Windows 8.1 or later.
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                if monitor.is_invalid() {
                    return USER_DEFAULT_SCREEN_DPI;
                }

                let mut dpi_x = 0;
                let mut dpi_y = 0;
                if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_ok() {
                    dpi_x
                } else {
                    USER_DEFAULT_SCREEN_DPI
                }
            } else {
                // We are on Vista or later.
                if IsProcessDPIAware().as_bool() {
                    let hdc = GetDC(Some(hwnd));
                    if hdc.is_invalid() {
                        return USER_DEFAULT_SCREEN_DPI;
                    }
                    // If the process is DPI aware, then scaling must be handled by the application using
                    // this DPI value.
                    let dpi = GetDeviceCaps(Some(hdc), LOGPIXELSX) as u32;
                    ReleaseDC(Some(hwnd), hdc);
                    dpi
                } else {
                    // If the process is DPI unaware, then scaling is performed by the OS; we thus return
                    // 96 (scale factor 1.0) to prevent the window from being re-scaled by both the
                    // application and the WM.
                    USER_DEFAULT_SCREEN_DPI
                }
            }
        }
    }

    #[allow(non_snake_case)]
    pub unsafe fn get_system_metrics_for_dpi(nindex: SYSTEM_METRICS_INDEX, dpi: u32) -> i32 {
        unsafe {
            if let Some(GetSystemMetricsForDpi) = *GET_SYSTEM_METRICS_FOR_DPI {
                GetSystemMetricsForDpi(nindex, dpi)
            } else {
                GetSystemMetrics(nindex)
            }
        }
    }
}

pub fn calculate_window_center_position(
    window_size: tao::dpi::PhysicalSize<u32>,
    target_monitor: tao::monitor::MonitorHandle,
) -> tao::dpi::PhysicalPosition<i32> {
    let work_area = target_monitor.work_area();

    tao::dpi::PhysicalPosition::new(
        (work_area.size.width as i32 - window_size.width as i32) / 2 + work_area.position.x,
        (work_area.size.height as i32 - window_size.height as i32) / 2 + work_area.position.y,
    )
}

struct DeviceRegistryInner {
    next_id: u32,
    by_device: HashMap<DeviceId, u32>,
    by_id: HashMap<u32, DeviceId>,
}

#[derive(Clone)]
pub struct DeviceRegistry {
    inner: Arc<RwLock<DeviceRegistryInner>>,
}

impl DeviceRegistry {
    pub fn new() -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self {
            inner: Arc::new(RwLock::new(DeviceRegistryInner {
                next_id: 1,
                by_device: HashMap::new(),
                by_id: HashMap::new(),
            })),
        })))
    }

    fn next_available_id(inner: &mut DeviceRegistryInner) -> u32 {
        loop {
            let id = inner.next_id;

            inner.next_id = inner.next_id.checked_add(1).unwrap_or(1);

            if !inner.by_id.contains_key(&id) {
                return id;
            }
        }
    }

    /// Returns the existing runtime id for this device.
    /// If the device is not registered yet, creates a new id first.
    pub fn get_or_create_id(&self, device_id: DeviceId) -> u32 {
        let mut inner = self.inner.write();

        if let Some(id) = inner.by_device.get(&device_id) {
            return *id;
        }

        let id = Self::next_available_id(&mut inner);

        inner.by_device.insert(device_id, id);
        inner.by_id.insert(id, device_id);

        id
    }

    pub fn get_id(&self, device_id: &DeviceId) -> Option<u32> {
        self.inner.read().by_device.get(device_id).copied()
    }

    pub fn get_device(&self, id: u32) -> Option<DeviceId> {
        self.inner.read().by_id.get(&id).copied()
    }

    pub fn remove_by_device(&self, device_id: &DeviceId) -> Option<u32> {
        let mut inner = self.inner.write();

        let id = inner.by_device.remove(device_id)?;
        inner.by_id.remove(&id);

        Some(id)
    }

    pub fn remove_by_id(&self, id: u32) -> Option<DeviceId> {
        let mut inner = self.inner.write();

        let device_id = inner.by_id.remove(&id)?;
        inner.by_device.remove(&device_id);

        Some(device_id)
    }

    pub fn clear(&self) {
        let mut inner = self.inner.write();

        inner.by_device.clear();
        inner.by_id.clear();
        inner.next_id = 1;
    }
}
