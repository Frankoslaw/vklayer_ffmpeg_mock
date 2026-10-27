#![allow(non_snake_case)] // Vulkan entry-point and cdylib names follow the Vulkan ABI.

use ash::vk;
use std::ffi::CStr;
use std::os::raw::c_char;

const LAYER_INTERFACE_VERSION: u32 = 2;

// Loader-private ABI from `vulkan/vk_layer.h`; ash only generates the Vulkan API.
#[repr(C)]
struct NegotiateLayerInterface {
    s_type: u32,
    p_next: *mut std::ffi::c_void,
    loader_layer_interface_version: u32,
    get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    get_device_proc_addr: vk::PFN_vkGetDeviceProcAddr,
    get_physical_device_proc_addr:
        Option<unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction>,
}

/// Reports permissive H.264 encode limits for drivers that omit this query.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetPhysicalDeviceVideoCapabilitiesKHR(
    _physical_device: vk::PhysicalDevice,
    _video_profile: *const vk::VideoProfileInfoKHR<'_>,
    capabilities: *mut vk::VideoCapabilitiesKHR<'_>,
) -> vk::Result {
    if capabilities.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    // Keep the caller-provided pNext chain intact; populate only core fields.
    unsafe {
        (*capabilities).min_bitstream_buffer_offset_alignment = 256;
        (*capabilities).min_bitstream_buffer_size_alignment = 256;
        (*capabilities).max_coded_extent = vk::Extent2D {
            width: 3840,
            height: 2160,
        };
        (*capabilities).min_coded_extent = vk::Extent2D {
            width: 64,
            height: 64,
        };
        (*capabilities).max_dpb_slots = 1;
        (*capabilities).max_active_reference_pictures = 1;
    }
    vk::Result::SUCCESS
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    _instance: vk::Instance,
    p_name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    if p_name.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(p_name) };
    if name.to_bytes() == b"vkGetInstanceProcAddr" {
        return Some(unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction,
                unsafe extern "system" fn(),
            >(vkGetInstanceProcAddr)
        });
    }
    if name.to_bytes() == b"vkGetPhysicalDeviceVideoCapabilitiesKHR" {
        return Some(unsafe {
            std::mem::transmute::<
                unsafe extern "system" fn(
                    vk::PhysicalDevice,
                    *const vk::VideoProfileInfoKHR<'_>,
                    *mut vk::VideoCapabilitiesKHR<'_>,
                ) -> vk::Result,
                unsafe extern "system" fn(),
            >(vkGetPhysicalDeviceVideoCapabilitiesKHR)
        });
    }
    None
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetDeviceProcAddr(
    _device: vk::Device,
    _p_name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    None
}

/// Vulkan loader layer entry point. This layer currently exposes its mock
/// query directly; the loader supplies dispatch for all other entry points.
#[unsafe(no_mangle)]
unsafe extern "system" fn vkNegotiateLoaderLayerInterfaceVersion(
    interface: *mut NegotiateLayerInterface,
) -> vk::Result {
    if interface.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let interface = unsafe { &mut *interface };
    interface.loader_layer_interface_version = interface
        .loader_layer_interface_version
        .min(LAYER_INTERFACE_VERSION);
    interface.get_instance_proc_addr = vkGetInstanceProcAddr;
    interface.get_device_proc_addr = vkGetDeviceProcAddr;
    interface.get_physical_device_proc_addr = None;
    vk::Result::SUCCESS
}
