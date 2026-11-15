use std::ffi::CStr;
use std::mem::size_of;

use hidapi::HidApi;
use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, SetupDiCreateDeviceInfoList, SetupDiDestroyDeviceInfoList,
    SetupDiGetDeviceInterfaceDetailW, SetupDiOpenDeviceInterfaceW, CR_SUCCESS, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVINFO_DATA,
};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_ContainerId;
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_ContainerId, DEVPROPTYPE, DEVPROP_TYPE_GUID,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, RPC_E_CHANGED_MODE};
use windows::Win32::Media::Audio::{
    eCapture, eConsole, IMMDevice, IMMDeviceEnumerator, IMMEndpoint, MMDeviceEnumerator,
    DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToGUID};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};

use super::protocol;

/// Compare Windows device containers, never user-editable display names.
///
/// This identifies the audio/HID relationship only. The HID service separately
/// verifies the supported model and the button mode before enabling commands.
pub(super) fn selected_input_matches_speechmike(mic_id: Option<&str>) -> Result<bool, ()> {
    let api = HidApi::new().map_err(|_| ())?;
    let mut paths: Vec<_> = api
        .device_list()
        .filter(|device| {
            device.vendor_id() == protocol::VENDOR_ID
                && device.product_id() == protocol::PRODUCT_ID
                && device.usage_page() == protocol::USAGE_PAGE
                && device.usage() == protocol::USAGE
        })
        .map(|device| device.path().to_owned())
        .collect();
    paths.sort();
    paths.dedup();

    let path = match paths.as_slice() {
        [] => return Ok(false),
        [path] => path,
        _ => return Err(()),
    };

    let _apartment = ComApartment::enter()?;
    let audio = selected_input(mic_id)?;
    let audio_container = audio_container(&audio)?;
    let hid_container = hid_container(path)?;
    Ok(containers_match(audio_container, hid_container))
}

fn selected_input(mic_id: Option<&str>) -> Result<IMMDevice, ()> {
    use windows::core::Interface;

    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(|_| ())?;
    let device = match mic_id {
        Some(value) => {
            let endpoint = native_endpoint_id(value)?;
            let endpoint = wide_string(&endpoint)?;
            unsafe { enumerator.GetDevice(PCWSTR(endpoint.as_ptr())) }.map_err(|_| ())?
        }
        // Same endpoint role as CPAL's default_input_device().
        None => {
            unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) }.map_err(|_| ())?
        }
    };

    if unsafe { device.GetState() }.map_err(|_| ())? != DEVICE_STATE_ACTIVE {
        return Err(());
    }
    let endpoint: IMMEndpoint = device.cast().map_err(|_| ())?;
    if unsafe { endpoint.GetDataFlow() }.map_err(|_| ())? != eCapture {
        return Err(());
    }
    Ok(device)
}

fn native_endpoint_id(value: &str) -> Result<String, ()> {
    let id = value.parse::<cpal::DeviceId>().map_err(|_| ())?;
    match id.0 {
        cpal::HostId::Wasapi if !id.1.is_empty() => Ok(id.1),
        _ => Err(()),
    }
}

fn audio_container(device: &IMMDevice) -> Result<GUID, ()> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.map_err(|_| ())?;
    let mut value = unsafe { store.GetValue(&PKEY_Device_ContainerId) }.map_err(|_| ())?;
    let container = unsafe { PropVariantToGUID(&value) }.map_err(|_| ());
    // GetValue owns allocated property data, including on a failed conversion.
    let _ = unsafe { PropVariantClear(&mut value) };
    container
}

fn hid_container(path: &CStr) -> Result<GUID, ()> {
    let path = wide_string(path.to_str().map_err(|_| ())?)?;
    let devices =
        DeviceInfoSet(unsafe { SetupDiCreateDeviceInfoList(None, None) }.map_err(|_| ())?);
    let mut interface = SP_DEVICE_INTERFACE_DATA {
        cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    unsafe {
        SetupDiOpenDeviceInterfaceW(devices.0, PCWSTR(path.as_ptr()), 0, Some(&mut interface))
    }
    .map_err(|_| ())?;

    let mut device = SP_DEVINFO_DATA {
        cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };
    // Microsoft documents that the devnode is filled even when the optional
    // path buffer is omitted and ERROR_INSUFFICIENT_BUFFER is returned.
    // https://learn.microsoft.com/windows/win32/api/setupapi/nf-setupapi-setupdigetdeviceinterfacedetailw
    match unsafe {
        SetupDiGetDeviceInterfaceDetailW(devices.0, &interface, None, 0, None, Some(&mut device))
    } {
        Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {}
        _ => return Err(()),
    }
    if device.DevInst == 0 {
        return Err(());
    }

    let mut container = GUID::default();
    let mut property_type = DEVPROPTYPE::default();
    let mut size = size_of::<GUID>() as u32;
    // The typed GUID allocation has the correct alignment and exact buffer size.
    let result = unsafe {
        CM_Get_DevNode_PropertyW(
            device.DevInst,
            &DEVPKEY_Device_ContainerId,
            &mut property_type,
            Some((&mut container as *mut GUID).cast::<u8>()),
            &mut size,
            0,
        )
    };
    match (result, property_type, size) {
        (CR_SUCCESS, DEVPROP_TYPE_GUID, size) if size == size_of::<GUID>() as u32 => Ok(container),
        _ => Err(()),
    }
}

fn containers_match(audio: GUID, hid: GUID) -> bool {
    audio != GUID::default() && audio == hid
}

fn wide_string(value: &str) -> Result<Vec<u16>, ()> {
    if value.is_empty() || value.contains('\0') {
        return Err(());
    }
    Ok(value.encode_utf16().chain(Some(0)).collect())
}

struct DeviceInfoSet(HDEVINFO);

impl Drop for DeviceInfoSet {
    fn drop(&mut self) {
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

struct ComApartment {
    uninitialize: bool,
}

impl ComApartment {
    fn enter() -> Result<Self, ()> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        match (result.is_ok(), result == RPC_E_CHANGED_MODE) {
            (true, _) => Ok(Self { uninitialize: true }),
            (false, true) => Ok(Self {
                uninitialize: false,
            }),
            _ => Err(()),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_containers_never_confirm_a_match() {
        assert!(!containers_match(GUID::default(), GUID::default()));
    }

    #[test]
    fn only_equal_nonempty_containers_match() {
        let device = GUID::from_u128(1);
        assert!(containers_match(device, device));
        assert!(!containers_match(device, GUID::from_u128(2)));
        assert!(!containers_match(device, GUID::default()));
    }

    #[test]
    fn endpoint_uses_the_cpal_parser_without_display_names() {
        assert_eq!(
            native_endpoint_id("wasapi:synthetic-endpoint"),
            Ok("synthetic-endpoint".to_string())
        );
        assert!(native_endpoint_id("SpeechMike").is_err());
        assert!(native_endpoint_id("wasapi:").is_err());
    }

    #[test]
    fn windows_strings_reject_embedded_terminators() {
        assert!(wide_string("").is_err());
        assert!(wide_string("synthetic\0endpoint").is_err());
        assert_eq!(wide_string("test"), Ok(vec![116, 101, 115, 116, 0]));
    }
}
