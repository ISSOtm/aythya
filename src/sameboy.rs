// SameBoy, oddly enough, doesn't respect Rust naming conventions!
#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]
// bindgen currently doesn't do internal `unsafe`.
#![allow(unsafe_op_in_unsafe_fn)]
// `bindgen` picks up on some libc functions. We don't use `u128`.
#![allow(improper_ctypes)]

use std::{mem::MaybeUninit, path::Path};

use slint::{SharedPixelBuffer, Weak};

use crate::MainWindow;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[repr(C)] // Necessary to cast the pointer to its `gb` member back to a pointer to the struct itself.
pub struct SameBoy {
    gb: GB_gameboy_t,
    framebuffer: Vec<u32>,
    main_window: Weak<MainWindow>,
}

impl SameBoy {
    pub fn new(main_window: Weak<MainWindow>) -> Self {
        let mut emu = MaybeUninit::uninit();
        // SAFETY: this very function is responsible for initialising the struct.
        unsafe { GB_init(emu.as_mut_ptr(), GB_model_t_GB_MODEL_DMG_B) };
        // SAFETY: this is called on an initialised struct.
        unsafe { GB_set_vblank_callback(emu.as_mut_ptr(), Some(Self::vblank_callback)) };
        // SAFETY: ditto.
        unsafe { GB_set_rgb_encode_callback(emu.as_mut_ptr(), Some(Self::rgb_encode_callback)) };
        // SAFETY: ditto.
        unsafe { GB_set_boot_rom_load_callback(emu.as_mut_ptr(), Some(Self::boot_rom_callback)) };
        let mut this = Self {
            // SAFETY: the above call initialised the struct.
            gb: unsafe { emu.assume_init() },
            framebuffer: vec![],
            main_window,
        };
        this.resize_framebuffer();
        this
    }
}

impl Drop for SameBoy {
    fn drop(&mut self) {
        // SAFETY: this function is intended to dealloc the struct.
        unsafe { GB_free(&mut self.gb) };
    }
}

impl SameBoy {
    /// Resizes the internal framebuffer in accordance with the new screen width and height.
    fn resize_framebuffer(&mut self) {
        // SAFETY: this is called on an instance initialised by the constructor.
        let width = unsafe { GB_get_screen_width(&mut self.gb) };
        // SAFETY: this is called on an instance initialised by the constructor.
        let height = unsafe { GB_get_screen_height(&mut self.gb) };
        self.framebuffer.resize((width * height) as usize, 0);
        // SAFETY: the pointer will remain unmodified by `self`, except by calls to `resize_framebuffer`, which refresh it.
        unsafe { GB_set_pixels_output(&mut self.gb, self.framebuffer.as_mut_ptr()) };
    }

    extern "C" fn vblank_callback(gb: *mut GB_gameboy_t, kind: GB_vblank_type_t) {
        // TODO: schedule the buffer to be rendered (unless we shouldn't)

        // SAFETY: This callback is called from one of the `run` functions, which are all called
        //         while holding a `&mut`. No other references are live here.
        //         Also, the pointer is guaranteed to be non-NULL.
        let this = unsafe { (gb as *mut SameBoy).as_mut().unwrap_unchecked() };
        // It's fine if we fail to update this due to the main loop being closed; we'll shut down soon anyway.
        let _ = slint::invoke_from_event_loop({
            let main_window = &this.main_window;
            let framebuffer = &this.framebuffer;
            // SAFETY: the instance is properly initialised.
            let width = unsafe { GB_get_screen_width(&mut this.gb) };
            // SAFETY: ditto.
            let height = unsafe { GB_get_screen_height(&mut this.gb) };
            move || {
                if let Some(main_window) = main_window.upgrade() {
                    main_window.set_screen(slint::Image::from_rgba8(
                        SharedPixelBuffer::clone_from_slice(
                            bytemuck::cast_slice(framebuffer),
                            width,
                            height,
                        ),
                    ));
                }
            }
        });
    }

    extern "C" fn rgb_encode_callback(_gb: *mut GB_gameboy_t, r: u8, g: u8, b: u8) -> u32 {
        u32::from_ne_bytes([r, g, b, 0xFF])
    }

    extern "C" fn boot_rom_callback(gb: *mut GB_gameboy_t, kind: GB_boot_rom_t) {
        const DMG_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/dmg_boot.bin");
        const MGB_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/mgb_boot.bin");
        const SGB_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/sgb_boot.bin");
        const SGB2_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/sgb2_boot.bin");
        const CGB0_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/cgb0_boot.bin");
        const CGB_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/cgb_boot.bin");
        const AGB_BOOT_ROM: &[u8] = include_bytes!("../SameBoy/build/bin/BootROMs/agb_boot.bin");

        match kind {
            // SAFETY: we are providing a buffer of the right size.
            GB_boot_rom_t_GB_BOOT_ROM_DMG_0 | GB_boot_rom_t_GB_BOOT_ROM_DMG => unsafe {
                GB_load_boot_rom_from_buffer(gb, DMG_BOOT_ROM.as_ptr(), DMG_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_MGB => unsafe {
                GB_load_boot_rom_from_buffer(gb, MGB_BOOT_ROM.as_ptr(), MGB_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_SGB => unsafe {
                GB_load_boot_rom_from_buffer(gb, SGB_BOOT_ROM.as_ptr(), SGB_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_SGB2 => unsafe {
                GB_load_boot_rom_from_buffer(gb, SGB2_BOOT_ROM.as_ptr(), SGB2_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_CGB0 => unsafe {
                GB_load_boot_rom_from_buffer(gb, CGB0_BOOT_ROM.as_ptr(), CGB0_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_CGB => unsafe {
                GB_load_boot_rom_from_buffer(gb, CGB_BOOT_ROM.as_ptr(), CGB_BOOT_ROM.len())
            },
            // SAFETY: Ditto.
            GB_boot_rom_t_GB_BOOT_ROM_AGB => unsafe {
                GB_load_boot_rom_from_buffer(gb, AGB_BOOT_ROM.as_ptr(), AGB_BOOT_ROM.len())
            },
        }
    }
}

impl SameBoy {
    pub fn run_once(&mut self) {
        // SAFETY: `gb` is initialised, and not running (we couldn't have a mutable ref to it otherwise).
        unsafe { GB_run(&mut self.gb) };
    }

    pub fn step(&mut self) {
        // SAFETY: `gb` is initialised, and not running (we couldn't have a mutable ref to it otherwise).
        unsafe {
            GB_set_turbo_mode(&mut self.gb, true, true);
            let _ = GB_run(&mut self.gb);
            GB_set_turbo_mode(&mut self.gb, false, true);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Schedule {
    Stop,
    Run,
    Step,
    RunFrame,

    Quit,
}

impl SameBoy {
    pub fn change_model(&mut self, model: GB_model_t) {
        // SAFETY: the instance is initialised by `new`, and not running thanks to the mutable reference.
        unsafe { GB_switch_model_and_reset(&mut self.gb, model) };
        self.resize_framebuffer();
    }

    pub fn load_rom(&mut self, path: &Path) {
        use std::ffi::CString;
        #[cfg(unix)]
        fn convert_path(path: &Path) -> Option<CString> {
            use std::os::unix::ffi::OsStrExt;
            CString::new(path.as_os_str().as_bytes()).ok()
        }
        #[cfg(not(unix))]
        fn convert_path(path: &Path) -> CString {
            CString::new(path.to_str()?).ok() // Laziness on my behalf? Certainly.
        }
        match convert_path(path) {
            Some(c_path) => {
                // SAFETY: Initialisation is done and all necessary callbacks are set in `new`.
                let err_code = unsafe { GB_load_rom(&mut self.gb, c_path.as_ptr()) };
            }
            None => todo!(), // Report error
        }
    }
}
