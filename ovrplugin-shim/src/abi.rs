#![allow(non_camel_case_types)]
//! Struct layouts copied from the C# OVRPlugin wrapper (Sequential layout).
//! Field order and sizes are the ABI; changing one breaks every P/Invoke.
use std::os::raw::c_void;

pub const BOOL_FALSE: i32 = 0;
pub const BOOL_TRUE: i32 = 1;

pub const RESULT_SUCCESS: i32 = 0;
pub const RESULT_FAILURE: i32 = -1000;
pub const RESULT_FAILURE_InvalidParameter: i32 = -1001;

pub const EYE_LEFT: i32 = 0;
pub const EYE_RIGHT: i32 = 1;

pub const NODE_EYE_LEFT: i32 = 0;
pub const NODE_EYE_RIGHT: i32 = 1;
pub const NODE_EYE_CENTER: i32 = 2;
pub const NODE_HAND_LEFT: i32 = 3;
pub const NODE_HAND_RIGHT: i32 = 4;
pub const NODE_HEAD: i32 = 9;

pub const STEP_RENDER: i32 = -1;
pub const STEP_PHYSICS: i32 = 0;

pub const TRACKING_ORIGIN_EYE_LEVEL: i32 = 0;
pub const TRACKING_ORIGIN_FLOOR_LEVEL: i32 = 1;

pub const CONTROLLER_NONE: u32 = 0;
pub const CONTROLLER_LTOUCH: u32 = 1;
pub const CONTROLLER_RTOUCH: u32 = 2;
pub const CONTROLLER_TOUCH: u32 = 3;
pub const CONTROLLER_REMOTE: u32 = 4;
pub const CONTROLLER_GAMEPAD: u32 = 16;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vector2f {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vector2i {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Quatf {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quatf {
    pub const IDENTITY: Quatf = Quatf { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Posef {
    pub orientation: Quatf,
    pub position: Vector3f,
}

impl Posef {
    pub const IDENTITY: Posef = Posef {
        orientation: Quatf::IDENTITY,
        position: Vector3f { x: 0.0, y: 0.0, z: 0.0 },
    };
    /// Upright pose at standing height, used before a session reports tracking.
    pub const STANDING: Posef = Posef {
        orientation: Quatf::IDENTITY,
        position: Vector3f { x: 0.0, y: 1.6, z: 0.0 },
    };
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct PoseStatef {
    pub pose: Posef,
    pub velocity: Vector3f,
    pub acceleration: Vector3f,
    pub angular_velocity: Vector3f,
    pub angular_acceleration: Vector3f,
    pub time: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Sizei {
    pub w: i32,
    pub h: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Sizef {
    pub w: f32,
    pub h: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Recti {
    pub pos: Vector2i,
    pub size: Sizei,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rectf {
    pub pos: Vector2f,
    pub size: Sizef,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fovf {
    pub up_tan: f32,
    pub down_tan: f32,
    pub left_tan: f32,
    pub right_tan: f32,
}

impl Default for Fovf {
    fn default() -> Self {
        Fovf { up_tan: 1.0, down_tan: 1.0, left_tan: 1.0, right_tan: 1.0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Frustumf {
    pub z_near: f32,
    pub z_far: f32,
    pub fov_x: f32,
    pub fov_y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Frustumf2 {
    pub z_near: f32,
    pub z_far: f32,
    pub fov: Fovf,
}

impl Default for Frustumf2 {
    fn default() -> Self {
        Frustumf2 { z_near: 0.1, z_far: 1000.0, fov: Fovf::default() }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ControllerState {
    pub connected_controllers: u32,
    pub buttons: u32,
    pub touches: u32,
    pub near_touches: u32,
    pub l_index_trigger: f32,
    pub r_index_trigger: f32,
    pub l_hand_trigger: f32,
    pub r_hand_trigger: f32,
    pub l_thumbstick: Vector2f,
    pub r_thumbstick: Vector2f,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ControllerState2 {
    pub connected_controllers: u32,
    pub buttons: u32,
    pub touches: u32,
    pub near_touches: u32,
    pub l_index_trigger: f32,
    pub r_index_trigger: f32,
    pub l_hand_trigger: f32,
    pub r_hand_trigger: f32,
    pub l_thumbstick: Vector2f,
    pub r_thumbstick: Vector2f,
    pub l_touchpad: Vector2f,
    pub r_touchpad: Vector2f,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ControllerState4 {
    pub connected_controllers: u32,
    pub buttons: u32,
    pub touches: u32,
    pub near_touches: u32,
    pub l_index_trigger: f32,
    pub r_index_trigger: f32,
    pub l_hand_trigger: f32,
    pub r_hand_trigger: f32,
    pub l_thumbstick: Vector2f,
    pub r_thumbstick: Vector2f,
    pub l_touchpad: Vector2f,
    pub r_touchpad: Vector2f,
    pub l_battery_percent_remaining: u8,
    pub r_battery_percent_remaining: u8,
    pub l_recenter_count: u8,
    pub r_recenter_count: u8,
    pub reserved: [u8; 28],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HapticsBuffer {
    pub samples: *mut c_void,
    pub samples_count: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct HapticsState {
    pub samples_available: i32,
    pub samples_queued: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HapticsDesc {
    pub sample_rate_hz: i32,
    pub sample_size_in_bytes: i32,
    pub minimum_safe_samples_queued: i32,
    pub minimum_buffer_samples_count: i32,
    pub optimal_buffer_samples_count: i32,
    pub maximum_buffer_samples_count: i32,
}

impl Default for HapticsDesc {
    fn default() -> Self {
        HapticsDesc {
            sample_rate_hz: 320,
            sample_size_in_bytes: 1,
            minimum_safe_samples_queued: 4,
            minimum_buffer_samples_count: 16,
            optimal_buffer_samples_count: 32,
            maximum_buffer_samples_count: 64,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AppPerfFrameStats {
    pub hmd_vsync_index: i32,
    pub app_frame_index: i32,
    pub app_dropped_frame_count: i32,
    pub app_motion_to_photon_latency: f32,
    pub app_queue_ahead_time: f32,
    pub app_cpu_elapsed_time: f32,
    pub app_gpu_elapsed_time: f32,
    pub compositor_frame_index: i32,
    pub compositor_dropped_frame_count: i32,
    pub compositor_latency: f32,
    pub compositor_cpu_elapsed_time: f32,
    pub compositor_gpu_elapsed_time: f32,
    pub compositor_cpu_start_to_gpu_end_elapsed_time: f32,
    pub compositor_gpu_end_to_vsync_elapsed_time: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AppPerfStats {
    pub frame_stats: [AppPerfFrameStats; 5],
    pub frame_stats_count: i32,
    pub any_frame_stats_dropped: i32,
    pub adaptive_gpu_performance_scale: f32,
}

impl Default for AppPerfStats {
    fn default() -> Self {
        AppPerfStats {
            frame_stats: [AppPerfFrameStats::default(); 5],
            frame_stats_count: 0,
            any_frame_stats_dropped: 0,
            adaptive_gpu_performance_scale: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BoundaryTestResult {
    pub is_triggering: i32,
    pub closest_distance: f32,
    pub closest_point: Vector3f,
    pub closest_point_normal: Vector3f,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BoundaryGeometry {
    pub boundary_type: i32,
    pub points: [Vector3f; 256],
    pub points_count: i32,
}

impl Default for BoundaryGeometry {
    fn default() -> Self {
        BoundaryGeometry { boundary_type: 0, points: [Vector3f::default(); 256], points_count: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Colorf {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CameraIntrinsics {
    pub is_valid: i32,
    pub last_changed_time_seconds: f64,
    pub fov_port: Fovf,
    pub virtual_near_plane_distance_meters: f32,
    pub virtual_far_plane_distance_meters: f32,
    pub image_sensor_pixel_resolution: Sizei,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CameraExtrinsics {
    pub is_valid: i32,
    pub last_changed_time_seconds: f64,
    pub camera_status_data: i32,
    pub attached_to_node: i32,
    pub relative_pose: Posef,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CameraDeviceIntrinsicsParameters {
    pub fx: f32,
    pub fy: f32,
    pub cx: f32,
    pub cy: f32,
    pub disto0: f64,
    pub disto1: f64,
    pub disto2: f64,
    pub disto3: f64,
    pub disto4: f64,
    pub v_fov: f32,
    pub h_fov: f32,
    pub d_fov: f32,
    pub w: i32,
    pub h: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TextureRectMatrixf {
    pub left_rect: Vector4,
    pub right_rect: Vector4,
    pub left_scale_bias: Vector4,
    pub right_scale_bias: Vector4,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LayerDesc {
    pub shape: i32,
    pub layout: i32,
    pub texture_size: Sizei,
    pub mip_levels: i32,
    pub sample_count: i32,
    pub format: i32,
    pub layer_flags: i32,
    pub fov: [Fovf; 2],
    pub visible_rect: [Rectf; 2],
    pub max_viewport_size: Sizei,
    pub depth_format: i32,
}

impl Default for LayerDesc {
    fn default() -> Self {
        LayerDesc {
            shape: 0,
            layout: 0,
            texture_size: Sizei { w: 0, h: 0 },
            mip_levels: 0,
            sample_count: 0,
            format: 0,
            layer_flags: 0,
            fov: [Fovf::default(); 2],
            visible_rect: [Rectf::default(); 2],
            max_viewport_size: Sizei { w: 0, h: 0 },
            depth_format: 0,
        }
    }
}

// ---- LibOVRRT (runtime) types, only what the core calls ----

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OvSessionStatus {
    pub is_visible: i32,
    pub hmd_present: i32,
    pub hmd_mounted: i32,
    pub display_lost: i32,
    pub should_quit: i32,
    pub should_recenter: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OvPoseStatef {
    pub orientation: Quatf,
    pub position: Vector3f,
    pub angular_velocity: Vector3f,
    pub linear_velocity: Vector3f,
    pub angular_acceleration: Vector3f,
    pub linear_acceleration: Vector3f,
    pub time_seconds: f64,
    pub status: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OvBoundaryTestResult {
    pub is_triggering: i32,
    pub closest_distance: f32,
    pub closest_point: Vector3f,
    pub closest_point_normal: Vector3f,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OvFovPort {
    pub up_tan: f32,
    pub down_tan: f32,
    pub left_tan: f32,
    pub right_tan: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct OvSizei {
    pub w: i32,
    pub h: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OvErrorInfo {
    pub result: i32,
    pub error_string: [u8; 512],
}

impl Default for OvErrorInfo {
    fn default() -> Self {
        OvErrorInfo { result: 0, error_string: [0; 512] }
    }
}
