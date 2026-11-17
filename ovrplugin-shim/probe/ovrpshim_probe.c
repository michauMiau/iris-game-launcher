// Probe for the OVRPlugin shim: calls the exports a Unity game calls and
// prints what comes back. Built for x86_64-pc-windows-gnu, run under wine.
#include <windows.h>
#include <stdio.h>
#include <stdint.h>

typedef struct { float x, y, z, w; } Quatf;
typedef struct { float x, y, z; } Vector3f;
typedef struct { Quatf Orientation; Vector3f Position; } Posef;
typedef struct { int w, h; } Sizei;
typedef struct {
    unsigned int ConnectedControllers, Buttons, Touches, NearTouches;
    float LIndexTrigger, RIndexTrigger, LHandTrigger, RHandTrigger;
    struct { float x, y; } LThumbstick, RThumbstick;
} ControllerState;
typedef struct {
    unsigned int ConnectedControllers, Buttons, Touches, NearTouches;
    float LIndexTrigger, RIndexTrigger, LHandTrigger, RHandTrigger;
    struct { float x, y; } LThumbstick, RThumbstick, LTouchpad, RTouchpad;
} ControllerState2;

typedef Posef (*FnGetNodePose)(int);
typedef Posef (*FnGetNodeVelocity)(int);
typedef ControllerState (*FnGetControllerState)(unsigned int);
typedef ControllerState2 (*FnGetControllerState2)(unsigned int);
typedef Sizei (*FnGetEyeTextureSize)(int);
typedef long (*FnGetSystemHeadsetType)(void);
typedef long (*FnGetConnectedControllers)(void);
typedef long (*FnGetBoundaryVisible)(void);
typedef long (*FnGetInitialized)(void);
typedef long (*FnGetAppHasVrFocus)(void);
typedef long (*FnGetNodePresent)(int);
typedef long (*FnGetAppShouldQuit)(void);
typedef long (*FnGetSystemDisplayFrequency)(void);
typedef float (*FnGetSystemDisplayFrequencyF)(void);
typedef long (*FnGetSystemBatteryStatus)(void);
typedef float (*FnGetUserIPD)(void);
typedef long (*FnGetTrackingOriginType)(void);
typedef long (*FnGetBoundaryConfigured)(void);
typedef float (*FnGetEyeTextureScale)(void);
typedef const char *(*FnGetVersion)(void);
typedef const char *(*FnGetNativeSDKVersion)(void);
typedef long (*FnGetAppHasInputFocus)(long *);
typedef long (*FnGetTimeInSeconds)(double *);

int main(void) {
    HMODULE h = LoadLibraryW(L"ovrplugin_shim.dll");
    if (!h) { printf("LoadLibrary failed %lu\n", GetLastError()); return 1; }
    printf("loaded %p\n", (void *)h);

    FnGetVersion pver = (FnGetVersion)GetProcAddress(h, "ovrp_GetVersion");
    FnGetNativeSDKVersion psdk = (FnGetNativeSDKVersion)GetProcAddress(h, "ovrp_GetNativeSDKVersion");
    FnGetInitialized pin = (FnGetInitialized)GetProcAddress(h, "ovrp_GetInitialized");
    FnGetNodePose pnp = (FnGetNodePose)GetProcAddress(h, "ovrp_GetNodePose");
    FnGetNodeVelocity pnv = (FnGetNodeVelocity)GetProcAddress(h, "ovrp_GetNodeVelocity");
    FnGetControllerState pcs = (FnGetControllerState)GetProcAddress(h, "ovrp_GetControllerState");
    FnGetControllerState2 pcs2 = (FnGetControllerState2)GetProcAddress(h, "ovrp_GetControllerState2");
    FnGetEyeTextureSize pe = (FnGetEyeTextureSize)GetProcAddress(h, "ovrp_GetEyeTextureSize");
    FnGetSystemHeadsetType ph = (FnGetSystemHeadsetType)GetProcAddress(h, "ovrp_GetSystemHeadsetType");
    FnGetConnectedControllers pc = (FnGetConnectedControllers)GetProcAddress(h, "ovrp_GetConnectedControllers");
    FnGetBoundaryVisible pb = (FnGetBoundaryVisible)GetProcAddress(h, "ovrp_GetBoundaryVisible");
    FnGetAppHasVrFocus pf = (FnGetAppHasVrFocus)GetProcAddress(h, "ovrp_GetAppHasVrFocus");
    FnGetNodePresent pp = (FnGetNodePresent)GetProcAddress(h, "ovrp_GetNodePresent");
    FnGetAppShouldQuit pq = (FnGetAppShouldQuit)GetProcAddress(h, "ovrp_GetAppShouldQuit");
    FnGetSystemDisplayFrequencyF pdf = (FnGetSystemDisplayFrequencyF)GetProcAddress(h, "ovrp_GetSystemDisplayFrequency");
    FnGetSystemBatteryStatus pbs = (FnGetSystemBatteryStatus)GetProcAddress(h, "ovrp_GetSystemBatteryStatus");
    FnGetUserIPD pipd = (FnGetUserIPD)GetProcAddress(h, "ovrp_GetUserIPD");
    FnGetTrackingOriginType pto = (FnGetTrackingOriginType)GetProcAddress(h, "ovrp_GetTrackingOriginType");
    FnGetBoundaryConfigured pbc = (FnGetBoundaryConfigured)GetProcAddress(h, "ovrp_GetBoundaryConfigured");
    FnGetEyeTextureScale pes = (FnGetEyeTextureScale)GetProcAddress(h, "ovrp_GetEyeTextureScale");
    FnGetAppHasInputFocus pif = (FnGetAppHasInputFocus)GetProcAddress(h, "ovrp_GetAppHasInputFocus");
    FnGetTimeInSeconds pti = (FnGetTimeInSeconds)GetProcAddress(h, "ovrp_GetTimeInSeconds");

    if (pver) printf("version=%s\n", pver());
    if (psdk) printf("sdk=%s\n", psdk());
    if (pin) printf("initialized=%ld\n", pin());
    if (pnp) { Posef q = pnp(9); printf("head pose pos=(%.2f %.2f %.2f) quat=(%.2f %.2f %.2f %.2f)\n",
        q.Position.x, q.Position.y, q.Position.z, q.Orientation.x, q.Orientation.y, q.Orientation.z, q.Orientation.w); }
    if (pnv) { Posef v = pnv(9); printf("head vel=(%.2f %.2f %.2f)\n", v.Position.x, v.Position.y, v.Position.z); }
    if (pcs) { ControllerState s = pcs(3); printf("controller connected=%u buttons=%u triggers=(%.1f %.1f)\n",
        s.ConnectedControllers, s.Buttons, s.LIndexTrigger, s.RIndexTrigger); }
    if (pcs2) { ControllerState2 s = pcs2(3); printf("controller2 connected=%u touches=%u\n", s.ConnectedControllers, s.Touches); }
    if (pe) { Sizei z = pe(0); printf("eye size=%dx%d\n", z.w, z.h); }
    if (ph) printf("headset=%ld\n", ph());
    if (pc) printf("connected controllers=%ld\n", pc());
    if (pb) printf("boundary visible=%ld\n", pb());
    if (pf) printf("vr focus=%ld\n", pf());
    if (pp) printf("node present=%ld\n", pp(9));
    if (pq) printf("should quit=%ld\n", pq());
    if (pdf) printf("display freq=%.1f\n", pdf());
    if (pbs) printf("battery status=%ld\n", pbs());
    if (pipd) printf("ipd=%.4f\n", pipd());
    if (pto) printf("tracking origin=%ld\n", pto());
    if (pbc) printf("boundary configured=%ld\n", pbc());
    if (pes) printf("eye texture scale=%.2f\n", pes());
    if (pif) { long f = -1; long rc = pif(&f); printf("input focus rc=%ld value=%ld\n", rc, f); }
    if (pti) { double t = 0; long rc = pti(&t); printf("time rc=%ld value=%.3f\n", rc, t); }
    Sleep(4000);
    printf("PROBE OK\n");
    return 0;
}
