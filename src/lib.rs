#![allow(non_snake_case)] // Vulkan exports and the cdylib name follow Vulkan conventions.

mod loader_abi;
mod mock_h264;
mod tracing;

use ash::vk::{self, Handle};
use loader_abi as abi;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::sync::{OnceLock, RwLock};

type VideoCapabilitiesFn = for<'profile, 'capabilities> unsafe extern "system" fn(
    vk::PhysicalDevice,
    *const vk::VideoProfileInfoKHR<'profile>,
    *mut vk::VideoCapabilitiesKHR<'capabilities>,
) -> vk::Result;

#[derive(Clone, Copy)]
struct InstanceDispatch {
    gipa: vk::PFN_vkGetInstanceProcAddr,
}
#[derive(Clone, Copy)]
struct DeviceDispatch {
    gdpa: vk::PFN_vkGetDeviceProcAddr,
}

static INSTANCES: OnceLock<RwLock<HashMap<u64, InstanceDispatch>>> = OnceLock::new();
static DEVICES: OnceLock<RwLock<HashMap<u64, DeviceDispatch>>> = OnceLock::new();
static PHYSICAL_DEVICES: OnceLock<RwLock<HashMap<u64, u64>>> = OnceLock::new();
static NATIVE_DEVICE_EXTENSIONS: OnceLock<RwLock<HashMap<u64, Vec<vk::ExtensionProperties>>>> =
    OnceLock::new();

fn instances() -> &'static RwLock<HashMap<u64, InstanceDispatch>> {
    INSTANCES.get_or_init(Default::default)
}
fn devices() -> &'static RwLock<HashMap<u64, DeviceDispatch>> {
    DEVICES.get_or_init(Default::default)
}
fn physical_devices() -> &'static RwLock<HashMap<u64, u64>> {
    PHYSICAL_DEVICES.get_or_init(Default::default)
}
fn native_device_extensions() -> &'static RwLock<HashMap<u64, Vec<vk::ExtensionProperties>>> {
    NATIVE_DEVICE_EXTENSIONS.get_or_init(Default::default)
}

fn instance_for_physical(physical: vk::PhysicalDevice) -> Option<(u64, InstanceDispatch)> {
    let instance_raw = physical_devices()
        .read()
        .ok()?
        .get(&physical.as_raw())
        .copied()?;
    let dispatch = instances().read().ok()?.get(&instance_raw).copied()?;
    Some((instance_raw, dispatch))
}

fn proc_name(name: *const c_char) -> Option<&'static CStr> {
    if name.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(name) })
    }
}

fn erase_fn<T: Copy>(function: T) -> vk::PFN_vkVoidFunction {
    assert_eq!(
        std::mem::size_of::<T>(),
        std::mem::size_of::<unsafe extern "system" fn()>()
    );
    Some(unsafe { std::mem::transmute_copy(&function) })
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkNegotiateLoaderLayerInterfaceVersion(
    version: *mut abi::NegotiateLayerInterface,
) -> vk::Result {
    tracing::init();
    if version.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let version = unsafe { &mut *version };
    version.loader_layer_interface_version = version
        .loader_layer_interface_version
        .min(abi::INTERFACE_VERSION);
    version.get_instance_proc_addr = Some(vkGetInstanceProcAddr);
    version.get_device_proc_addr = Some(vkGetDeviceProcAddr);
    version.get_physical_device_proc_addr = None;
    info!(
        "negotiated Vulkan loader layer interface v{}",
        version.loader_layer_interface_version
    );
    vk::Result::SUCCESS
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    tracing::init();
    let Some(name) = proc_name(name) else {
        return None;
    };
    match name.to_bytes() {
        b"vkNegotiateLoaderLayerInterfaceVersion" => {
            return erase_fn(
                vkNegotiateLoaderLayerInterfaceVersion
                    as unsafe extern "system" fn(*mut abi::NegotiateLayerInterface) -> vk::Result,
            );
        }
        b"vkGetInstanceProcAddr" => {
            return erase_fn(vkGetInstanceProcAddr as vk::PFN_vkGetInstanceProcAddr);
        }
        b"vkGetDeviceProcAddr" => {
            return erase_fn(vkGetDeviceProcAddr as vk::PFN_vkGetDeviceProcAddr);
        }
        b"vkCreateInstance" => return erase_fn(vkCreateInstance as vk::PFN_vkCreateInstance),
        b"vkCreateDevice" => return erase_fn(vkCreateDevice as vk::PFN_vkCreateDevice),
        b"vkDestroyInstance" => return erase_fn(vkDestroyInstance as vk::PFN_vkDestroyInstance),
        b"vkDestroyDevice" => return erase_fn(vkDestroyDevice as vk::PFN_vkDestroyDevice),
        b"vkEnumeratePhysicalDevices" => {
            return erase_fn(vkEnumeratePhysicalDevices as vk::PFN_vkEnumeratePhysicalDevices);
        }
        b"vkEnumerateDeviceExtensionProperties" => {
            return erase_fn(
                vkEnumerateDeviceExtensionProperties
                    as vk::PFN_vkEnumerateDeviceExtensionProperties,
            );
        }
        b"vkGetPhysicalDeviceQueueFamilyProperties" => {
            return erase_fn(
                vkGetPhysicalDeviceQueueFamilyProperties
                    as vk::PFN_vkGetPhysicalDeviceQueueFamilyProperties,
            );
        }
        b"vkGetPhysicalDeviceVideoCapabilitiesKHR" => {
            return erase_fn(vkGetPhysicalDeviceVideoCapabilitiesKHR as VideoCapabilitiesFn);
        }
        _ => {}
    }
    if instance.is_null() {
        return None;
    }
    let dispatch = instances().read().ok()?.get(&instance.as_raw()).copied()?;
    debug!("vkGetInstanceProcAddr({})", name.to_string_lossy());
    let started = std::time::Instant::now();
    let result = unsafe { (dispatch.gipa)(instance, name.as_ptr()) };
    debug!(
        "vkGetInstanceProcAddr -> {:?} ({:?})",
        result,
        started.elapsed()
    );
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetDeviceProcAddr(
    device: vk::Device,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    tracing::init();
    let Some(name) = proc_name(name) else {
        return None;
    };
    if name.to_bytes() == b"vkGetDeviceProcAddr" {
        return erase_fn(vkGetDeviceProcAddr as vk::PFN_vkGetDeviceProcAddr);
    }
    if name.to_bytes() == b"vkDestroyDevice" {
        return erase_fn(vkDestroyDevice as vk::PFN_vkDestroyDevice);
    }
    if device.is_null() {
        return None;
    }
    let dispatch = devices().read().ok()?.get(&device.as_raw()).copied()?;
    debug!("vkGetDeviceProcAddr({})", name.to_string_lossy());
    let started = std::time::Instant::now();
    let result = unsafe { (dispatch.gdpa)(device, name.as_ptr()) };
    debug!(
        "vkGetDeviceProcAddr -> {:?} ({:?})",
        result,
        started.elapsed()
    );
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkCreateInstance(
    create_info: *const vk::InstanceCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    instance_out: *mut vk::Instance,
) -> vk::Result {
    tracing::init();
    if create_info.is_null() || instance_out.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let Some(link_ptr) = (unsafe { abi::instance_link(create_info) }) else {
        warn!("vkCreateInstance received no loader link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let link = unsafe { &mut *link_ptr };
    let next_gipa = link.next_get_instance_proc_addr;
    let next_link = link.p_next;
    let Some(create_fn) =
        (unsafe { next_gipa(vk::Instance::null(), c"vkCreateInstance".as_ptr()) })
    else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let next_create: vk::PFN_vkCreateInstance = unsafe { std::mem::transmute(create_fn) };
    unsafe { abi::advance_instance_chain(create_info, next_link) };
    let result = crate::trace_call!("vkCreateInstance", "create_info", unsafe {
        next_create(create_info, allocator, instance_out)
    });
    if result == vk::Result::SUCCESS {
        instances().write().unwrap().insert(
            unsafe { (*instance_out).as_raw() },
            InstanceDispatch { gipa: next_gipa },
        );
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkCreateDevice(
    physical_device: vk::PhysicalDevice,
    create_info: *const vk::DeviceCreateInfo<'_>,
    allocator: *const vk::AllocationCallbacks<'_>,
    device_out: *mut vk::Device,
) -> vk::Result {
    tracing::init();
    if create_info.is_null() || device_out.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let Some(link_ptr) = (unsafe { abi::device_link(create_info) }) else {
        warn!("vkCreateDevice received no loader link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let link = unsafe { &mut *link_ptr };
    let next_gipa = link.next_get_instance_proc_addr;
    let next_gdpa = link.next_get_device_proc_addr;
    let Some(instance_raw) = physical_devices()
        .read()
        .ok()
        .and_then(|map| map.get(&physical_device.as_raw()).copied())
    else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let instance = vk::Instance::from_raw(instance_raw);
    let Some(create_fn) = (unsafe { next_gipa(instance, c"vkCreateDevice".as_ptr()) }) else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let next_create: vk::PFN_vkCreateDevice = unsafe { std::mem::transmute(create_fn) };

    // The synthetic video extension names must not reach an ICD that does not
    // implement them. Keep real extensions intact and remove only names this
    // layer invented for the capability probe.
    let mut native_extensions = Vec::new();
    if let Some(enumerate_fn) =
        unsafe { next_gipa(instance, c"vkEnumerateDeviceExtensionProperties".as_ptr()) }
    {
        let enumerate: vk::PFN_vkEnumerateDeviceExtensionProperties =
            unsafe { std::mem::transmute(enumerate_fn) };
        let mut count = 0;
        let result = unsafe {
            enumerate(
                physical_device,
                std::ptr::null(),
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if result == vk::Result::SUCCESS || result == vk::Result::INCOMPLETE {
            native_extensions.resize(count as usize, vk::ExtensionProperties::default());
            if count > 0 {
                let result = unsafe {
                    enumerate(
                        physical_device,
                        std::ptr::null(),
                        &mut count,
                        native_extensions.as_mut_ptr(),
                    )
                };
                if result == vk::Result::SUCCESS || result == vk::Result::INCOMPLETE {
                    native_extensions.truncate(count as usize);
                } else {
                    native_extensions.clear();
                }
            }
        }
    }
    let enabled_names: Vec<*const c_char> =
        if unsafe { (*create_info).enabled_extension_count } == 0 {
            Vec::new()
        } else {
            let names = unsafe {
                std::slice::from_raw_parts(
                    (*create_info).pp_enabled_extension_names,
                    (*create_info).enabled_extension_count as usize,
                )
            };
            names
                .iter()
                .copied()
                .filter(|name| {
                    !name.is_null()
                        && !mock_h264::should_strip_extension(
                            unsafe { CStr::from_ptr(*name) },
                            &native_extensions,
                        )
                })
                .collect()
        };
    let mut forwarded_create_info = unsafe { *create_info };
    forwarded_create_info.enabled_extension_count = enabled_names.len() as u32;
    forwarded_create_info.pp_enabled_extension_names = enabled_names.as_ptr();
    unsafe { abi::advance_device_chain(create_info, link.p_next) };
    let result = crate::trace_call!(
        "vkCreateDevice",
        format!("physical_device={physical_device:?}"),
        unsafe {
            next_create(
                physical_device,
                &forwarded_create_info,
                allocator,
                device_out,
            )
        }
    );
    if result == vk::Result::SUCCESS {
        devices().write().unwrap().insert(
            unsafe { (*device_out).as_raw() },
            DeviceDispatch { gdpa: next_gdpa },
        );
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkEnumeratePhysicalDevices(
    instance: vk::Instance,
    count: *mut u32,
    physicals: *mut vk::PhysicalDevice,
) -> vk::Result {
    tracing::init();
    let Some(dispatch) = instances()
        .read()
        .ok()
        .and_then(|map| map.get(&instance.as_raw()).copied())
    else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let Some(function) =
        (unsafe { (dispatch.gipa)(instance, c"vkEnumeratePhysicalDevices".as_ptr()) })
    else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let next: vk::PFN_vkEnumeratePhysicalDevices = unsafe { std::mem::transmute(function) };
    let result = crate::trace_call!("vkEnumeratePhysicalDevices", "instance", unsafe {
        next(instance, count, physicals)
    });
    if matches!(result, vk::Result::SUCCESS | vk::Result::INCOMPLETE)
        && !physicals.is_null()
        && !count.is_null()
    {
        let mut map = physical_devices().write().unwrap();
        for physical in unsafe { std::slice::from_raw_parts(physicals, *count as usize) } {
            map.insert(physical.as_raw(), instance.as_raw());
        }
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkEnumerateDeviceExtensionProperties(
    physical: vk::PhysicalDevice,
    layer_name: *const c_char,
    count: *mut u32,
    properties: *mut vk::ExtensionProperties,
) -> vk::Result {
    tracing::init();
    log::debug!("enumerating native plus synthetic device extensions for {physical:?}");
    let Some((instance_raw, dispatch)) = instance_for_physical(physical) else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let instance = vk::Instance::from_raw(instance_raw);
    let Some(function) =
        (unsafe { (dispatch.gipa)(instance, c"vkEnumerateDeviceExtensionProperties".as_ptr()) })
    else {
        return vk::Result::ERROR_EXTENSION_NOT_PRESENT;
    };
    let next: vk::PFN_vkEnumerateDeviceExtensionProperties =
        unsafe { std::mem::transmute(function) };
    if !layer_name.is_null() {
        if unsafe { CStr::from_ptr(layer_name) } == mock_h264::LAYER_NAME {
            let advertised = mock_h264::extension_properties();
            return write_extension_properties(count, properties, &advertised);
        }
        return unsafe { next(physical, layer_name, count, properties) };
    }
    if count.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }

    let mut native_count = 0;
    let first = unsafe {
        next(
            physical,
            std::ptr::null(),
            &mut native_count,
            std::ptr::null_mut(),
        )
    };
    if first != vk::Result::SUCCESS && first != vk::Result::INCOMPLETE {
        return first;
    }
    let mut native = vec![vk::ExtensionProperties::default(); native_count as usize];
    if native_count != 0 {
        let result = unsafe {
            next(
                physical,
                std::ptr::null(),
                &mut native_count,
                native.as_mut_ptr(),
            )
        };
        if result != vk::Result::SUCCESS && result != vk::Result::INCOMPLETE {
            return result;
        }
        native.truncate(native_count as usize);
    }
    native_device_extensions()
        .write()
        .unwrap()
        .insert(physical.as_raw(), native.clone());
    let mut advertised = native;
    mock_h264::add_missing_extensions(&mut advertised);

    write_extension_properties(count, properties, &advertised)
}

fn write_extension_properties(
    count: *mut u32,
    properties: *mut vk::ExtensionProperties,
    advertised: &[vk::ExtensionProperties],
) -> vk::Result {
    if count.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    if properties.is_null() {
        unsafe { *count = advertised.len() as u32 };
        return vk::Result::SUCCESS;
    }
    let capacity = unsafe { *count as usize };
    let written = capacity.min(advertised.len());
    unsafe {
        std::ptr::copy_nonoverlapping(advertised.as_ptr(), properties, written);
        *count = written as u32;
    }
    if written < advertised.len() {
        vk::Result::INCOMPLETE
    } else {
        log::debug!("reported {} device extensions", written);
        vk::Result::SUCCESS
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetPhysicalDeviceQueueFamilyProperties(
    physical: vk::PhysicalDevice,
    count: *mut u32,
    properties: *mut vk::QueueFamilyProperties,
) {
    tracing::init();
    let Some((instance_raw, dispatch)) = instance_for_physical(physical) else {
        if !count.is_null() {
            unsafe { *count = 0 };
        }
        return;
    };
    let instance = vk::Instance::from_raw(instance_raw);
    let Some(function) = (unsafe {
        (dispatch.gipa)(
            instance,
            c"vkGetPhysicalDeviceQueueFamilyProperties".as_ptr(),
        )
    }) else {
        if !count.is_null() {
            unsafe { *count = 0 };
        }
        return;
    };
    let next: vk::PFN_vkGetPhysicalDeviceQueueFamilyProperties =
        unsafe { std::mem::transmute(function) };
    unsafe { next(physical, count, properties) };
    if !properties.is_null() && !count.is_null() {
        let queues = unsafe { std::slice::from_raw_parts_mut(properties, *count as usize) };
        mock_h264::add_encode_queue(queues);
        log::debug!("added synthetic video encode queue flag for {physical:?}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetPhysicalDeviceVideoCapabilitiesKHR(
    physical: vk::PhysicalDevice,
    profile: *const vk::VideoProfileInfoKHR<'_>,
    capabilities: *mut vk::VideoCapabilitiesKHR<'_>,
) -> vk::Result {
    tracing::init();
    let is_h264 = unsafe { mock_h264::is_h264_encode(profile) };
    let Some(instance_raw) = physical_devices()
        .read()
        .ok()
        .and_then(|map| map.get(&physical.as_raw()).copied())
    else {
        if is_h264 {
            warn!("video query for unknown physical device; supplying H.264 mock limits");
            return unsafe { mock_h264::fill_capabilities(capabilities) };
        }
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let Some(dispatch) = instances()
        .read()
        .ok()
        .and_then(|map| map.get(&instance_raw).copied())
    else {
        return if is_h264 {
            unsafe { mock_h264::fill_capabilities(capabilities) }
        } else {
            vk::Result::ERROR_EXTENSION_NOT_PRESENT
        };
    };
    if is_h264 {
        let native_has_h264 = native_device_extensions()
            .read()
            .ok()
            .and_then(|map| map.get(&physical.as_raw()).cloned())
            .is_some_and(|extensions| {
                extensions.iter().any(|property| unsafe {
                    CStr::from_ptr(property.extension_name.as_ptr())
                        == ash::khr::video_encode_h264::NAME
                })
            });
        if !native_has_h264 {
            info!("ICD does not advertise H.264 encode; using synthetic capabilities");
            return unsafe { mock_h264::fill_capabilities(capabilities) };
        }
    }
    let instance = vk::Instance::from_raw(instance_raw);
    let Some(function) = (unsafe {
        (dispatch.gipa)(
            instance,
            c"vkGetPhysicalDeviceVideoCapabilitiesKHR".as_ptr(),
        )
    }) else {
        if is_h264 {
            info!(
                "driver lacks vkGetPhysicalDeviceVideoCapabilitiesKHR; supplying H.264 mock limits"
            );
            return unsafe { mock_h264::fill_capabilities(capabilities) };
        }
        return vk::Result::ERROR_EXTENSION_NOT_PRESENT;
    };
    let next: VideoCapabilitiesFn = unsafe { std::mem::transmute(function) };
    let result = crate::trace_call!(
        "vkGetPhysicalDeviceVideoCapabilitiesKHR",
        format!("physical_device={physical:?}"),
        unsafe { next(physical, profile, capabilities) }
    );
    if is_h264
        && matches!(
            result,
            vk::Result::ERROR_EXTENSION_NOT_PRESENT
                | vk::Result::ERROR_VIDEO_PROFILE_CODEC_NOT_SUPPORTED_KHR
                | vk::Result::ERROR_VIDEO_PROFILE_OPERATION_NOT_SUPPORTED_KHR
        )
    {
        info!("driver rejected requested video profile; supplying H.264 mock limits");
        return unsafe { mock_h264::fill_capabilities(capabilities) };
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkDestroyInstance(
    instance: vk::Instance,
    allocator: *const vk::AllocationCallbacks<'_>,
) {
    if let Some(dispatch) = instances().write().unwrap().remove(&instance.as_raw()) {
        if let Some(function) = unsafe { (dispatch.gipa)(instance, c"vkDestroyInstance".as_ptr()) }
        {
            let next: vk::PFN_vkDestroyInstance = unsafe { std::mem::transmute(function) };
            unsafe { next(instance, allocator) };
        }
    }
    physical_devices()
        .write()
        .unwrap()
        .retain(|_, owner| *owner != instance.as_raw());
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkDestroyDevice(
    device: vk::Device,
    allocator: *const vk::AllocationCallbacks<'_>,
) {
    if let Some(dispatch) = devices().write().unwrap().remove(&device.as_raw()) {
        if let Some(function) = unsafe { (dispatch.gdpa)(device, c"vkDestroyDevice".as_ptr()) } {
            let next: vk::PFN_vkDestroyDevice = unsafe { std::mem::transmute(function) };
            unsafe { next(device, allocator) };
        }
    }
}
