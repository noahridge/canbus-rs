//! Raw FFI bindings to the PEAK PCAN-Basic API.
//!
//! Private to this crate: loaded at runtime via `libloading`. The safe
//! wrapper lives in the sibling modules.
//!
//! The library is searched for in this order:
//!   1. `PCAN_SDK_DIR` environment variable (looks in the `x64/` subdir on
//!      64-bit Windows, `x86/` on 32-bit Windows, root otherwise)
//!   2. System default search path
//!
//! On Windows the DLL is `PCANBasic.dll`; elsewhere it's `libpcanbasic.so`.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// Mirrors the C header; not every constant or symbol is used yet.
#![allow(dead_code)]

use std::sync::OnceLock;

// ---- Type aliases ----

pub type TPCANHandle = u16;
pub type TPCANStatus = u32;
pub type TPCANParameter = u8;
pub type TPCANDevice = u8;
pub type TPCANMessageType = u8;
pub type TPCANMode = u8;
pub type TPCANBaudrate = u16;
pub type TPCANBitrateFD = *mut std::os::raw::c_char;
pub type TPCANTimestampFD = u64;

// ---- Channel handles ----

pub const PCAN_NONEBUS: TPCANHandle = 0x00;

pub const PCAN_PCIBUS1: TPCANHandle = 0x041;
pub const PCAN_PCIBUS2: TPCANHandle = 0x042;
pub const PCAN_PCIBUS3: TPCANHandle = 0x043;
pub const PCAN_PCIBUS4: TPCANHandle = 0x044;
pub const PCAN_PCIBUS5: TPCANHandle = 0x045;
pub const PCAN_PCIBUS6: TPCANHandle = 0x046;
pub const PCAN_PCIBUS7: TPCANHandle = 0x047;
pub const PCAN_PCIBUS8: TPCANHandle = 0x048;
pub const PCAN_PCIBUS9: TPCANHandle = 0x409;
pub const PCAN_PCIBUS10: TPCANHandle = 0x40A;
pub const PCAN_PCIBUS11: TPCANHandle = 0x40B;
pub const PCAN_PCIBUS12: TPCANHandle = 0x40C;
pub const PCAN_PCIBUS13: TPCANHandle = 0x40D;
pub const PCAN_PCIBUS14: TPCANHandle = 0x40E;
pub const PCAN_PCIBUS15: TPCANHandle = 0x40F;
pub const PCAN_PCIBUS16: TPCANHandle = 0x410;

pub const PCAN_USBBUS1: TPCANHandle = 0x051;
pub const PCAN_USBBUS2: TPCANHandle = 0x052;
pub const PCAN_USBBUS3: TPCANHandle = 0x053;
pub const PCAN_USBBUS4: TPCANHandle = 0x054;
pub const PCAN_USBBUS5: TPCANHandle = 0x055;
pub const PCAN_USBBUS6: TPCANHandle = 0x056;
pub const PCAN_USBBUS7: TPCANHandle = 0x057;
pub const PCAN_USBBUS8: TPCANHandle = 0x058;
pub const PCAN_USBBUS9: TPCANHandle = 0x509;
pub const PCAN_USBBUS10: TPCANHandle = 0x50A;
pub const PCAN_USBBUS11: TPCANHandle = 0x50B;
pub const PCAN_USBBUS12: TPCANHandle = 0x50C;
pub const PCAN_USBBUS13: TPCANHandle = 0x50D;
pub const PCAN_USBBUS14: TPCANHandle = 0x50E;
pub const PCAN_USBBUS15: TPCANHandle = 0x50F;
pub const PCAN_USBBUS16: TPCANHandle = 0x510;

pub const PCAN_LANBUS1: TPCANHandle = 0x801;
pub const PCAN_LANBUS2: TPCANHandle = 0x802;
pub const PCAN_LANBUS3: TPCANHandle = 0x803;
pub const PCAN_LANBUS4: TPCANHandle = 0x804;
pub const PCAN_LANBUS5: TPCANHandle = 0x805;
pub const PCAN_LANBUS6: TPCANHandle = 0x806;
pub const PCAN_LANBUS7: TPCANHandle = 0x807;
pub const PCAN_LANBUS8: TPCANHandle = 0x808;
pub const PCAN_LANBUS9: TPCANHandle = 0x809;
pub const PCAN_LANBUS10: TPCANHandle = 0x80A;
pub const PCAN_LANBUS11: TPCANHandle = 0x80B;
pub const PCAN_LANBUS12: TPCANHandle = 0x80C;
pub const PCAN_LANBUS13: TPCANHandle = 0x80D;
pub const PCAN_LANBUS14: TPCANHandle = 0x80E;
pub const PCAN_LANBUS15: TPCANHandle = 0x80F;
pub const PCAN_LANBUS16: TPCANHandle = 0x810;

// ---- Status codes (bitmask) ----

pub const PCAN_ERROR_OK: TPCANStatus = 0x00000;
pub const PCAN_ERROR_XMTFULL: TPCANStatus = 0x00001;
pub const PCAN_ERROR_OVERRUN: TPCANStatus = 0x00002;
pub const PCAN_ERROR_BUSLIGHT: TPCANStatus = 0x00004;
pub const PCAN_ERROR_BUSHEAVY: TPCANStatus = 0x00008;
pub const PCAN_ERROR_BUSWARNING: TPCANStatus = PCAN_ERROR_BUSHEAVY;
pub const PCAN_ERROR_BUSPASSIVE: TPCANStatus = 0x40000;
pub const PCAN_ERROR_BUSOFF: TPCANStatus = 0x00010;
pub const PCAN_ERROR_QRCVEMPTY: TPCANStatus = 0x00020;
pub const PCAN_ERROR_QOVERRUN: TPCANStatus = 0x00040;
pub const PCAN_ERROR_QXMTFULL: TPCANStatus = 0x00080;
pub const PCAN_ERROR_REGTEST: TPCANStatus = 0x00100;
pub const PCAN_ERROR_NODRIVER: TPCANStatus = 0x00200;
pub const PCAN_ERROR_HWINUSE: TPCANStatus = 0x00400;
pub const PCAN_ERROR_NETINUSE: TPCANStatus = 0x00800;
pub const PCAN_ERROR_ILLHW: TPCANStatus = 0x01400;
pub const PCAN_ERROR_ILLNET: TPCANStatus = 0x01800;
pub const PCAN_ERROR_ILLCLIENT: TPCANStatus = 0x01C00;
pub const PCAN_ERROR_RESOURCE: TPCANStatus = 0x02000;
pub const PCAN_ERROR_ILLPARAMTYPE: TPCANStatus = 0x04000;
pub const PCAN_ERROR_ILLPARAMVAL: TPCANStatus = 0x08000;
pub const PCAN_ERROR_UNKNOWN: TPCANStatus = 0x10000;
pub const PCAN_ERROR_ILLDATA: TPCANStatus = 0x20000;
pub const PCAN_ERROR_ILLMODE: TPCANStatus = 0x80000;
pub const PCAN_ERROR_CAUTION: TPCANStatus = 0x2000000;
pub const PCAN_ERROR_INITIALIZE: TPCANStatus = 0x4000000;
pub const PCAN_ERROR_ILLOPERATION: TPCANStatus = 0x8000000;

// ---- Parameters ----

pub const PCAN_DEVICE_ID: TPCANParameter = 0x01;
pub const PCAN_5VOLTS_POWER: TPCANParameter = 0x02;
pub const PCAN_RECEIVE_EVENT: TPCANParameter = 0x03;
pub const PCAN_MESSAGE_FILTER: TPCANParameter = 0x04;
pub const PCAN_API_VERSION: TPCANParameter = 0x05;
pub const PCAN_CHANNEL_VERSION: TPCANParameter = 0x06;
pub const PCAN_BUSOFF_AUTORESET: TPCANParameter = 0x07;
pub const PCAN_LISTEN_ONLY: TPCANParameter = 0x08;
pub const PCAN_CHANNEL_CONDITION: TPCANParameter = 0x0D;
pub const PCAN_HARDWARE_NAME: TPCANParameter = 0x0E;
pub const PCAN_RECEIVE_STATUS: TPCANParameter = 0x0F;
pub const PCAN_CONTROLLER_NUMBER: TPCANParameter = 0x10;
pub const PCAN_CHANNEL_FEATURES: TPCANParameter = 0x16;
pub const PCAN_BITRATE_ADAPTING: TPCANParameter = 0x17;
pub const PCAN_BITRATE_INFO: TPCANParameter = 0x18;
pub const PCAN_BITRATE_INFO_FD: TPCANParameter = 0x19;
pub const PCAN_BUSSPEED_NOMINAL: TPCANParameter = 0x1A;
pub const PCAN_BUSSPEED_FD: TPCANParameter = 0x1B;
pub const PCAN_ALLOW_STATUS_FRAMES: TPCANParameter = 0x1E;
pub const PCAN_ALLOW_RTR_FRAMES: TPCANParameter = 0x1F;
pub const PCAN_ALLOW_ERROR_FRAMES: TPCANParameter = 0x20;
pub const PCAN_ACCEPTANCE_FILTER_11BIT: TPCANParameter = 0x22;
pub const PCAN_ACCEPTANCE_FILTER_29BIT: TPCANParameter = 0x23;
pub const PCAN_FIRMWARE_VERSION: TPCANParameter = 0x29;
pub const PCAN_ATTACHED_CHANNELS_COUNT: TPCANParameter = 0x2A;
pub const PCAN_ATTACHED_CHANNELS: TPCANParameter = 0x2B;
pub const PCAN_ALLOW_ECHO_FRAMES: TPCANParameter = 0x2C;
pub const PCAN_DEVICE_PART_NUMBER: TPCANParameter = 0x2D;

// ---- Parameter values ----

pub const PCAN_PARAMETER_OFF: u32 = 0x00;
pub const PCAN_PARAMETER_ON: u32 = 0x01;
pub const PCAN_FILTER_CLOSE: u32 = 0x00;
pub const PCAN_FILTER_OPEN: u32 = 0x01;
pub const PCAN_FILTER_CUSTOM: u32 = 0x02;
pub const PCAN_CHANNEL_UNAVAILABLE: u32 = 0x00;
pub const PCAN_CHANNEL_AVAILABLE: u32 = 0x01;
pub const PCAN_CHANNEL_OCCUPIED: u32 = 0x02;

// ---- Feature flags ----

pub const FEATURE_FD_CAPABLE: u32 = 0x01;
pub const FEATURE_DELAY_CAPABLE: u32 = 0x02;
pub const FEATURE_IO_CAPABLE: u32 = 0x04;
pub const FEATURE_XL_CAPABLE: u32 = 0x08;

// ---- PCAN devices ----

pub const PCAN_NONE: TPCANDevice = 0x00;
pub const PCAN_PCI: TPCANDevice = 0x04;
pub const PCAN_USB: TPCANDevice = 0x05;
pub const PCAN_LAN: TPCANDevice = 0x08;

// ---- Baud rates (BTR0/BTR1) ----

pub const PCAN_BAUD_1M: TPCANBaudrate = 0x0014;
pub const PCAN_BAUD_800K: TPCANBaudrate = 0x0016;
pub const PCAN_BAUD_500K: TPCANBaudrate = 0x001C;
pub const PCAN_BAUD_250K: TPCANBaudrate = 0x011C;
pub const PCAN_BAUD_125K: TPCANBaudrate = 0x031C;
pub const PCAN_BAUD_100K: TPCANBaudrate = 0x432F;
pub const PCAN_BAUD_95K: TPCANBaudrate = 0xC34E;
pub const PCAN_BAUD_83K: TPCANBaudrate = 0x852B;
pub const PCAN_BAUD_50K: TPCANBaudrate = 0x472F;
pub const PCAN_BAUD_47K: TPCANBaudrate = 0x1414;
pub const PCAN_BAUD_33K: TPCANBaudrate = 0x8B2F;
pub const PCAN_BAUD_20K: TPCANBaudrate = 0x532F;
pub const PCAN_BAUD_10K: TPCANBaudrate = 0x672F;
pub const PCAN_BAUD_5K: TPCANBaudrate = 0x7F7F;

// ---- Message types ----

pub const PCAN_MESSAGE_STANDARD: TPCANMessageType = 0x00;
pub const PCAN_MESSAGE_RTR: TPCANMessageType = 0x01;
pub const PCAN_MESSAGE_EXTENDED: TPCANMessageType = 0x02;
pub const PCAN_MESSAGE_FD: TPCANMessageType = 0x04;
pub const PCAN_MESSAGE_BRS: TPCANMessageType = 0x08;
pub const PCAN_MESSAGE_ESI: TPCANMessageType = 0x10;
pub const PCAN_MESSAGE_ECHO: TPCANMessageType = 0x20;
pub const PCAN_MESSAGE_ERRFRAME: TPCANMessageType = 0x40;
pub const PCAN_MESSAGE_STATUS: TPCANMessageType = 0x80;

// ---- Mode (used by CAN_FilterMessages) ----

pub const PCAN_MODE_STANDARD: TPCANMode = 0x00;
pub const PCAN_MODE_EXTENDED: TPCANMode = 0x02;

// ---- Other limits ----

pub const MAX_LENGTH_HARDWARE_NAME: usize = 33;
pub const MAX_LENGTH_VERSION_STRING: usize = 256;
pub const MAX_VALUE_STANDARD_ID: u32 = 0x7FF;
pub const MAX_VALUE_EXTENDED_ID: u32 = 0x1FFFFFFF;

// ---- Structures ----

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TPCANMsg {
    pub ID: u32,
    pub MSGTYPE: TPCANMessageType,
    pub LEN: u8,
    pub DATA: [u8; 8],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TPCANTimestamp {
    pub millis: u32,
    pub millis_overflow: u16,
    pub micros: u16,
}

impl TPCANTimestamp {
    /// Total microseconds since boot.
    pub fn total_micros(&self) -> u64 {
        (self.micros as u64)
            + 1000u64 * (self.millis as u64)
            + 0x100000000u64 * 1000u64 * (self.millis_overflow as u64)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TPCANMsgFD {
    pub ID: u32,
    pub MSGTYPE: TPCANMessageType,
    pub DLC: u8,
    pub DATA: [u8; 64],
}

impl Default for TPCANMsgFD {
    fn default() -> Self {
        Self {
            ID: 0,
            MSGTYPE: 0,
            DLC: 0,
            DATA: [0; 64],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TPCANChannelInformation {
    pub channel_handle: TPCANHandle,
    pub device_type: TPCANDevice,
    pub controller_number: u8,
    pub device_features: u32,
    pub device_name: [u8; MAX_LENGTH_HARDWARE_NAME],
    pub device_id: u32,
    pub channel_condition: u32,
}

impl Default for TPCANChannelInformation {
    fn default() -> Self {
        Self {
            channel_handle: 0,
            device_type: 0,
            controller_number: 0,
            device_features: 0,
            device_name: [0; MAX_LENGTH_HARDWARE_NAME],
            device_id: 0,
            channel_condition: 0,
        }
    }
}

// ---- Loaded library ----

/// Resolved function pointers from the loaded PCAN-Basic shared library.
///
/// On Windows the API uses `__stdcall`, so everything is declared
/// `extern "system"` (stdcall on 32-bit Windows, the C ABI everywhere else).
pub struct PcanLib {
    _lib: libloading::Library,
    pub CAN_Initialize: unsafe extern "system" fn(
        channel: TPCANHandle,
        btr0btr1: TPCANBaudrate,
        hw_type: u8,
        io_port: u32,
        interrupt: u16,
    ) -> TPCANStatus,
    pub CAN_InitializeFD:
        unsafe extern "system" fn(channel: TPCANHandle, bitrate_fd: TPCANBitrateFD) -> TPCANStatus,
    pub CAN_Uninitialize: unsafe extern "system" fn(channel: TPCANHandle) -> TPCANStatus,
    pub CAN_Reset: unsafe extern "system" fn(channel: TPCANHandle) -> TPCANStatus,
    pub CAN_GetStatus: unsafe extern "system" fn(channel: TPCANHandle) -> TPCANStatus,
    pub CAN_Read: unsafe extern "system" fn(
        channel: TPCANHandle,
        message: *mut TPCANMsg,
        timestamp: *mut TPCANTimestamp,
    ) -> TPCANStatus,
    pub CAN_ReadFD: unsafe extern "system" fn(
        channel: TPCANHandle,
        message: *mut TPCANMsgFD,
        timestamp: *mut TPCANTimestampFD,
    ) -> TPCANStatus,
    pub CAN_Write:
        unsafe extern "system" fn(channel: TPCANHandle, message: *mut TPCANMsg) -> TPCANStatus,
    pub CAN_WriteFD:
        unsafe extern "system" fn(channel: TPCANHandle, message: *mut TPCANMsgFD) -> TPCANStatus,
    pub CAN_FilterMessages: unsafe extern "system" fn(
        channel: TPCANHandle,
        from_id: u32,
        to_id: u32,
        mode: TPCANMode,
    ) -> TPCANStatus,
    pub CAN_GetValue: unsafe extern "system" fn(
        channel: TPCANHandle,
        parameter: TPCANParameter,
        buffer: *mut std::os::raw::c_void,
        buffer_length: u32,
    ) -> TPCANStatus,
    pub CAN_SetValue: unsafe extern "system" fn(
        channel: TPCANHandle,
        parameter: TPCANParameter,
        buffer: *mut std::os::raw::c_void,
        buffer_length: u32,
    ) -> TPCANStatus,
    pub CAN_GetErrorText: unsafe extern "system" fn(
        error: TPCANStatus,
        language: u16,
        buffer: *mut std::os::raw::c_char,
    ) -> TPCANStatus,
    pub CAN_LookUpChannel: unsafe extern "system" fn(
        parameters: *mut std::os::raw::c_char,
        found_channel: *mut TPCANHandle,
    ) -> TPCANStatus,
}

// SAFETY: PCAN-Basic functions are thread-safe per PEAK documentation.
unsafe impl Send for PcanLib {}
unsafe impl Sync for PcanLib {}

#[cfg(target_os = "windows")]
const LIB_NAME: &str = "PCANBasic.dll";

// PEAK only ships Linux and Windows libraries. Other platforms still
// compile; loading simply fails at runtime with `LoadError::LibraryNotFound`.
#[cfg(not(target_os = "windows"))]
const LIB_NAME: &str = "libpcanbasic.so";

#[derive(Debug)]
pub enum LoadError {
    LibraryNotFound(libloading::Error),
    SymbolNotFound(&'static str, libloading::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::LibraryNotFound(e) => write!(
                f,
                "Failed to load {}: {}. Install PCAN-Basic or set PCAN_SDK_DIR.",
                LIB_NAME, e
            ),
            LoadError::SymbolNotFound(name, e) => {
                write!(f, "Symbol '{}' not found in {}: {}", name, LIB_NAME, e)
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::LibraryNotFound(e) | LoadError::SymbolNotFound(_, e) => Some(e),
        }
    }
}

macro_rules! load_sym {
    ($lib:expr, $name:literal) => {{
        // The symbol type is inferred from the struct field it initialises, so
        // the declared signature is the single source of truth.
        // SAFETY: the field signatures mirror the PCAN-Basic headers.
        *$lib
            .get($name.as_bytes())
            .map_err(|e| LoadError::SymbolNotFound($name, e))?
    }};
}

impl PcanLib {
    /// Load the PCAN-Basic shared library and resolve all function symbols.
    ///
    /// # Safety
    ///
    /// The loaded library must be a genuine PCAN-Basic implementation
    /// with the expected ABI.
    pub unsafe fn load() -> Result<Self, LoadError> {
        let lib = if let Ok(dir) = std::env::var("PCAN_SDK_DIR") {
            let mut path = std::path::PathBuf::from(&dir);
            if cfg!(target_os = "windows") && cfg!(target_pointer_width = "64") {
                path.push("x64");
            } else if cfg!(target_os = "windows") {
                path.push("x86");
            }
            path.push(LIB_NAME);
            libloading::Library::new(&path)
                .or_else(|_| libloading::Library::new(LIB_NAME))
                .map_err(LoadError::LibraryNotFound)?
        } else {
            libloading::Library::new(LIB_NAME).map_err(LoadError::LibraryNotFound)?
        };

        Ok(Self {
            CAN_Initialize: load_sym!(lib, "CAN_Initialize"),
            CAN_InitializeFD: load_sym!(lib, "CAN_InitializeFD"),
            CAN_Uninitialize: load_sym!(lib, "CAN_Uninitialize"),
            CAN_Reset: load_sym!(lib, "CAN_Reset"),
            CAN_GetStatus: load_sym!(lib, "CAN_GetStatus"),
            CAN_Read: load_sym!(lib, "CAN_Read"),
            CAN_ReadFD: load_sym!(lib, "CAN_ReadFD"),
            CAN_Write: load_sym!(lib, "CAN_Write"),
            CAN_WriteFD: load_sym!(lib, "CAN_WriteFD"),
            CAN_FilterMessages: load_sym!(lib, "CAN_FilterMessages"),
            CAN_GetValue: load_sym!(lib, "CAN_GetValue"),
            CAN_SetValue: load_sym!(lib, "CAN_SetValue"),
            CAN_GetErrorText: load_sym!(lib, "CAN_GetErrorText"),
            CAN_LookUpChannel: load_sym!(lib, "CAN_LookUpChannel"),
            _lib: lib,
        })
    }
}

static PCAN: OnceLock<PcanLib> = OnceLock::new();

/// Get a reference to the globally loaded PCAN-Basic instance.
///
/// The library is loaded on the first call. Returns an error if the
/// shared library cannot be found or a required symbol is missing.
pub fn get() -> Result<&'static PcanLib, &'static LoadError> {
    static LOAD_ERROR: OnceLock<LoadError> = OnceLock::new();

    if let Some(lib) = PCAN.get() {
        return Ok(lib);
    }

    match unsafe { PcanLib::load() } {
        Ok(lib) => {
            let _ = PCAN.set(lib);
            Ok(PCAN.get().unwrap())
        }
        Err(e) => {
            let _ = LOAD_ERROR.set(e);
            Err(LOAD_ERROR.get().unwrap())
        }
    }
}
