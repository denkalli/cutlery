#define _GNU_SOURCE
#include <dlfcn.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

static void *(*real_dlopen)(const char *, int) = NULL;

void *dlopen(const char *filename, int flags) {
    if (!real_dlopen) {
        real_dlopen = dlsym(RTLD_NEXT, "dlopen");
    }
    if (filename && strstr(filename, "winex11.so")) {
        const char *override_path = getenv("WINEX11_OVERRIDE");
        if (override_path && override_path[0]) {
            fprintf(stderr, "[VinegarFork] LD_PRELOAD redirecting dlopen: %s -> %s\n", filename, override_path);
            return real_dlopen(override_path, flags);
        }
    }
    return real_dlopen(filename, flags);
}
