#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <X11/Xlib.h>
#include <X11/Xatom.h>

/* Bind dlsym to base GLIBC version for maximum cross-distro compatibility (Debian, Ubuntu, Fedora, Arch) */
#if defined(__x86_64__)
__asm__(".symver dlsym,dlsym@GLIBC_2.2.5");
#endif

typedef int (*pfn_XGetWindowProperty)(
    Display *, Window, Atom, long, long, Bool, Atom,
    Atom *, int *, unsigned long *, unsigned long *, unsigned char **
);
typedef Atom (*pfn_XInternAtom)(Display *, const char *, Bool);
typedef Status (*pfn_XReconfigureWMWindow)(Display *, Window, int, unsigned int, XWindowChanges *);
typedef Status (*pfn_XGetWindowAttributes)(Display *, Window, XWindowAttributes *);
typedef int (*pfn_XConfigureWindow)(Display *, Window, unsigned int, XWindowChanges *);
typedef int (*pfn_XRaiseWindow)(Display *, Window);
typedef Bool (*pfn_XCheckIfEvent)(Display *, XEvent *, Bool (*)(Display *, XEvent *, XPointer), XPointer);

int XGetWindowProperty(
    Display *display,
    Window w,
    Atom property,
    long long_offset,
    long long_length,
    Bool delete_prop,
    Atom req_type,
    Atom *actual_type_return,
    int *actual_format_return,
    unsigned long *nitems_return,
    unsigned long *bytes_after_return,
    unsigned char **prop_return)
{
    static pfn_XGetWindowProperty real_XGetWindowProperty = NULL;
    static pfn_XInternAtom real_XInternAtom = NULL;

    if (!real_XGetWindowProperty)
        real_XGetWindowProperty = (pfn_XGetWindowProperty)dlsym(RTLD_NEXT, "XGetWindowProperty");
    if (!real_XInternAtom)
        real_XInternAtom = (pfn_XInternAtom)dlsym(RTLD_NEXT, "XInternAtom");

    int ret = real_XGetWindowProperty(
        display, w, property, long_offset, long_length, delete_prop, req_type,
        actual_type_return, actual_format_return, nitems_return, bytes_after_return, prop_return
    );

    /* Strip _NET_WM_MOVERESIZE from _NET_SUPPORTED so Wine's X11DRV_SysCommand
     * falls back to Win32 internal sys_command_size_move loop across ALL Wine versions,
     * enabling continuous WM_MOVING events required by Roblox Studio's QtitanDocking. */
    if (ret == Success && display && real_XInternAtom &&
        prop_return && *prop_return &&
        actual_format_return && *actual_format_return == 32 &&
        nitems_return && *nitems_return > 0)
    {
        Atom net_supported = real_XInternAtom(display, "_NET_SUPPORTED", False);
        if (property == net_supported) {
            Atom net_wm_moveresize = real_XInternAtom(display, "_NET_WM_MOVERESIZE", False);
            Atom *atoms = (Atom *)*prop_return;
            for (unsigned long i = 0; i < *nitems_return; i++) {
                if (atoms[i] == net_wm_moveresize) {
                    atoms[i] = None;
                    static int logged = 0;
                    if (!logged) {
                        logged = 1;
                        fprintf(stderr, "[Cutlery] Universal X11 Docking Hook active (_NET_WM_MOVERESIZE bypassed)\n");
                    }
                }
            }
        }
    }

    return ret;
}

Status XReconfigureWMWindow(
    Display *display,
    Window w,
    int screen_number,
    unsigned int value_mask,
    XWindowChanges *values)
{
    static pfn_XReconfigureWMWindow real_XReconfigureWMWindow = NULL;
    static pfn_XGetWindowAttributes real_XGetWindowAttributes = NULL;
    static pfn_XConfigureWindow real_XConfigureWindow = NULL;
    static pfn_XRaiseWindow real_XRaiseWindow = NULL;

    if (!real_XReconfigureWMWindow)
        real_XReconfigureWMWindow = (pfn_XReconfigureWMWindow)dlsym(RTLD_NEXT, "XReconfigureWMWindow");
    if (!real_XGetWindowAttributes)
        real_XGetWindowAttributes = (pfn_XGetWindowAttributes)dlsym(RTLD_NEXT, "XGetWindowAttributes");
    if (!real_XConfigureWindow)
        real_XConfigureWindow = (pfn_XConfigureWindow)dlsym(RTLD_NEXT, "XConfigureWindow");
    if (!real_XRaiseWindow)
        real_XRaiseWindow = (pfn_XRaiseWindow)dlsym(RTLD_NEXT, "XRaiseWindow");

    /* For unmanaged / override-redirect windows (such as QtitanDocking guide overlays),
     * window managers ignore synthetic ConfigureRequest events from XReconfigureWMWindow.
     * Apply geometry and stacking directly via XConfigureWindow + XRaiseWindow. */
    if (display && values && real_XGetWindowAttributes && real_XConfigureWindow) {
        XWindowAttributes attr;
        if (real_XGetWindowAttributes(display, w, &attr) && attr.override_redirect) {
            int ret = real_XConfigureWindow(display, w, value_mask, values);
            if ((value_mask & CWStackMode) && values->stack_mode == Above && real_XRaiseWindow) {
                real_XRaiseWindow(display, w);
            }
            return ret;
        }
    }

    return real_XReconfigureWMWindow(display, w, screen_number, value_mask, values);
}

Bool XCheckIfEvent(
    Display *display,
    XEvent *event_return,
    Bool (*predicate)(Display *, XEvent *, XPointer),
    XPointer arg)
{
    static pfn_XCheckIfEvent real_XCheckIfEvent = NULL;
    if (!real_XCheckIfEvent)
        real_XCheckIfEvent = (pfn_XCheckIfEvent)dlsym(RTLD_NEXT, "XCheckIfEvent");

    while (real_XCheckIfEvent(display, event_return, predicate, arg)) {
        /* When a mouse button is held down during a drag (e.g. moving an undocked
         * Studio panel), ignore EnterNotify events on newly mapped overlay windows
         * so QtitanDocking guide diamonds do not steal or abort the drag capture. */
        if (event_return && event_return->type == EnterNotify &&
            (event_return->xcrossing.state & (Button1Mask | Button2Mask | Button3Mask)))
        {
            continue;
        }
        return True;
    }
    return False;
}
