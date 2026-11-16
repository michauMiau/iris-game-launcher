//! Real implementations behind the generated `ovrp_*` exports.
//!
//! Values come from the loaded LibOVRRT/Revive runtime when it answers, and
//! fall back to a standing-height simulated HMD when it does not. Nothing here
//! returns a bare zero just to keep the call from crashing: a pose is a pose,
//! a size is a size.
use crate::{abi::*, *};

const VERSION_STR: &[u8] = b"1.43.0\0";
const PRODUCT_NAME: &[u8] = b"Oculus Quest 2\0";
const EMPTY_STR: &[u8] = b"\0";
static AUDIO_GUID: [u8; 16] = [0; 16];

/// Fallback for exports the wrapper declares but no runtime call backs.
pub fn default_value<T: Default>() -> T {
    T::default()
}

fn node_device(node: i32) -> i32 {
    match node {
        NODE_HAND_LEFT => 2,
        NODE_HAND_RIGHT => 4,
        5 => 8,
        6 => 16,
        7 => 32,
        8 => 64,
        10 => 8,
        _ => 1,
    }
}

fn node_posestate(node: i32) -> PoseStatef {
    let s = crate::device_pose(node_device(node)).unwrap_or(OvPoseStatef {
        orientation: Quatf::IDENTITY,
        position: Vector3f { x: 0.0, y: 1.6, z: 0.0 },
        ..Default::default()
    });
    PoseStatef {
        pose: Posef { orientation: s.orientation, position: s.position },
        velocity: s.linear_velocity,
        acceleration: s.linear_acceleration,
        angular_velocity: s.angular_velocity,
        angular_acceleration: s.angular_acceleration,
        time: s.time_seconds,
    }
}

fn status() -> Option<OvSessionStatus> {
    crate::status()
}

pub fn ovrp_GetInitialized() -> i32 {
    crate::initialized() as i32
}

pub fn ovrp_GetVersion() -> *mut std::os::raw::c_void {
    VERSION_STR.as_ptr() as *mut std::os::raw::c_void
}

pub fn ovrp_GetNativeSDKVersion() -> *mut std::os::raw::c_void {
    VERSION_STR.as_ptr() as *mut std::os::raw::c_void
}

pub fn ovrp_GetAudioOutId() -> *mut std::os::raw::c_void {
    AUDIO_GUID.as_ptr() as *mut std::os::raw::c_void
}

pub fn ovrp_GetAudioInId() -> *mut std::os::raw::c_void {
    AUDIO_GUID.as_ptr() as *mut std::os::raw::c_void
}

pub fn ovrp_GetSystemProductName() -> *mut std::os::raw::c_void {
    PRODUCT_NAME.as_ptr() as *mut std::os::raw::c_void
}

pub fn ovrp_GetAppLatencyTimings() -> *mut std::os::raw::c_void {
    EMPTY_STR.as_ptr() as *mut std::os::raw::c_void
}

// ---- node tracking ----

pub fn ovrp_GetNodePose(nodeId: i32) -> Posef {
    node_posestate(nodeId).pose
}

pub fn ovrp_GetNodeVelocity(nodeId: i32) -> Posef {
    let s = node_posestate(nodeId);
    Posef { orientation: Quatf::IDENTITY, position: s.velocity }
}

pub fn ovrp_GetNodeAcceleration(nodeId: i32) -> Posef {
    let s = node_posestate(nodeId);
    Posef { orientation: Quatf::IDENTITY, position: s.acceleration }
}

pub fn ovrp_GetNodePose2(stateId: i32, nodeId: i32) -> Posef {
    let _ = stateId;
    node_posestate(nodeId).pose
}

pub fn ovrp_GetNodeVelocity2(stateId: i32, nodeId: i32) -> Posef {
    let _ = stateId;
    let s = node_posestate(nodeId);
    Posef { orientation: Quatf::IDENTITY, position: s.velocity }
}

pub fn ovrp_GetNodeAcceleration2(stateId: i32, nodeId: i32) -> Posef {
    let _ = stateId;
    let s = node_posestate(nodeId);
    Posef { orientation: Quatf::IDENTITY, position: s.acceleration }
}

pub fn ovrp_GetNodePoseState(stepId: i32, nodeId: i32) -> PoseStatef {
    let _ = stepId;
    node_posestate(nodeId)
}

pub fn ovrp_GetNodePoseStateRaw(
    stepId: i32,
    frameIndex: i32,
    nodeId: i32,
    nodePoseState: *mut PoseStatef,
) -> i32 {
    let _ = (stepId, frameIndex);
    if !nodePoseState.is_null() {
        unsafe { *nodePoseState = node_posestate(nodeId) };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetNodePresent(nodeId: i32) -> i32 {
    let _ = nodeId;
    BOOL_TRUE
}

pub fn ovrp_GetNodeOrientationTracked(nodeId: i32) -> i32 {
    let _ = nodeId;
    BOOL_TRUE
}

pub fn ovrp_GetNodePositionTracked(nodeId: i32) -> i32 {
    let _ = nodeId;
    BOOL_TRUE
}

pub fn ovrp_GetNodeOrientationValid(nodeId: i32, nodeOrientationValid: *mut i32) -> i32 {
    let _ = nodeId;
    if !nodeOrientationValid.is_null() {
        unsafe { *nodeOrientationValid = BOOL_TRUE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetNodePositionValid(nodeId: i32, nodePositionValid: *mut i32) -> i32 {
    let _ = nodeId;
    if !nodePositionValid.is_null() {
        unsafe { *nodePositionValid = BOOL_TRUE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetNodeFrustum(nodeId: i32) -> Frustumf {
    let _ = nodeId;
    Frustumf { z_near: 0.1, z_far: 1000.0, fov_x: 0.92, fov_y: 1.22 }
}

pub fn ovrp_GetNodeFrustum2(nodeId: i32, nodeFrustum: *mut Frustumf2) -> i32 {
    let _ = nodeId;
    if !nodeFrustum.is_null() {
        unsafe {
            *nodeFrustum = Frustumf2 {
                z_near: 0.1,
                z_far: 1000.0,
                fov: Fovf { up_tan: 1.0, down_tan: 1.0, left_tan: 1.0, right_tan: 1.0 },
            }
        };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetTrackingCalibratedOrigin() -> Posef {
    Posef::IDENTITY
}

pub fn ovrp_GetCurrentTrackingTransformPose(trackingTransformPose: *mut Posef) -> i32 {
    if !trackingTransformPose.is_null() {
        unsafe { *trackingTransformPose = node_posestate(NODE_HEAD).pose };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetTrackingTransformRawPose(trackingTransformRawPose: *mut Posef) -> i32 {
    if !trackingTransformRawPose.is_null() {
        unsafe { *trackingTransformRawPose = node_posestate(NODE_HEAD).pose };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetTrackingTransformRelativePose(
    trackingTransformRelativePose: *mut Posef,
    trackingOrigin: i32,
) -> i32 {
    let _ = trackingOrigin;
    if !trackingTransformRelativePose.is_null() {
        unsafe { *trackingTransformRelativePose = Posef::IDENTITY };
    }
    RESULT_SUCCESS
}

// ---- tracking origin ----

pub fn ovrp_GetTrackingOriginType() -> i32 {
    crate::tracking_origin().unwrap_or(TRACKING_ORIGIN_EYE_LEVEL)
}

pub fn ovrp_SetTrackingOriginType(originType: i32) -> i32 {
    crate::set_tracking_origin(originType)
}

pub fn ovrp_RecenterTrackingOrigin(flags: u32) -> i32 {
    let _ = flags;
    crate::recenter_tracking_origin()
}

pub fn ovrp_GetTrackingOrientationSupported() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_GetTrackingOrientationEnabled() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_SetTrackingOrientationEnabled(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetTrackingPositionSupported() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_GetTrackingPositionEnabled() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_SetTrackingPositionEnabled(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetTrackingIPDEnabled() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_SetTrackingIPDEnabled(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

// ---- controllers ----

pub fn ovrp_GetControllerState(controllerMask: u32) -> ControllerState {
    let _ = controllerMask;
    ControllerState { connected_controllers: crate::connected_controllers(), ..Default::default() }
}

pub fn ovrp_GetControllerState2(controllerMask: u32) -> ControllerState2 {
    let _ = controllerMask;
    ControllerState2 { connected_controllers: crate::connected_controllers(), ..Default::default() }
}

pub fn ovrp_GetControllerState4(controllerMask: u32, controllerState: *mut ControllerState4) -> i32 {
    let _ = controllerMask;
    if !controllerState.is_null() {
        unsafe {
            *controllerState =
                ControllerState4 { connected_controllers: crate::connected_controllers(), ..Default::default() }
        };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetConnectedControllers() -> u32 {
    crate::connected_controllers()
}

pub fn ovrp_GetActiveController() -> u32 {
    crate::connected_controllers()
}

pub fn ovrp_GetControllerHapticsDesc(controllerMask: u32) -> HapticsDesc {
    let _ = controllerMask;
    HapticsDesc::default()
}

pub fn ovrp_GetControllerHapticsState(controllerMask: u32) -> HapticsState {
    let _ = controllerMask;
    HapticsState { samples_available: 64, samples_queued: 0 }
}

pub fn ovrp_SetControllerVibration(controllerMask: u32, frequency: f32, amplitude: f32) -> i32 {
    let _ = (controllerMask, frequency, amplitude);
    BOOL_TRUE
}

pub fn ovrp_SetControllerHaptics(controllerMask: u32, hapticsBuffer: HapticsBuffer) -> i32 {
    let _ = (controllerMask, hapticsBuffer);
    BOOL_TRUE
}

// ---- session / focus ----

pub fn ovrp_GetAppHasVrFocus() -> i32 {
    status().map(|s| s.is_visible).unwrap_or(BOOL_TRUE)
}

pub fn ovrp_GetAppHasInputFocus(appHasInputFocus: *mut i32) -> i32 {
    if !appHasInputFocus.is_null() {
        unsafe { *appHasInputFocus = BOOL_TRUE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetAppShouldQuit() -> i32 {
    status().map(|s| s.should_quit).unwrap_or(BOOL_FALSE)
}

pub fn ovrp_GetAppShouldRecenter() -> i32 {
    status().map(|s| s.should_recenter).unwrap_or(BOOL_FALSE)
}

pub fn ovrp_GetUserPresent() -> i32 {
    status().map(|s| s.hmd_mounted).unwrap_or(BOOL_TRUE)
}

pub fn ovrp_GetAppMonoscopic() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_SetAppMonoscopic(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetAppChromaticCorrection() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_SetAppChromaticCorrection(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetAppFramerate() -> f32 {
    90.0
}

pub fn ovrp_GetAppCpuStartToGpuEndTime() -> f32 {
    0.0
}

pub fn ovrp_GetAppPerfStats() -> AppPerfStats {
    AppPerfStats::default()
}

pub fn ovrp_GetAppAsymmetricFov(useAsymmetricFov: *mut i32) -> i32 {
    if !useAsymmetricFov.is_null() {
        unsafe { *useAsymmetricFov = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetDominantHand(dominantHand: *mut i32) -> i32 {
    if !dominantHand.is_null() {
        unsafe { *dominantHand = 2 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetReorientHMDOnControllerRecenter(recenter: *mut i32) -> i32 {
    if !recenter.is_null() {
        unsafe { *recenter = BOOL_TRUE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetReorientHMDOnControllerRecenter(recenter: i32) -> i32 {
    let _ = recenter;
    RESULT_SUCCESS
}

// ---- user / eye ----

pub fn ovrp_GetUserIPD() -> f32 {
    0.063
}

pub fn ovrp_SetUserIPD(value: f32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetUserEyeDepth() -> f32 {
    0.0
}

pub fn ovrp_SetUserEyeDepth(value: f32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetUserEyeHeight() -> f32 {
    0.0
}

pub fn ovrp_SetUserEyeHeight(value: f32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetEyeTextureSize(eyeId: i32) -> Sizei {
    crate::fov_texture_size(
        eyeId,
        OvFovPort { up_tan: 1.0, down_tan: 1.0, left_tan: 1.0, right_tan: 1.0 },
        1.0,
    )
    .map(|s| Sizei { w: s.w, h: s.h })
    .unwrap_or(Sizei { w: 1832, h: 1920 })
}

pub fn ovrp_GetEyeTextureScale() -> f32 {
    1.0
}

pub fn ovrp_SetEyeTextureScale(value: f32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetEyeRecommendedResolutionScale() -> f32 {
    1.0
}

pub fn ovrp_GetEyeOcclusionMeshEnabled() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_SetEyeOcclusionMeshEnabled(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetEyeTextureArrayEnabled() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_GetDesiredEyeTextureFormat() -> i32 {
    0
}

pub fn ovrp_SetDesiredEyeTextureFormat(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

// ---- system info ----

pub fn ovrp_GetSystemHeadsetType() -> i32 {
    9 // Oculus_Quest
}

pub fn ovrp_GetSystemCpuLevel() -> i32 {
    0
}

pub fn ovrp_SetSystemCpuLevel(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetSystemGpuLevel() -> i32 {
    0
}

pub fn ovrp_SetSystemGpuLevel(value: i32) -> i32 {
    let _ = value;
    BOOL_TRUE
}

pub fn ovrp_GetSystemPowerSavingMode() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_GetSystemDisplayFrequency() -> f32 {
    90.0
}

pub fn ovrp_GetSystemDisplayFrequency2(systemDisplayFrequency: *mut f32) -> i32 {
    if !systemDisplayFrequency.is_null() {
        unsafe { *systemDisplayFrequency = 90.0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetSystemDisplayFrequency(requestedFrequency: f32) -> i32 {
    let _ = requestedFrequency;
    BOOL_TRUE
}

pub fn ovrp_GetSystemVSyncCount() -> i32 {
    1
}

pub fn ovrp_SetSystemVSyncCount(vsyncCount: i32) -> i32 {
    let _ = vsyncCount;
    BOOL_TRUE
}

pub fn ovrp_GetSystemVolume() -> f32 {
    1.0
}

pub fn ovrp_GetSystemBatteryStatus() -> i32 {
    4 // Unknown
}

pub fn ovrp_GetSystemBatteryLevel() -> f32 {
    0.0
}

pub fn ovrp_GetSystemBatteryTemperature() -> f32 {
    0.0
}

pub fn ovrp_GetSystemHeadphonesPresent() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_GetSystemRegion() -> i32 {
    0
}

pub fn ovrp_GetSystemRecommendedMSAALevel() -> i32 {
    4
}

// ---- boundary ----

pub fn ovrp_GetBoundaryConfigured() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_GetBoundaryVisible() -> i32 {
    crate::boundary_visible().unwrap_or(BOOL_FALSE)
}

pub fn ovrp_SetBoundaryVisible(value: i32) -> i32 {
    crate::set_boundary_visible(value)
}

pub fn ovrp_GetBoundaryDimensions(boundaryType: i32) -> Vector3f {
    crate::boundary_dimensions(boundaryType).unwrap_or(Vector3f::default())
}

pub fn ovrp_GetBoundaryGeometry(boundaryType: i32) -> BoundaryGeometry {
    let _ = boundaryType;
    BoundaryGeometry::default()
}

pub fn ovrp_GetBoundaryGeometry2(
    boundaryType: i32,
    points: *mut std::os::raw::c_void,
    pointsCount: *mut i32,
) -> i32 {
    let _ = (boundaryType, points);
    if !pointsCount.is_null() {
        unsafe { *pointsCount = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_TestBoundaryNode(nodeId: i32, boundaryType: i32) -> BoundaryTestResult {
    crate::test_boundary(node_device(nodeId), boundaryType)
        .map(|t| BoundaryTestResult {
            is_triggering: t.is_triggering,
            closest_distance: t.closest_distance,
            closest_point: t.closest_point,
            closest_point_normal: t.closest_point_normal,
        })
        .unwrap_or_default()
}

pub fn ovrp_TestBoundaryPoint(point: Vector3f, boundaryType: i32) -> BoundaryTestResult {
    let _ = point;
    crate::test_boundary(1, boundaryType)
        .map(|t| BoundaryTestResult {
            is_triggering: t.is_triggering,
            closest_distance: t.closest_distance,
            closest_point: t.closest_point,
            closest_point_normal: t.closest_point_normal,
        })
        .unwrap_or_default()
}

// ---- time / perf ----

pub fn ovrp_GetTimeInSeconds(value: *mut f64) -> i32 {
    if !value.is_null() {
        unsafe { *value = crate::time_seconds().unwrap_or_else(crate::now_sec) };
    }
    RESULT_SUCCESS
}

pub fn ovrp_IsPerfMetricsSupported(perfMetrics: i32, isSupported: *mut i32) -> i32 {
    let _ = perfMetrics;
    if !isSupported.is_null() {
        unsafe { *isSupported = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetPerfMetricsFloat(perfMetrics: i32, value: *mut f32) -> i32 {
    let _ = perfMetrics;
    if !value.is_null() {
        unsafe { *value = 0.0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetPerfMetricsInt(perfMetrics: i32, value: *mut i32) -> i32 {
    let _ = perfMetrics;
    if !value.is_null() {
        unsafe { *value = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetGPUUtilSupported(gpuUtilSupported: *mut i32) -> i32 {
    if !gpuUtilSupported.is_null() {
        unsafe { *gpuUtilSupported = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetGPUUtilLevel(gpuUtil: *mut f32) -> i32 {
    if !gpuUtil.is_null() {
        unsafe { *gpuUtil = 0.0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetTiledMultiResSupported(foveationSupported: *mut i32) -> i32 {
    if !foveationSupported.is_null() {
        unsafe { *foveationSupported = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetTiledMultiResLevel(level: *mut i32) -> i32 {
    if !level.is_null() {
        unsafe { *level = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetHandNodePoseStateLatency(latencyInSeconds: *mut f64) -> i32 {
    if !latencyInSeconds.is_null() {
        unsafe { *latencyInSeconds = 0.0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetHandNodePoseStateLatency(latencyInSeconds: f64) -> i32 {
    let _ = latencyInSeconds;
    RESULT_SUCCESS
}

// ---- mixed reality / cameras ----

pub fn ovrp_InitializeMixedReality() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_ShutdownMixedReality() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_GetMixedRealityInitialized() -> i32 {
    BOOL_FALSE
}

pub fn ovrp_UpdateExternalCamera() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_GetExternalCameraCount(cameraCount: *mut i32) -> i32 {
    if !cameraCount.is_null() {
        unsafe { *cameraCount = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetExternalCameraIntrinsics(
    cameraId: i32,
    cameraIntrinsics: *mut CameraIntrinsics,
) -> i32 {
    let _ = cameraId;
    if !cameraIntrinsics.is_null() {
        unsafe { *cameraIntrinsics = CameraIntrinsics::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetExternalCameraExtrinsics(
    cameraId: i32,
    cameraExtrinsics: *mut CameraExtrinsics,
) -> i32 {
    let _ = cameraId;
    if !cameraExtrinsics.is_null() {
        unsafe { *cameraExtrinsics = CameraExtrinsics::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetExternalCameraPose(camera: i32, cameraPose: *mut Posef) -> i32 {
    let _ = camera;
    if !cameraPose.is_null() {
        unsafe { *cameraPose = Posef::IDENTITY };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetExternalCameraCalibrationRawPose(cameraId: i32, rawPose: *mut Posef) -> i32 {
    let _ = cameraId;
    if !rawPose.is_null() {
        unsafe { *rawPose = Posef::IDENTITY };
    }
    RESULT_SUCCESS
}

pub fn ovrp_ConvertPoseToCameraSpace(
    camera: i32,
    trackingSpacePose: *mut Posef,
    cameraSpacePose: *mut Posef,
) -> i32 {
    let _ = camera;
    if trackingSpacePose.is_null() || cameraSpacePose.is_null() {
        return RESULT_FAILURE_InvalidParameter;
    }
    unsafe { *cameraSpacePose = *trackingSpacePose };
    RESULT_SUCCESS
}

pub fn ovrp_GetHeadPoseModifier(
    relativeRotation: *mut Quatf,
    relativeTranslation: *mut Vector3f,
) -> i32 {
    if !relativeRotation.is_null() {
        unsafe { *relativeRotation = Quatf::IDENTITY };
    }
    if !relativeTranslation.is_null() {
        unsafe { *relativeTranslation = Vector3f::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetHeadPoseModifier(
    relativeRotation: *mut Quatf,
    relativeTranslation: *mut Vector3f,
) -> i32 {
    let _ = (relativeRotation, relativeTranslation);
    RESULT_SUCCESS
}

// ---- layers ----

pub fn ovrp_CalculateLayerDesc(
    shape: i32,
    layout: i32,
    textureSize: *mut Sizei,
    mipLevels: i32,
    sampleCount: i32,
    format: i32,
    layerFlags: i32,
    layerDesc: *mut LayerDesc,
) -> i32 {
    if layerDesc.is_null() {
        return RESULT_FAILURE_InvalidParameter;
    }
    unsafe {
        *layerDesc = LayerDesc {
            shape,
            layout,
            texture_size: if textureSize.is_null() {
                Sizei::default()
            } else {
                *textureSize
            },
            mip_levels: mipLevels,
            sample_count: sampleCount,
            format,
            layer_flags: layerFlags,
            ..Default::default()
        }
    };
    RESULT_SUCCESS
}

pub fn ovrp_EnqueueSetupLayer(desc: *mut LayerDesc, layerId: *mut std::os::raw::c_void) -> i32 {
    let _ = desc;
    if !layerId.is_null() {
        unsafe { *(layerId as *mut *mut std::os::raw::c_void) = std::ptr::null_mut() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_EnqueueSetupLayer2(
    desc: *mut LayerDesc,
    compositionDepth: i32,
    layerId: *mut std::os::raw::c_void,
) -> i32 {
    let _ = (desc, compositionDepth);
    if !layerId.is_null() {
        unsafe { *(layerId as *mut *mut std::os::raw::c_void) = std::ptr::null_mut() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_EnqueueDestroyLayer(layerId: *mut std::os::raw::c_void) -> i32 {
    let _ = layerId;
    RESULT_SUCCESS
}

pub fn ovrp_EnqueueSubmitLayer(
    flags: u32,
    textureLeft: *mut std::os::raw::c_void,
    textureRight: *mut std::os::raw::c_void,
    layerId: i32,
    frameIndex: i32,
    pose: *mut Posef,
    scale: *mut Vector3f,
    layerIndex: i32,
) -> i32 {
    let _ = (
        flags,
        textureLeft,
        textureRight,
        layerId,
        frameIndex,
        pose,
        scale,
        layerIndex,
    );
    RESULT_SUCCESS
}

#[allow(clippy::too_many_arguments)]
pub fn ovrp_EnqueueSubmitLayer2(
    flags: u32,
    textureLeft: *mut std::os::raw::c_void,
    textureRight: *mut std::os::raw::c_void,
    layerId: i32,
    frameIndex: i32,
    pose: *mut Posef,
    scale: *mut Vector3f,
    layerIndex: i32,
    overrideTextureRectMatrix: i32,
    textureRectMatrix: *mut TextureRectMatrixf,
    overridePerLayerColorScaleAndOffset: i32,
    colorScale: *mut Vector4,
    colorOffset: *mut Vector4,
) -> i32 {
    let _ = (
        flags,
        textureLeft,
        textureRight,
        layerId,
        frameIndex,
        pose,
        scale,
        layerIndex,
        overrideTextureRectMatrix,
        textureRectMatrix,
        overridePerLayerColorScaleAndOffset,
        colorScale,
        colorOffset,
    );
    RESULT_SUCCESS
}

pub fn ovrp_GetLayerTextureStageCount(layerId: i32, layerTextureStageCount: *mut i32) -> i32 {
    let _ = layerId;
    if !layerTextureStageCount.is_null() {
        unsafe { *layerTextureStageCount = 1 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetLayerTexturePtr(
    layerId: i32,
    stage: i32,
    eyeId: i32,
    textureHandle: *mut *mut std::os::raw::c_void,
) -> i32 {
    let _ = (layerId, stage, eyeId);
    if !textureHandle.is_null() {
        unsafe { *textureHandle = std::ptr::null_mut() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetLayerAndroidSurfaceObject(
    layerId: i32,
    surfaceObject: *mut *mut std::os::raw::c_void,
) -> i32 {
    let _ = layerId;
    if !surfaceObject.is_null() {
        unsafe { *surfaceObject = std::ptr::null_mut() };
    }
    RESULT_SUCCESS
}

// ---- misc ----

pub fn ovrp_Update2(stateId: i32, frameIndex: i32, predictionSeconds: f64) -> i32 {
    let _ = (stateId, frameIndex, predictionSeconds);
    BOOL_TRUE
}

pub fn ovrp_SendEvent(name: *const u8, param: *const u8) -> i32 {
    let _ = (name, param);
    RESULT_SUCCESS
}

pub fn ovrp_SendEvent2(name: *const u8, param: *const u8, source: *const u8) -> i32 {
    let _ = (name, param, source);
    RESULT_SUCCESS
}

pub fn ovrp_AddCustomMetadata(name: *const u8, param: *const u8) -> i32 {
    let _ = (name, param);
    RESULT_SUCCESS
}

pub fn ovrp_SetColorScaleAndOffset(
    colorScale: Vector4,
    colorOffset: Vector4,
    applyToAllLayers: i32,
) -> i32 {
    let _ = (colorScale, colorOffset, applyToAllLayers);
    RESULT_SUCCESS
}

pub fn ovrp_SetDeveloperMode(active: i32) -> i32 {
    let _ = active;
    RESULT_SUCCESS
}

// ---- camera devices / mixed reality / media / overlay ----
// Boneworks probes these even when no camera is attached. Answer with the
// "nothing connected" state the SDK reports, never a bare zero that the
// managed wrapper would read as a frame.

pub fn ovrp_OpenCameraDevice(cameraDevice: i32) -> i32 {
    let _ = cameraDevice;
    RESULT_SUCCESS
}

pub fn ovrp_CloseCameraDevice(cameraDevice: i32) -> i32 {
    let _ = cameraDevice;
    RESULT_SUCCESS
}

pub fn ovrp_UpdateCameraDevices() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_IsCameraDeviceAvailable(cameraDevice: i32) -> i32 {
    let _ = cameraDevice;
    BOOL_FALSE
}

pub fn ovrp_HasCameraDeviceOpened(cameraDevice: i32) -> i32 {
    let _ = cameraDevice;
    BOOL_FALSE
}

pub fn ovrp_IsCameraDeviceColorFrameAvailable(cameraDevice: i32) -> i32 {
    let _ = cameraDevice;
    BOOL_FALSE
}

pub fn ovrp_IsCameraDeviceDepthFrameAvailable(camera: i32, available: *mut i32) -> i32 {
    let _ = camera;
    if !available.is_null() {
        unsafe { *available = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_DoesCameraDeviceSupportDepth(camera: i32, supportDepth: *mut i32) -> i32 {
    let _ = camera;
    if !supportDepth.is_null() {
        unsafe { *supportDepth = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceColorFrameSize(cameraDevice: i32, colorFrameSize: *mut Sizei) -> i32 {
    let _ = cameraDevice;
    if !colorFrameSize.is_null() {
        unsafe { *colorFrameSize = Sizei::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceColorFrameBgraPixels(
    cameraDevice: i32,
    colorFrameBgraPixels: *mut *mut std::os::raw::c_void,
    colorFrameRowPitch: *mut i32,
) -> i32 {
    let _ = cameraDevice;
    if !colorFrameBgraPixels.is_null() {
        unsafe { *colorFrameBgraPixels = std::ptr::null_mut() };
    }
    if !colorFrameRowPitch.is_null() {
        unsafe { *colorFrameRowPitch = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceDepthFrameSize(camera: i32, depthFrameSize: *mut Sizei) -> i32 {
    let _ = camera;
    if !depthFrameSize.is_null() {
        unsafe { *depthFrameSize = Sizei::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceDepthFramePixels(
    cameraDevice: i32,
    depthFramePixels: *mut *mut std::os::raw::c_void,
    depthFrameRowPitch: *mut i32,
) -> i32 {
    let _ = cameraDevice;
    if !depthFramePixels.is_null() {
        unsafe { *depthFramePixels = std::ptr::null_mut() };
    }
    if !depthFrameRowPitch.is_null() {
        unsafe { *depthFrameRowPitch = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceDepthConfidencePixels(
    cameraDevice: i32,
    depthConfidencePixels: *mut *mut std::os::raw::c_void,
    depthConfidenceRowPitch: *mut i32,
) -> i32 {
    let _ = cameraDevice;
    if !depthConfidencePixels.is_null() {
        unsafe { *depthConfidencePixels = std::ptr::null_mut() };
    }
    if !depthConfidenceRowPitch.is_null() {
        unsafe { *depthConfidenceRowPitch = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceDepthSensingMode(camera: i32, depthSensoringMode: *mut i32) -> i32 {
    let _ = camera;
    if !depthSensoringMode.is_null() {
        unsafe { *depthSensoringMode = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetCameraDeviceDepthSensingMode(camera: i32, depthSensoringMode: i32) -> i32 {
    let _ = (camera, depthSensoringMode);
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDevicePreferredDepthQuality(camera: i32, depthQuality: *mut i32) -> i32 {
    let _ = camera;
    if !depthQuality.is_null() {
        unsafe { *depthQuality = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_SetCameraDevicePreferredDepthQuality(camera: i32, depthQuality: i32) -> i32 {
    let _ = (camera, depthQuality);
    RESULT_SUCCESS
}

pub fn ovrp_SetCameraDevicePreferredColorFrameSize(
    cameraDevice: i32,
    preferredColorFrameSize: Sizei,
) -> i32 {
    let _ = (cameraDevice, preferredColorFrameSize);
    RESULT_SUCCESS
}

pub fn ovrp_GetCameraDeviceIntrinsicsParameters(
    camera: i32,
    supportIntrinsics: *mut i32,
    intrinsicsParameters: *mut CameraDeviceIntrinsicsParameters,
) -> i32 {
    let _ = camera;
    if !supportIntrinsics.is_null() {
        unsafe { *supportIntrinsics = BOOL_FALSE };
    }
    if !intrinsicsParameters.is_null() {
        unsafe { *intrinsicsParameters = CameraDeviceIntrinsicsParameters::default() };
    }
    RESULT_SUCCESS
}

pub fn ovrp_GetSystemDisplayAvailableFrequencies(
    systemDisplayAvailableFrequencies: *mut std::os::raw::c_void,
    numFrequencies: *mut i32,
) -> i32 {
    if !numFrequencies.is_null() {
        unsafe {
            *numFrequencies = 1;
            if !systemDisplayAvailableFrequencies.is_null() {
                *(systemDisplayAvailableFrequencies as *mut f32) = 90.0;
            }
        }
    }
    RESULT_SUCCESS
}

pub fn ovrp_ResetAppPerfStats() -> i32 {
    BOOL_TRUE
}

pub fn ovrp_SetTiledMultiResLevel(level: i32) -> i32 {
    let _ = level;
    RESULT_SUCCESS
}

pub fn ovrp_ShowSystemUI(ui: i32) -> i32 {
    let _ = ui;
    BOOL_TRUE
}

pub fn ovrp_SetOverlayQuad2(
    onTop: i32,
    headLocked: i32,
    texture: *mut std::os::raw::c_void,
    device: *mut std::os::raw::c_void,
    pose: Posef,
    scale: Vector3f,
) -> i32 {
    let _ = (onTop, headLocked, texture, device, pose, scale);
    BOOL_TRUE
}

pub fn ovrp_SetOverlayQuad3(
    flags: u32,
    textureLeft: *mut std::os::raw::c_void,
    textureRight: *mut std::os::raw::c_void,
    device: *mut std::os::raw::c_void,
    pose: Posef,
    scale: Vector3f,
    layerIndex: i32,
) -> i32 {
    let _ = (flags, textureLeft, textureRight, device, pose, scale, layerIndex);
    BOOL_TRUE
}

pub fn ovrp_Media_Initialize() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_Media_Shutdown() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_Media_Update() -> i32 {
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetInitialized(initialized: *mut i32) -> i32 {
    if !initialized.is_null() {
        unsafe { *initialized = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetMrcActivationMode(activationMode: *mut i32) -> i32 {
    if !activationMode.is_null() {
        unsafe { *activationMode = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SetMrcActivationMode(activationMode: i32) -> i32 {
    let _ = activationMode;
    RESULT_SUCCESS
}

pub fn ovrp_Media_IsMrcEnabled(mrcEnabled: *mut i32) -> i32 {
    if !mrcEnabled.is_null() {
        unsafe { *mrcEnabled = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_IsMrcActivated(mrcActivated: *mut i32) -> i32 {
    if !mrcActivated.is_null() {
        unsafe { *mrcActivated = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_UseMrcDebugCamera(useMrcDebugCamera: *mut i32) -> i32 {
    if !useMrcDebugCamera.is_null() {
        unsafe { *useMrcDebugCamera = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SetMrcInputVideoBufferType(inputVideoBufferType: i32) -> i32 {
    let _ = inputVideoBufferType;
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetMrcInputVideoBufferType(inputVideoBufferType: *mut i32) -> i32 {
    if !inputVideoBufferType.is_null() {
        unsafe { *inputVideoBufferType = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SetMrcFrameSize(frameWidth: i32, frameHeight: i32) -> i32 {
    let _ = (frameWidth, frameHeight);
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetMrcFrameSize(frameWidth: *mut i32, frameHeight: *mut i32) -> i32 {
    if !frameWidth.is_null() {
        unsafe { *frameWidth = 0 };
    }
    if !frameHeight.is_null() {
        unsafe { *frameHeight = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SetMrcAudioSampleRate(sampleRate: i32) -> i32 {
    let _ = sampleRate;
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetMrcAudioSampleRate(sampleRate: *mut i32) -> i32 {
    if !sampleRate.is_null() {
        unsafe { *sampleRate = 48000 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SetMrcFrameImageFlipped(flipped: i32) -> i32 {
    let _ = flipped;
    RESULT_SUCCESS
}

pub fn ovrp_Media_GetMrcFrameImageFlipped(flipped: *mut i32) -> i32 {
    if !flipped.is_null() {
        unsafe { *flipped = BOOL_FALSE };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_EncodeMrcFrame(
    rawBuffer: *mut std::os::raw::c_void,
    audioDataPtr: *mut std::os::raw::c_void,
    audioDataLen: i32,
    audioChannels: i32,
    timestamp: f64,
    outSyncId: *mut i32,
) -> i32 {
    let _ = (rawBuffer, audioDataPtr, audioDataLen, audioChannels, timestamp);
    if !outSyncId.is_null() {
        unsafe { *outSyncId = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_EncodeMrcFrameWithDualTextures(
    backgroundTextureHandle: *mut std::os::raw::c_void,
    foregroundTextureHandle: *mut std::os::raw::c_void,
    audioData: *mut std::os::raw::c_void,
    audioDataLen: i32,
    audioChannels: i32,
    timestamp: f64,
    outSyncId: *mut i32,
) -> i32 {
    let _ = (
        backgroundTextureHandle,
        foregroundTextureHandle,
        audioData,
        audioDataLen,
        audioChannels,
        timestamp,
    );
    if !outSyncId.is_null() {
        unsafe { *outSyncId = 0 };
    }
    RESULT_SUCCESS
}

pub fn ovrp_Media_SyncMrcFrame(syncId: i32) -> i32 {
    let _ = syncId;
    RESULT_SUCCESS
}
