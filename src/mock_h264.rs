//! Conservative synthetic limits used when the driver lacks H.264 video queries.

use ash::vk;

const H264_ENCODE_OPERATION: u32 = 0x0001_0000;

pub const LAYER_NAME: &std::ffi::CStr = c"VK_LAYER_FFMPEG_mock";

const MOCKED_DEVICE_EXTENSIONS: [&std::ffi::CStr; 7] = [
    ash::khr::video_queue::NAME,
    ash::khr::video_encode_queue::NAME,
    ash::khr::video_encode_h264::NAME,
    ash::khr::external_memory::NAME,
    ash::khr::external_memory_fd::NAME,
    ash::ext::external_memory_dma_buf::NAME,
    ash::ext::image_drm_format_modifier::NAME,
];

/// Add the Vulkan Video encode extensions needed by Moonshine when the ICD
/// does not list them. The layer does not claim extensions already reported.
pub fn add_missing_extensions(extensions: &mut Vec<vk::ExtensionProperties>) {
    for name in MOCKED_DEVICE_EXTENSIONS {
        let present = extensions.iter().any(|property| unsafe {
            std::ffi::CStr::from_ptr(property.extension_name.as_ptr()) == name
        });
        if !present {
            let mut property = vk::ExtensionProperties::default();
            let bytes = name.to_bytes_with_nul();
            for (dst, src) in property.extension_name.iter_mut().zip(bytes) {
                *dst = *src as std::ffi::c_char;
            }
            property.spec_version = 1;
            extensions.push(property);
        }
    }
}

pub fn extension_properties() -> Vec<vk::ExtensionProperties> {
    MOCKED_DEVICE_EXTENSIONS
        .iter()
        .map(|name| {
            let mut property = vk::ExtensionProperties::default();
            for (dst, src) in property
                .extension_name
                .iter_mut()
                .zip(name.to_bytes_with_nul())
            {
                *dst = *src as std::ffi::c_char;
            }
            property.spec_version = 1;
            property
        })
        .collect()
}

/// Mark a real queue as the synthetic encode queue for device selection.
pub fn add_encode_queue(queue_families: &mut [vk::QueueFamilyProperties]) {
    if let Some(queue) = queue_families
        .iter_mut()
        .find(|queue| queue.queue_count > 0)
    {
        queue.queue_flags |= vk::QueueFlags::VIDEO_ENCODE_KHR;
    }
}

/// Return true when an enabled extension is one of the synthetic extensions
/// and the ICD did not advertise it. Such names must be removed before the
/// downstream vkCreateDevice call or the ICD will reject the device.
pub fn should_strip_extension(
    name: &std::ffi::CStr,
    native_extensions: &[vk::ExtensionProperties],
) -> bool {
    MOCKED_DEVICE_EXTENSIONS.contains(&name)
        && !native_extensions.iter().any(|property| unsafe {
            std::ffi::CStr::from_ptr(property.extension_name.as_ptr()) == name
        })
}

pub unsafe fn is_h264_encode(profile: *const vk::VideoProfileInfoKHR<'_>) -> bool {
    !profile.is_null()
        && unsafe { (*profile).video_codec_operation.as_raw() & H264_ENCODE_OPERATION != 0 }
}

pub unsafe fn fill_capabilities(capabilities: *mut vk::VideoCapabilitiesKHR<'_>) -> vk::Result {
    if capabilities.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    // Do not touch pNext: callers may have attached codec-specific output structs.
    unsafe {
        (*capabilities).min_bitstream_buffer_offset_alignment = 256;
        (*capabilities).min_bitstream_buffer_size_alignment = 256;
        (*capabilities).min_coded_extent = vk::Extent2D {
            width: 64,
            height: 64,
        };
        (*capabilities).max_coded_extent = vk::Extent2D {
            width: 3840,
            height: 2160,
        };
        (*capabilities).max_dpb_slots = 1;
        (*capabilities).max_active_reference_pictures = 1;
    }
    vk::Result::SUCCESS
}
