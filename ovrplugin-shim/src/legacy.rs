// Names the C# wrapper never declares; kept so the export table matches the real plugin.
#[no_mangle]
pub extern "C" fn ovrp_AutoThreadScheduling() -> i32 {
    crate::trace_call("ovrp_AutoThreadScheduling");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_BeginFrame");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame2() -> i32 {
    crate::trace_call("ovrp_BeginFrame2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame3() -> i32 {
    crate::trace_call("ovrp_BeginFrame3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_BeginFrame4() -> i32 {
    crate::trace_call("ovrp_BeginFrame4");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc() -> i32 {
    crate::trace_call("ovrp_CalculateEyeLayerDesc");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeLayerDesc2() -> i32 {
    crate::trace_call("ovrp_CalculateEyeLayerDesc2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyePreviewRect() -> i32 {
    crate::trace_call("ovrp_CalculateEyePreviewRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_CalculateEyeViewportRect() -> i32 {
    crate::trace_call("ovrp_CalculateEyeViewportRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow() -> i32 {
    crate::trace_call("ovrp_DestroyDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_DestroyDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyEyeTexture() -> i32 {
    crate::trace_call("ovrp_DestroyEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyLayer() -> i32 {
    crate::trace_call("ovrp_DestroyLayer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture() -> i32 {
    crate::trace_call("ovrp_DestroyMirrorTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DestroyMirrorTexture2() -> i32 {
    crate::trace_call("ovrp_DestroyMirrorTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_DismissHSW() -> i32 {
    crate::trace_call("ovrp_DismissHSW");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye() -> i32 {
    crate::trace_call("ovrp_EndEye");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndEye2() -> i32 {
    crate::trace_call("ovrp_EndEye2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_EndFrame");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame2() -> i32 {
    crate::trace_call("ovrp_EndFrame2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame3() -> i32 {
    crate::trace_call("ovrp_EndFrame3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EndFrame4() -> i32 {
    crate::trace_call("ovrp_EndFrame4");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAllCameraDevices() -> i32 {
    crate::trace_call("ovrp_EnumerateAllCameraDevices");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_EnumerateAvailableCameraDevices() -> i32 {
    crate::trace_call("ovrp_EnumerateAvailableCameraDevices");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWAdaptiveMode() -> i32 {
    crate::trace_call("ovrp_GetASWAdaptiveMode");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWDepthScale() -> i32 {
    crate::trace_call("ovrp_GetASWDepthScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWEnable() -> i32 {
    crate::trace_call("ovrp_GetASWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetASWVelocityScale() -> i32 {
    crate::trace_call("ovrp_GetASWVelocityScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetActiveController2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetActiveController2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdapterId() -> i32 {
    crate::trace_call("ovrp_GetAdapterId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale() -> i32 {
    crate::trace_call("ovrp_GetAdaptiveGpuPerformanceScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAdaptiveGpuPerformanceScale2() -> i32 {
    crate::trace_call("ovrp_GetAdaptiveGpuPerformanceScale2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority() -> i32 {
    crate::trace_call("ovrp_GetAppCPUPriority");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCPUPriority2() -> i32 {
    crate::trace_call("ovrp_GetAppCPUPriority2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppCpuStartToGpuEndTime2() -> i32 {
    crate::trace_call("ovrp_GetAppCpuStartToGpuEndTime2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppFramerate2() -> i32 {
    crate::trace_call("ovrp_GetAppFramerate2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasSystemOverlayPresent() -> i32 {
    crate::trace_call("ovrp_GetAppHasSystemOverlayPresent");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppHasVrFocus2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppHasVrFocus2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppLatencyTimings2() -> i32 {
    crate::trace_call("ovrp_GetAppLatencyTimings2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppPerfStats2() -> i32 {
    crate::trace_call("ovrp_GetAppPerfStats2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppSRGB() -> i32 {
    crate::trace_call("ovrp_GetAppSRGB");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldQuit2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppShouldQuit2");
    unsafe {
        if !a0.is_null() {
            *a0 = 0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecenter2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecenter2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow() -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecreateDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAppShouldRecreateDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_GetAppShouldRecreateDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId() -> i32 {
    crate::trace_call("ovrp_GetAudioInDeviceId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInDeviceId2() -> i32 {
    crate::trace_call("ovrp_GetAudioInDeviceId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioInId2() -> i32 {
    crate::trace_call("ovrp_GetAudioInId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId() -> i32 {
    crate::trace_call("ovrp_GetAudioOutDeviceId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutDeviceId2() -> i32 {
    crate::trace_call("ovrp_GetAudioOutDeviceId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetAudioOutId2() -> i32 {
    crate::trace_call("ovrp_GetAudioOutId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBatteryStatus() -> i32 {
    crate::trace_call("ovrp_GetBatteryStatus");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryConfigured2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryConfigured2");
    unsafe {
        if !a0.is_null() {
            *a0 = 0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryDimensions2() -> i32 {
    crate::trace_call("ovrp_GetBoundaryDimensions2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryGeometry3() -> i32 {
    crate::trace_call("ovrp_GetBoundaryGeometry3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBoundaryVisible2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetBoundaryVisible2");
    unsafe {
        if !a0.is_null() {
            *a0 = 0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetBufferCount() -> i32 {
    crate::trace_call("ovrp_GetBufferCount");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps() -> i32 {
    crate::trace_call("ovrp_GetCaps");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetCaps2() -> i32 {
    crate::trace_call("ovrp_GetCaps2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetConnectedControllers2() -> i32 {
    crate::trace_call("ovrp_GetConnectedControllers2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsDesc2() -> i32 {
    crate::trace_call("ovrp_GetControllerHapticsDesc2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerHapticsState2() -> i32 {
    crate::trace_call("ovrp_GetControllerHapticsState2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetControllerState3() -> i32 {
    crate::trace_call("ovrp_GetControllerState3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDepthCompositingSupported() -> i32 {
    crate::trace_call("ovrp_GetDepthCompositingSupported");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDeviceExtensionsVk() -> i32 {
    crate::trace_call("ovrp_GetDeviceExtensionsVk");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId() -> i32 {
    crate::trace_call("ovrp_GetDisplayAdapterId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetDisplayAdapterId2() -> i32 {
    crate::trace_call("ovrp_GetDisplayAdapterId2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetExternalCameraName() -> i32 {
    crate::trace_call("ovrp_GetExternalCameraName");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeAcceleration() -> i32 {
    crate::trace_call("ovrp_GetEyeAcceleration");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFovLayerId() -> i32 {
    crate::trace_call("ovrp_GetEyeFovLayerId");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeFrustum() -> i32 {
    crate::trace_call("ovrp_GetEyeFrustum");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeOcclusionMesh() -> i32 {
    crate::trace_call("ovrp_GetEyeOcclusionMesh");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter() -> i32 {
    crate::trace_call("ovrp_GetEyePixelsPerTanAngleAtCenter");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePixelsPerTanAngleAtCenter2() -> i32 {
    crate::trace_call("ovrp_GetEyePixelsPerTanAngleAtCenter2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePose(ret: *mut u8, _eye: i32) -> *mut u8 {
    crate::trace_call("ovrp_GetEyePose");
    unsafe {
        if !ret.is_null() {
            std::ptr::write_bytes(ret, 0, 28);
            let p = ret as *mut f32;
            p.add(3).write(1.0);
        }
    }
    ret
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyePreviewRect() -> i32 {
    crate::trace_call("ovrp_GetEyePreviewRect");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureArraySupported");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureArraySupported2() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureArraySupported2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureFlippedY() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureFlippedY");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureShared() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureShared");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeTextureStageCount() -> i32 {
    crate::trace_call("ovrp_GetEyeTextureStageCount");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeVelocity() -> i32 {
    crate::trace_call("ovrp_GetEyeVelocity");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetEyeViewportScale() -> i32 {
    crate::trace_call("ovrp_GetEyeViewportScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetFloat(a0: i32) -> f32 {
    crate::trace_call("ovrp_GetFloat");
    0.0f32
}

#[no_mangle]
pub extern "C" fn ovrp_GetGPUFrameTime() -> i32 {
    crate::trace_call("ovrp_GetGPUFrameTime");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHeadphonesPresent() -> i32 {
    crate::trace_call("ovrp_GetHeadphonesPresent");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHmdToEyeOffset() -> i32 {
    crate::trace_call("ovrp_GetHmdToEyeOffset");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetHmdToEyeOffset2() -> i32 {
    crate::trace_call("ovrp_GetHmdToEyeOffset2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetInputState() -> i32 {
    crate::trace_call("ovrp_GetInputState");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetInstanceExtensionsVk() -> i32 {
    crate::trace_call("ovrp_GetInstanceExtensionsVk");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerOcclusionMesh() -> i32 {
    crate::trace_call("ovrp_GetLayerOcclusionMesh");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture() -> i32 {
    crate::trace_call("ovrp_GetLayerTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTexture2() -> i32 {
    crate::trace_call("ovrp_GetLayerTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetLayerTextureFoveation() -> i32 {
    crate::trace_call("ovrp_GetLayerTextureFoveation");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativePointer() -> i32 {
    crate::trace_call("ovrp_GetNativePointer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer() -> *mut std::os::raw::c_void {
    crate::trace_call("ovrp_GetNativeSDKPointer");
    static mut DUMMY: u64 = 1;
    unsafe { std::ptr::addr_of_mut!(DUMMY) as *mut std::os::raw::c_void }
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKPointer2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetNativeSDKPointer2");
    static mut DUMMY: u64 = 1;
    unsafe {
        if !a0.is_null() {
            *a0 = std::ptr::addr_of_mut!(DUMMY) as *mut std::os::raw::c_void;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNativeSDKVersion2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetNativeSDKVersion2");
    unsafe {
        if !a0.is_null() {
            static V: &[u8] = b"1.43.0\0";
            *a0 = V.as_ptr() as *mut std::os::raw::c_void;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodeOrientationTracked2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodeOrientationTracked2");
    unsafe {
        if !a1.is_null() {
            *a1 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState2(_step: i32, _frame: i32, _node: i32, out: *mut u8) -> i32 {
    crate::trace_call("ovrp_GetNodePoseState2");
    if out.is_null() {
        return -1001i32;
    }
    unsafe {
        std::ptr::write_bytes(out, 0, 96);
        let p = out as *mut f32;
        p.add(3).write(1.0);
        p.add(7).write(1.0);
        p.add(5).write(1.6);
        let d = out as *mut f64;
        *d = crate::now_sec();
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePoseState3(_step: i32, _frame: i32, _node: i32, out: *mut u8) -> i32 {
    crate::trace_call("ovrp_GetNodePoseState3");
    if out.is_null() {
        return -1001i32;
    }
    unsafe {
        std::ptr::write_bytes(out, 0, 96);
        let p = out as *mut f32;
        p.add(3).write(1.0);
        p.add(7).write(1.0);
        p.add(5).write(1.6);
        let d = out as *mut f64;
        *d = crate::now_sec();
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePositionTracked2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodePositionTracked2");
    unsafe {
        if !a1.is_null() {
            *a1 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetNodePresent2(a0: i32, a1: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetNodePresent2");
    unsafe {
        if !a1.is_null() {
            *a1 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWEnable() -> i32 {
    crate::trace_call("ovrp_GetPTWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPTWNear() -> i32 {
    crate::trace_call("ovrp_GetPTWNear");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetPredictedDisplayTime() -> f64 {
    crate::trace_call("ovrp_GetPredictedDisplayTime");
    crate::now_sec() + 0.011
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus() -> i32 {
    crate::trace_call("ovrp_GetStatus");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetStatus2() -> i32 {
    crate::trace_call("ovrp_GetStatus2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetString() -> i32 {
    crate::trace_call("ovrp_GetString");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryStatus2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryStatus2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemBatteryTemperature2() -> i32 {
    crate::trace_call("ovrp_GetSystemBatteryTemperature2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemCpuLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemCpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemGpuLevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemGpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadphonesPresent2() -> i32 {
    crate::trace_call("ovrp_GetSystemHeadphonesPresent2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemHeadsetType2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetSystemHeadsetType2");
    unsafe {
        if !a0.is_null() {
            *a0 = 8;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported() -> i32 {
    crate::trace_call("ovrp_GetSystemMultiViewSupported");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemMultiViewSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetSystemMultiViewSupported2");
    unsafe {
        if !a0.is_null() {
            *a0 = 0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemPowerSavingMode2() -> i32 {
    crate::trace_call("ovrp_GetSystemPowerSavingMode2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemProductName2() -> i32 {
    crate::trace_call("ovrp_GetSystemProductName2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRecommendedMSAALevel2() -> i32 {
    crate::trace_call("ovrp_GetSystemRecommendedMSAALevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemRegion2() -> i32 {
    crate::trace_call("ovrp_GetSystemRegion2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVSyncCount2() -> i32 {
    crate::trace_call("ovrp_GetSystemVSyncCount2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetSystemVolume2(a0: *mut f32) -> i32 {
    crate::trace_call("ovrp_GetSystemVolume2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1.0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerFrustum() -> i32 {
    crate::trace_call("ovrp_GetTrackerFrustum");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackerPose() -> i32 {
    crate::trace_call("ovrp_GetTrackerPose");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingCalibratedOrigin2() -> i32 {
    crate::trace_call("ovrp_GetTrackingCalibratedOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingIPDEnabled2() -> i32 {
    crate::trace_call("ovrp_GetTrackingIPDEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationEnabled2() -> i32 {
    crate::trace_call("ovrp_GetTrackingOrientationEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOrientationSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingOrientationSupported2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingOriginType2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingOriginType2");
    unsafe {
        if !a0.is_null() {
            *a0 = 0;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionEnabled2() -> i32 {
    crate::trace_call("ovrp_GetTrackingPositionEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetTrackingPositionSupported2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetTrackingPositionSupported2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserEyeHeight2() -> i32 {
    crate::trace_call("ovrp_GetUserEyeHeight2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserIPD2() -> i32 {
    crate::trace_call("ovrp_GetUserIPD2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance() -> i32 {
    crate::trace_call("ovrp_GetUserNeckEyeDistance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserNeckEyeDistance2() -> i32 {
    crate::trace_call("ovrp_GetUserNeckEyeDistance2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetUserPresent2(a0: *mut i32) -> i32 {
    crate::trace_call("ovrp_GetUserPresent2");
    unsafe {
        if !a0.is_null() {
            *a0 = 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVersion2(a0: *mut *mut std::os::raw::c_void) -> i32 {
    crate::trace_call("ovrp_GetVersion2");
    unsafe {
        if !a0.is_null() {
            static V: &[u8] = b"1.43.0\0";
            *a0 = V.as_ptr() as *mut std::os::raw::c_void;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetViewportStencil() -> i32 {
    crate::trace_call("ovrp_GetViewportStencil");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_GetVrApiPropertyInt() -> i32 {
    crate::trace_call("ovrp_GetVrApiPropertyInt");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_HasCameraDeviceOpened2() -> i32 {
    crate::trace_call("ovrp_HasCameraDeviceOpened2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize() -> i32 {
    crate::trace_call("ovrp_Initialize");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize2() -> i32 {
    crate::trace_call("ovrp_Initialize2");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize3() -> i32 {
    crate::trace_call("ovrp_Initialize3");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize4() -> i32 {
    crate::trace_call("ovrp_Initialize4");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_Initialize5() -> i32 {
    crate::trace_call("ovrp_Initialize5");
    crate::compat_initialize()
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceAvailable2() -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceAvailable2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsCameraDeviceColorFrameAvailable2() -> i32 {
    crate::trace_call("ovrp_IsCameraDeviceColorFrameAvailable2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_IsRequestingASWData() -> i32 {
    crate::trace_call("ovrp_IsRequestingASWData");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_GetMrcFrameInverseAlpha() -> i32 {
    crate::trace_call("ovrp_Media_GetMrcFrameInverseAlpha");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetAvailableQueueIndexVulkan() -> i32 {
    crate::trace_call("ovrp_Media_SetAvailableQueueIndexVulkan");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Media_SetMrcFrameInverseAlpha() -> i32 {
    crate::trace_call("ovrp_Media_SetMrcFrameInverseAlpha");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize() -> i32 {
    crate::trace_call("ovrp_PreInitialize");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize2() -> i32 {
    crate::trace_call("ovrp_PreInitialize2");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_PreInitialize3() -> i32 {
    crate::trace_call("ovrp_PreInitialize3");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterPose() -> i32 {
    crate::trace_call("ovrp_RecenterPose");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecenterTrackingOrigin2() -> i32 {
    crate::trace_call("ovrp_RecenterTrackingOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_RecreateEyeTexture() -> i32 {
    crate::trace_call("ovrp_RecreateEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ReleaseEyeTexture() -> i32 {
    crate::trace_call("ovrp_ReleaseEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ResetAppPerfStats2() -> i32 {
    crate::trace_call("ovrp_ResetAppPerfStats2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWAdaptiveMode() -> i32 {
    crate::trace_call("ovrp_SetASWAdaptiveMode");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetASWEnable() -> i32 {
    crate::trace_call("ovrp_SetASWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppAsymmetricFov() -> i32 {
    crate::trace_call("ovrp_SetAppAsymmetricFov");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority() -> i32 {
    crate::trace_call("ovrp_SetAppCPUPriority");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppCPUPriority2() -> i32 {
    crate::trace_call("ovrp_SetAppCPUPriority2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo() -> i32 {
    crate::trace_call("ovrp_SetAppEngineInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppEngineInfo2() -> i32 {
    crate::trace_call("ovrp_SetAppEngineInfo2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppIgnoreVrFocus() -> i32 {
    crate::trace_call("ovrp_SetAppIgnoreVrFocus");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetAppSRGB() -> i32 {
    crate::trace_call("ovrp_SetAppSRGB");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetBoundaryVisible2() -> i32 {
    crate::trace_call("ovrp_SetBoundaryVisible2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetCaps() -> i32 {
    crate::trace_call("ovrp_SetCaps");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerHaptics2() -> i32 {
    crate::trace_call("ovrp_SetControllerHaptics2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetControllerVibration2() -> i32 {
    crate::trace_call("ovrp_SetControllerVibration2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthCompositingInfo() -> i32 {
    crate::trace_call("ovrp_SetDepthCompositingInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetDepthProjInfo() -> i32 {
    crate::trace_call("ovrp_SetDepthProjInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTexture() -> i32 {
    crate::trace_call("ovrp_SetEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureArrayEnabled() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureArrayEnabled");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureFlippedY() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureFlippedY");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeTextureShared() -> i32 {
    crate::trace_call("ovrp_SetEyeTextureShared");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetEyeViewportScale() -> i32 {
    crate::trace_call("ovrp_SetEyeViewportScale");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetFloat() -> i32 {
    crate::trace_call("ovrp_SetFloat");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX() -> i32 {
    crate::trace_call("ovrp_SetInhibitSystemUX");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetInhibitSystemUX2() -> i32 {
    crate::trace_call("ovrp_SetInhibitSystemUX2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked() -> i32 {
    crate::trace_call("ovrp_SetNodePositionTracked");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetNodePositionTracked2() -> i32 {
    crate::trace_call("ovrp_SetNodePositionTracked2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOctilinearInfo() -> i32 {
    crate::trace_call("ovrp_SetOctilinearInfo");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetOverlayQuad() -> i32 {
    crate::trace_call("ovrp_SetOverlayQuad");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetPTWEnable() -> i32 {
    crate::trace_call("ovrp_SetPTWEnable");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetRemoteHandedness() -> i32 {
    crate::trace_call("ovrp_SetRemoteHandedness");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemCpuLevel2() -> i32 {
    crate::trace_call("ovrp_SetSystemCpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemGpuLevel2() -> i32 {
    crate::trace_call("ovrp_SetSystemGpuLevel2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetSystemVSyncCount2() -> i32 {
    crate::trace_call("ovrp_SetSystemVSyncCount2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetThreadPerformance() -> i32 {
    crate::trace_call("ovrp_SetThreadPerformance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingCalibratedOrigin2() -> i32 {
    crate::trace_call("ovrp_SetTrackingCalibratedOrigin2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingIPDEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingIPDEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOrientationEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingOrientationEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingOriginType2() -> i32 {
    crate::trace_call("ovrp_SetTrackingOriginType2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetTrackingPositionEnabled2() -> i32 {
    crate::trace_call("ovrp_SetTrackingPositionEnabled2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserEyeHeight2() -> i32 {
    crate::trace_call("ovrp_SetUserEyeHeight2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserIPD2() -> i32 {
    crate::trace_call("ovrp_SetUserIPD2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance() -> i32 {
    crate::trace_call("ovrp_SetUserNeckEyeDistance");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetUserNeckEyeDistance2() -> i32 {
    crate::trace_call("ovrp_SetUserNeckEyeDistance2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyFloat() -> i32 {
    crate::trace_call("ovrp_SetVrApiPropertyFloat");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetVrApiPropertyInt() -> i32 {
    crate::trace_call("ovrp_SetVrApiPropertyInt");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects() -> i32 {
    crate::trace_call("ovrp_SetupDisplayObjects");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDisplayObjects2() -> i32 {
    crate::trace_call("ovrp_SetupDisplayObjects2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow2() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupDistortionWindow3() -> i32 {
    crate::trace_call("ovrp_SetupDistortionWindow3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture() -> i32 {
    crate::trace_call("ovrp_SetupEyeTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupEyeTexture2() -> i32 {
    crate::trace_call("ovrp_SetupEyeTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayer() -> i32 {
    crate::trace_call("ovrp_SetupLayer");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupLayerDepth() -> i32 {
    crate::trace_call("ovrp_SetupLayerDepth");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture() -> i32 {
    crate::trace_call("ovrp_SetupMirrorTexture");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_SetupMirrorTexture2() -> i32 {
    crate::trace_call("ovrp_SetupMirrorTexture2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowSystemUI2() -> i32 {
    crate::trace_call("ovrp_ShowSystemUI2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_ShowUI() -> i32 {
    crate::trace_call("ovrp_ShowUI");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown() -> i32 {
    crate::trace_call("ovrp_Shutdown");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Shutdown2() -> i32 {
    crate::trace_call("ovrp_Shutdown2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryNode2() -> i32 {
    crate::trace_call("ovrp_TestBoundaryNode2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_TestBoundaryPoint2() -> i32 {
    crate::trace_call("ovrp_TestBoundaryPoint2");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_Update(a0: i32) -> i32 {
    crate::trace_call("ovrp_Update");
    1
}

#[no_mangle]
pub extern "C" fn ovrp_Update3(a0: i32, a1: i32, a2: f64) -> i32 {
    crate::trace_call("ovrp_Update3");
    0
}

#[no_mangle]
pub extern "C" fn ovrp_WaitToBeginFrame(a0: i32) -> i32 {
    crate::trace_call("ovrp_WaitToBeginFrame");
    0
}

#[export_name = "?ovrp_SetFunctionPointer@@YA?AW4ovrpResult@@W4ovrpFunctionType@@PEAX@Z"]
pub extern "C" fn ovrp_set_function_pointer_alias(
    a0: i32,
    a1: *mut std::os::raw::c_void,
) -> i32 {
    crate::trace_call("ovrp_set_function_pointer_alias");
    0
}

