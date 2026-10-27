//! The Vulkan loader's private layer ABI from `vulkan/vk_layer.h`.
//! These structures are not part of Vulkan's public API, so ash does not define them.

use ash::vk;
use std::ffi::{c_char, c_void};

pub const INSTANCE_CREATE_INFO: i32 = 47;
pub const DEVICE_CREATE_INFO: i32 = 48;
pub const LINK_INFO: u32 = 0;
pub const INTERFACE_VERSION: u32 = 2;

#[repr(C)]
pub struct NegotiateLayerInterface {
    pub s_type: u32,
    pub p_next: *mut c_void,
    pub loader_layer_interface_version: u32,
    pub get_instance_proc_addr: Option<vk::PFN_vkGetInstanceProcAddr>,
    pub get_device_proc_addr: Option<vk::PFN_vkGetDeviceProcAddr>,
    pub get_physical_device_proc_addr:
        Option<unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction>,
}

#[repr(C)]
pub struct InstanceLink {
    pub p_next: *mut InstanceLink,
    pub next_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub _next_get_physical_device_proc_addr: vk::PFN_vkVoidFunction,
}

#[repr(C)]
pub struct DeviceLink {
    pub p_next: *mut DeviceLink,
    pub next_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub next_get_device_proc_addr: vk::PFN_vkGetDeviceProcAddr,
}

#[repr(C)]
pub struct InstanceCreateInfo {
    pub s_type: vk::StructureType,
    pub p_next: *const c_void,
    pub function: u32,
    pub layer_info: *mut InstanceLink,
}

#[repr(C)]
pub struct DeviceCreateInfo {
    pub s_type: vk::StructureType,
    pub p_next: *const c_void,
    pub function: u32,
    pub layer_info: *mut DeviceLink,
}

#[repr(C)]
pub struct BaseInStructure {
    pub s_type: vk::StructureType,
    pub p_next: *const BaseInStructure,
}

pub unsafe fn instance_link(info: *const vk::InstanceCreateInfo<'_>) -> Option<*mut InstanceLink> {
    let mut node = unsafe { (*info).p_next.cast::<BaseInStructure>() };
    while !node.is_null() {
        let current = unsafe { &*node };
        if current.s_type.as_raw() == INSTANCE_CREATE_INFO {
            let loader = unsafe { &mut *node.cast_mut().cast::<InstanceCreateInfo>() };
            if loader.function == LINK_INFO {
                return Some(loader.layer_info);
            }
        }
        node = current.p_next;
    }
    None
}

pub unsafe fn device_link(info: *const vk::DeviceCreateInfo<'_>) -> Option<*mut DeviceLink> {
    let mut node = unsafe { (*info).p_next.cast::<BaseInStructure>() };
    while !node.is_null() {
        let current = unsafe { &*node };
        if current.s_type.as_raw() == DEVICE_CREATE_INFO {
            let loader = unsafe { &mut *node.cast_mut().cast::<DeviceCreateInfo>() };
            if loader.function == LINK_INFO {
                return Some(loader.layer_info);
            }
        }
        node = current.p_next;
    }
    None
}

/// Change the loader-provided chain pointer to the next layer before forwarding.
pub unsafe fn advance_instance_chain(
    info: *const vk::InstanceCreateInfo<'_>,
    next: *mut InstanceLink,
) {
    let mut node = unsafe { (*info).p_next.cast::<BaseInStructure>() };
    while !node.is_null() {
        if unsafe { (*node).s_type.as_raw() == INSTANCE_CREATE_INFO } {
            unsafe { (*node.cast_mut().cast::<InstanceCreateInfo>()).layer_info = next };
            return;
        }
        node = unsafe { (*node).p_next };
    }
}

pub unsafe fn advance_device_chain(info: *const vk::DeviceCreateInfo<'_>, next: *mut DeviceLink) {
    let mut node = unsafe { (*info).p_next.cast::<BaseInStructure>() };
    while !node.is_null() {
        if unsafe { (*node).s_type.as_raw() == DEVICE_CREATE_INFO } {
            unsafe { (*node.cast_mut().cast::<DeviceCreateInfo>()).layer_info = next };
            return;
        }
        node = unsafe { (*node).p_next };
    }
}
