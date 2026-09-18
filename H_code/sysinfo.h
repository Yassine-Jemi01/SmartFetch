#ifndef SYSINFO_H
#define SYSINFO_H

#ifndef SF_VERSION
#define SF_VERSION "unknown"
#endif

#define SF_LINE_COUNT 15
#define SF_LINE_WIDTH 512
#define SF_ASCII_WIDTH 45
#define SF_REPO_URL "https://github.com/Yassine-Jemi01/SmartFetch.git"

typedef struct {
    char os_name[64];
    char kernel[64];
    char shell_info[64];
    char cpu_name[128];
    char cpu_temp[32];
    char ram_total[64];
    char ram_type[32];
    char gpu_type[128];
    char storage_info[512];
    char screen_info[256];
    char os_age[64];
    char os_uptime[64];
    char pkg_manager[64];
    char flatpak_count[32];
    char pkgs_total[64];
} SystemData;


void collect_system_data(SystemData *data);
void render_ui(const SystemData *data);
void print_help(void);
void check_for_update(void);


void print_color_palette(void);
void make_progress_bar(char *out, size_t size, double percentage, int width);

#endif
