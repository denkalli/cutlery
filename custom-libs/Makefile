CC ?= gcc
CFLAGS ?= -O2 -Wall -fPIC -shared
LDFLAGS ?= -lX11 -ldl

all: x86_64 i386

x86_64: wine_hook.c
	@mkdir -p x86_64
	$(CC) $(CFLAGS) wine_hook.c -o x86_64/wine_hook.so $(LDFLAGS)
	@cp x86_64/wine_hook.so wine_hook.so
	@echo "[Cutlery] Built x86_64/wine_hook.so successfully."

i386: wine_hook.c
	@mkdir -p i386
	@$(CC) -m32 $(CFLAGS) wine_hook.c -o i386/wine_hook.so $(LDFLAGS) && echo "[Cutlery] Built i386/wine_hook.so successfully." || echo "[Cutlery] Warning: 32-bit compilation skipped (requires gcc-multilib)."

clean:
	rm -f wine_hook.so x86_64/wine_hook.so i386/wine_hook.so

.PHONY: all x86_64 i386 clean
