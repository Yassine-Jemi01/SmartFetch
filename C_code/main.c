#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif
#include <windows.h>
#endif

#include "sysinfo.h"

#ifdef _WIN32
static void enable_console_features(void) {
    SetConsoleOutputCP(CP_UTF8);
    SetConsoleCP(CP_UTF8);
    SetProcessDPIAware();

    HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD mode = 0;

    if (out != INVALID_HANDLE_VALUE && GetConsoleMode(out, &mode)) {
#ifdef ENABLE_VIRTUAL_TERMINAL_PROCESSING
        SetConsoleMode(out, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
#endif
    }
}
#endif

int main(int argc, char *argv[]) {
#ifdef _WIN32
    enable_console_features();
#endif

    if (argc > 1) {
        if (strcmp(argv[1], "--help") == 0 || strcmp(argv[1], "-h") == 0) {
            print_help();
            return 0;
        }
        if (strcmp(argv[1], "--update") == 0 || strcmp(argv[1], "-u") == 0) {
            check_for_update();
            return 0;
        }

        printf("Unknown option: %s\n\n", argv[1]);
        print_help();
        return 1;
    }

    SystemData sys;
    collect_system_data(&sys);
    render_ui(&sys);
    return 0;
}
