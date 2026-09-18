#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <stdint.h>
#include <dirent.h>
#include <limits.h>
#include <unistd.h>
#include <fcntl.h>
#include <time.h>
#include <mntent.h>
#include <sys/utsname.h>
#include <sys/statvfs.h>
#include <sys/stat.h>
#include "sysinfo.h"

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

static void set_default(char *out, size_t size) {
    if (!out || size == 0) return;

    strncpy(out, "N/A", size - 1);
    out[size - 1] = '\0';
}

static void get_os_name(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    FILE *fp = fopen("/etc/os-release", "r");
    if (!fp) return;

    char line[256];

    while (fgets(line, sizeof(line), fp)) {
        if (strncmp(line, "PRETTY_NAME=", 12) == 0) {
            char *value = line + 12;

            value[strcspn(value, "\n")] = '\0';

            size_t len = strlen(value);

            if (len >= 2 &&
                value[0] == '"' &&
                value[len - 1] == '"') {

                value[len - 1] = '\0';
                value++;
            }

            snprintf(out, size, "%s", value);
            break;
        }
    }

    fclose(fp);
}

static void get_kernel(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    struct utsname u;

    if (uname(&u) == 0) {
        snprintf(out, size, "%s", u.release);
    }
}

static void get_cpu_name(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    FILE *fp = fopen("/proc/cpuinfo", "r");
    if (!fp) return;

    char line[256];

    while (fgets(line, sizeof(line), fp)) {
        if (strncmp(line, "model name", 10) == 0) {
            char *colon = strchr(line, ':');

            if (colon) {
                colon++;

                while (*colon == ' ' || *colon == '\t') {
                    colon++;
                }

                colon[strcspn(colon, "\n")] = '\0';

                snprintf(out, size, "%s", colon);
            }

            break;
        }
    }

    fclose(fp);
}

static void get_cpu_temp(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    DIR *d = opendir("/sys/class/thermal");
    if (!d) return;

    char best_path[PATH_MAX] = "";
    struct dirent *entry;

    while ((entry = readdir(d)) != NULL) {
        if (strncmp(entry->d_name, "thermal_zone", 12) != 0) {
            continue;
        }

        char type_path[PATH_MAX];

        snprintf(
            type_path,
            sizeof(type_path),
            "/sys/class/thermal/%s/type",
            entry->d_name
        );

        char type[64] = "";

        FILE *tf = fopen(type_path, "r");

        if (tf) {
            if (fgets(type, sizeof(type), tf)) {
                type[strcspn(type, "\n")] = '\0';
            }

            fclose(tf);
        }

        if (strcasestr(type, "x86_pkg_temp") ||
            strcasestr(type, "cpu")) {

            snprintf(
                best_path,
                sizeof(best_path),
                "/sys/class/thermal/%s/temp",
                entry->d_name
            );

            break;
        }
    }

    closedir(d);

    if (best_path[0] == '\0') {
        return;
    }

    FILE *tf = fopen(best_path, "r");
    if (!tf) return;

    long milli = 0;

    if (fscanf(tf, "%ld", &milli) == 1) {
        snprintf(
            out,
            size,
            "%.1f°C",
            (double)milli / 1000.0
        );
    }

    fclose(tf);
}

static double kb_to_gib(long long kb) {
    return (double)kb / (1024.0 * 1024.0);
}

static const char *usage_color(double percent) {
    if (percent < 50.0) {
        return "\033[1;32m";
    }

    if (percent < 80.0) {
        return "\033[1;33m";
    }

    return "\033[1;31m";
}

static void get_ram_info(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    FILE *fp = fopen("/proc/meminfo", "r");
    if (!fp) return;

    long long mem_total = -1;
    long long mem_available = -1;

    char line[256];

    while (fgets(line, sizeof(line), fp)) {
        if (mem_total < 0 &&
            strncmp(line, "MemTotal:", 9) == 0) {

            sscanf(line + 9, "%lld", &mem_total);

        } else if (
            mem_available < 0 &&
            strncmp(line, "MemAvailable:", 13) == 0
        ) {
            sscanf(
                line + 13,
                "%lld",
                &mem_available
            );
        }

        if (mem_total >= 0 &&
            mem_available >= 0) {
            break;
        }
    }

    fclose(fp);

    if (mem_total <= 0) {
        return;
    }

    if (mem_available >= 0 &&
        mem_available <= mem_total) {

        long long used_kb =
            mem_total - mem_available;

        double percent =
            (double)used_kb /
            (double)mem_total *
            100.0;

        snprintf(
            out,
            size,
            "%.1fGi / %.1fGi (%s%.0f%%\033[0m)",
            kb_to_gib(used_kb),
            kb_to_gib(mem_total),
            usage_color(percent),
            percent
        );

    } else {
        snprintf(
            out,
            size,
            "%.1fGi total",
            kb_to_gib(mem_total)
        );
    }
}

static const char *mem_type_name(uint8_t t) {
    static const char *low_types[] = {
        NULL,
        "Other",
        "Unknown",
        "DRAM",
        "EDRAM",
        "VRAM",
        "SRAM",
        "RAM",
        "ROM",
        "FLASH",
        "EEPROM",
        "FEPROM",
        "EPROM",
        "CDRAM",
        "3DRAM",
        "SDRAM",
        "SGRAM",
        "RDRAM",
        "DDR",
        "DDR2",
        "DDR2 FB-DIMM"
    };

    static const struct {
        uint8_t code;
        const char *name;
    } high_types[] = {
        {0x18, "DDR3"},
        {0x19, "FBD2"},
        {0x1A, "DDR4"},
        {0x1B, "LPDDR"},
        {0x1C, "LPDDR2"},
        {0x1D, "LPDDR3"},
        {0x1E, "LPDDR4"},
        {0x20, "HBM"},
        {0x21, "HBM2"},
        {0x22, "DDR5"},
        {0x23, "LPDDR5"}
    };

    if (t >= 1 && t <= 20) {
        return low_types[t];
    }

    for (
        size_t i = 0;
        i < sizeof(high_types) / sizeof(high_types[0]);
        i++
    ) {
        if (high_types[i].code == t) {
            return high_types[i].name;
        }
    }

    return NULL;
}

static int ram_type_via_dmidecode(
    char *out,
    size_t size
) {
    if (!out || size == 0) return 0;

    FILE *fp = popen(
        "dmidecode -t 17 2>/dev/null",
        "r"
    );

    if (!fp) return 0;

    char line[256];
    int found = 0;

    while (fgets(line, sizeof(line), fp)) {
        char *p = strstr(line, "Type:");

        if (p && !strstr(line, "Type Detail")) {
            p += 5;

            while (*p == ' ' || *p == '\t') {
                p++;
            }

            p[strcspn(p, "\n")] = '\0';

            if (*p &&
                strcmp(p, "Unknown") != 0) {

                snprintf(
                    out,
                    size,
                    "%s",
                    p
                );

                found = 1;
                break;
            }
        }
    }

    pclose(fp);

    return found;
}

static void get_ram_type(char *out, size_t size) {
    set_default(out, size);
    if (!out || size == 0) return;

    /*
     * Try reading the DMI table directly first.
     * This avoids launching dmidecode on every run
     * when the installed capability already allows access.
     */
    FILE *fp = fopen(
        "/sys/firmware/dmi/tables/DMI",
        "rb"
    );

    if (fp) {
        unsigned char *buf = NULL;
        size_t capacity = 0;
        size_t n = 0;

        unsigned char chunk[8192];
        size_t got;

        while (
            (got = fread(
                chunk,
                1,
                sizeof(chunk),
                fp
            )) > 0
        ) {
            if (n + got > capacity) {
                size_t new_capacity =
                    capacity ? capacity * 2 : 8192;

                while (new_capacity < n + got) {
                    new_capacity *= 2;
                }

                unsigned char *tmp =
                    realloc(buf, new_capacity);

                if (!tmp) {
                    free(buf);
                    fclose(fp);
                    return;
                }

                buf = tmp;
                capacity = new_capacity;
            }

            memcpy(
                buf + n,
                chunk,
                got
            );

            n += got;
        }

        fclose(fp);

        const char *found = NULL;
        size_t offset = 0;

        while (offset + 4 <= n) {
            uint8_t type = buf[offset];
            uint8_t length = buf[offset + 1];

            if (length < 4 ||
                offset + length > n) {
                break;
            }

            /*
             * SMBIOS Type 17:
             * Memory Device
             * Type field is at offset 0x12.
             */
            if (type == 17 &&
                length > 0x12) {

                const char *name =
                    mem_type_name(
                        buf[offset + 0x12]
                    );

                if (name &&
                    strcmp(name, "Unknown") != 0) {

                    found = name;
                    break;
                }
            }

            if (type == 127) {
                break;
            }

            size_t str_off =
                offset + length;

            int terminated = 0;

            while (str_off + 1 < n) {
                if (
                    buf[str_off] == 0 &&
                    buf[str_off + 1] == 0
                ) {
                    terminated = 1;
                    break;
                }

                str_off++;
            }

            if (!terminated) {
                break;
            }

            offset = str_off + 2;
        }

        if (found) {
            snprintf(
                out,
                size,
                "%s",
                found
            );

            free(buf);
            return;
        }

        free(buf);
    }

    /*
     * Fallback for systems where direct DMI access
     * is unavailable.
     */
    (void)ram_type_via_dmidecode(out, size);
}

static void get_storage_info(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    FILE *mtab =
        setmntent("/proc/mounts", "r");

    if (!mtab) return;

    struct mntent entry;
    char strings_buf[4096];
    struct mntent *m;

    char device[256] = "";
    char fstype[64] = "";

    int found = 0;

    while (
        (m = getmntent_r(
            mtab,
            &entry,
            strings_buf,
            sizeof(strings_buf)
        )) != NULL
    ) {
        if (strcmp(m->mnt_dir, "/") == 0) {
            snprintf(
                device,
                sizeof(device),
                "%s",
                m->mnt_fsname
            );

            snprintf(
                fstype,
                sizeof(fstype),
                "%s",
                m->mnt_type
            );

            found = 1;
            break;
        }
    }

    endmntent(mtab);

    if (!found) return;

    struct statvfs vfs;

    if (statvfs("/", &vfs) != 0) {
        snprintf(
            out,
            size,
            "%s (%s)",
            device,
            fstype
        );

        return;
    }

    double total_gb =
        (double)vfs.f_blocks *
        (double)vfs.f_frsize /
        (1024.0 * 1024.0 * 1024.0);

    /*
     * f_bavail is the amount of free space available
     * to an unprivileged user.
     */
    double available_gb =
        (double)vfs.f_bavail *
        (double)vfs.f_frsize /
        (1024.0 * 1024.0 * 1024.0);

    double used_gb =
        total_gb - available_gb;

    if (used_gb < 0.0) {
        used_gb = 0.0;
    }

    snprintf(
        out,
        size,
        "%s (%s) %.1fG/%.1fG",
        device,
        fstype,
        used_gb,
        total_gb
    );
}

static void get_screen_info(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    DIR *d = opendir("/sys/class/drm");
    if (!d) return;

    long best_area = 0;
    char best_mode[64] = "";

    struct dirent *entry;

    while ((entry = readdir(d)) != NULL) {
        if (entry->d_name[0] == '.') {
            continue;
        }

        if (
            strstr(entry->d_name, "Writeback") ||
            strstr(entry->d_name, "Virtual")
        ) {
            continue;
        }

        char status_path[PATH_MAX];

        snprintf(
            status_path,
            sizeof(status_path),
            "/sys/class/drm/%s/status",
            entry->d_name
        );

        char status[32] = "";

        FILE *sf = fopen(status_path, "r");
        if (!sf) continue;

        if (fgets(status, sizeof(status), sf)) {
            status[strcspn(status, "\n")] = '\0';
        }

        fclose(sf);

        if (strcmp(status, "connected") != 0) {
            continue;
        }

        char modes_path[PATH_MAX];

        snprintf(
            modes_path,
            sizeof(modes_path),
            "/sys/class/drm/%s/modes",
            entry->d_name
        );

        FILE *mf = fopen(modes_path, "r");
        if (!mf) continue;

        char mode[64];

        /*
         * Check all available modes instead of
         * stopping after the first one.
         */
        while (fgets(mode, sizeof(mode), mf)) {
            mode[strcspn(mode, "\n")] = '\0';

            int w = 0;
            int h = 0;

            if (
                sscanf(
                    mode,
                    "%dx%d",
                    &w,
                    &h
                ) == 2 &&
                w > 0 &&
                h > 0
            ) {
                long area =
                    (long)w * (long)h;

                if (area > best_area) {
                    best_area = area;

                    snprintf(
                        best_mode,
                        sizeof(best_mode),
                        "%s",
                        mode
                    );
                }
            }
        }

        fclose(mf);
    }

    closedir(d);

    if (best_area > 0) {
        snprintf(
            out,
            size,
            "%s",
            best_mode
        );
    }
}

static int parse_vendor_line(
    const char *line,
    char *vname,
    size_t vname_size
) {
    if (!line ||
        !vname ||
        vname_size == 0) {
        return 0;
    }

    const char *name = line + 4;

    while (
        *name == ' ' ||
        *name == '\t'
    ) {
        name++;
    }

    snprintf(
        vname,
        vname_size,
        "%s",
        name
    );

    vname[strcspn(vname, "\n")] = '\0';

    return 1;
}

static int parse_device_line(
    const char *line,
    unsigned device,
    char *dname,
    size_t dname_size
) {
    if (!line ||
        !dname ||
        dname_size == 0) {
        return 0;
    }

    unsigned dv;

    if (
        sscanf(
            line + 1,
            "%4x",
            &dv
        ) != 1 ||
        dv != device
    ) {
        return 0;
    }

    const char *name = line + 5;

    while (
        *name == ' ' ||
        *name == '\t'
    ) {
        name++;
    }

    snprintf(
        dname,
        dname_size,
        "%s",
        name
    );

    dname[strcspn(dname, "\n")] = '\0';

    return 1;
}

static int pci_ids_lookup(
    const char *path,
    unsigned vendor,
    unsigned device,
    char *vname,
    size_t vname_size,
    char *dname,
    size_t dname_size
) {
    FILE *fp = fopen(path, "r");
    if (!fp) return 0;

    char line[512];

    int in_vendor = 0;
    int found_vendor = 0;
    int found_device = 0;

    while (fgets(line, sizeof(line), fp)) {
        if (
            line[0] == '#' ||
            line[0] == '\n'
        ) {
            continue;
        }

        if (
            line[0] == '\t' &&
            line[1] == '\t'
        ) {
            continue;
        }

        if (line[0] == '\t') {
            if (
                in_vendor &&
                parse_device_line(
                    line,
                    device,
                    dname,
                    dname_size
                )
            ) {
                found_device = 1;
                break;
            }

            continue;
        }

        unsigned v;

        if (
            sscanf(
                line,
                "%4x",
                &v
            ) != 1
        ) {
            continue;
        }

        if (v == vendor) {
            in_vendor = 1;

            found_vendor =
                parse_vendor_line(
                    line,
                    vname,
                    vname_size
                );

        } else if (in_vendor) {
            break;
        }
    }

    fclose(fp);

    return found_vendor &&
           found_device;
}

static void get_gpu_info(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    DIR *d =
        opendir("/sys/bus/pci/devices");

    if (!d) return;

    struct dirent *entry;

    while ((entry = readdir(d)) != NULL) {
        if (entry->d_name[0] == '.') {
            continue;
        }

        char class_path[PATH_MAX];

        snprintf(
            class_path,
            sizeof(class_path),
            "/sys/bus/pci/devices/%s/class",
            entry->d_name
        );

        unsigned class_code = 0;

        FILE *cf = fopen(class_path, "r");
        if (!cf) continue;

        int ok =
            fscanf(
                cf,
                "0x%x",
                &class_code
            ) == 1;

        fclose(cf);

        if (!ok ||
            ((class_code >> 16) & 0xFF) != 0x03) {
            continue;
        }

        char vendor_path[PATH_MAX];
        char device_path[PATH_MAX];

        snprintf(
            vendor_path,
            sizeof(vendor_path),
            "/sys/bus/pci/devices/%s/vendor",
            entry->d_name
        );

        snprintf(
            device_path,
            sizeof(device_path),
            "/sys/bus/pci/devices/%s/device",
            entry->d_name
        );

        unsigned vendor = 0;
        unsigned device = 0;

        FILE *vf = fopen(vendor_path, "r");

        int vendor_ok =
            vf &&
            fscanf(
                vf,
                "0x%x",
                &vendor
            ) == 1;

        if (vf) {
            fclose(vf);
        }

        FILE *df = fopen(device_path, "r");

        int device_ok =
            df &&
            fscanf(
                df,
                "0x%x",
                &device
            ) == 1;

        if (df) {
            fclose(df);
        }

        if (!vendor_ok || !device_ok) {
            continue;
        }

        char vname[128] = "";
        char dname[128] = "";

        int resolved = 0;

        const char *ids_paths[] = {
            "/usr/share/hwdata/pci.ids",
            "/usr/share/misc/pci.ids",
            "/usr/share/pci.ids"
        };

        for (
            size_t i = 0;
            i < sizeof(ids_paths) / sizeof(ids_paths[0]) &&
            !resolved;
            i++
        ) {
            resolved =
                pci_ids_lookup(
                    ids_paths[i],
                    vendor,
                    device,
                    vname,
                    sizeof(vname),
                    dname,
                    sizeof(dname)
                );
        }

        if (resolved) {
            snprintf(
                out,
                size,
                "%s %s",
                vname,
                dname
            );
        } else {
            snprintf(
                out,
                size,
                "PCI %04x:%04x",
                vendor,
                device
            );
        }

        break;
    }

    closedir(d);
}

static void get_shell_info(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    const char *shell =
        getenv("SHELL");

    if (!shell ||
        shell[0] == '\0') {
        return;
    }

    const char *slash =
        strrchr(shell, '/');

    const char *name =
        slash ? slash + 1 : shell;

    snprintf(
        out,
        size,
        "%s",
        name
    );
}

static int count_dir_entries(
    const char *path
) {
    DIR *d = opendir(path);
    if (!d) return -1;

    int count = 0;
    struct dirent *entry;

    while ((entry = readdir(d)) != NULL) {
        if (entry->d_name[0] == '.') {
            continue;
        }

        count++;
    }

    closedir(d);

    return count;
}

static int flatpak_count_via_cli(void) {
    FILE *fp = popen(
        "flatpak list --app --columns=application 2>/dev/null",
        "r"
    );

    if (!fp) return -1;

    int count = 0;
    char line[256];

    while (fgets(line, sizeof(line), fp)) {
        if (
            line[
                strspn(
                    line,
                    " \t\r\n"
                )
            ] != '\0'
        ) {
            count++;
        }
    }

    int status = pclose(fp);

    return status == 0
        ? count
        : -1;
}

static void get_flatpak_count(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    int cli_count =
        flatpak_count_via_cli();

    if (cli_count >= 0) {
        snprintf(
            out,
            size,
            "%d",
            cli_count
        );

        return;
    }

    int total = 0;
    int checked_any = 0;

    int sys_count =
        count_dir_entries(
            "/var/lib/flatpak/app"
        );

    if (sys_count >= 0) {
        total += sys_count;
        checked_any = 1;
    }

    const char *home =
        getenv("HOME");

    if (home) {
        char user_path[PATH_MAX];

        snprintf(
            user_path,
            sizeof(user_path),
            "%s/.local/share/flatpak/app",
            home
        );

        int user_count =
            count_dir_entries(user_path);

        if (user_count >= 0) {
            total += user_count;
            checked_any = 1;
        }
    }

    if (checked_any) {
        snprintf(
            out,
            size,
            "%d",
            total
        );
    }
}

static void get_os_age(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    struct statx stx;

    if (
        statx(
            AT_FDCWD,
            "/",
            0,
            STATX_BTIME,
            &stx
        ) == 0 &&
        (stx.stx_mask & STATX_BTIME)
    ) {
        time_t btime =
            stx.stx_btime.tv_sec;

        struct tm tm_info;

        if (localtime_r(
                &btime,
                &tm_info
            )) {

            strftime(
                out,
                size,
                "%Y-%m-%d",
                &tm_info
            );
        }
    }
}

static void get_os_uptime(
    char *out,
    size_t size
) {
    set_default(out, size);
    if (!out || size == 0) return;

    FILE *fp =
        fopen("/proc/uptime", "r");

    if (!fp) return;

    double seconds = 0.0;

    int ok =
        fscanf(
            fp,
            "%lf",
            &seconds
        ) == 1;

    fclose(fp);

    if (!ok ||
        seconds < 0.0) {
        return;
    }

    long long total_minutes =
        (long long)(seconds / 60.0);

    long long days =
        total_minutes / (60 * 24);

    long long hours =
        (total_minutes / 60) % 24;

    long long minutes =
        total_minutes % 60;

    char buf[64] = "";
    char part[32];

    int wrote = 0;

    if (days > 0) {
        snprintf(
            part,
            sizeof(part),
            "%lld day%s",
            days,
            days == 1 ? "" : "s"
        );

        strncat(
            buf,
            part,
            sizeof(buf) - strlen(buf) - 1
        );

        wrote = 1;
    }

    if (hours > 0) {
        if (wrote) {
            strncat(
                buf,
                ", ",
                sizeof(buf) - strlen(buf) - 1
            );
        }

        snprintf(
            part,
            sizeof(part),
            "%lld hour%s",
            hours,
            hours == 1 ? "" : "s"
        );

        strncat(
            buf,
            part,
            sizeof(buf) - strlen(buf) - 1
        );

        wrote = 1;
    }

    if (minutes > 0 || !wrote) {
        if (wrote) {
            strncat(
                buf,
                ", ",
                sizeof(buf) - strlen(buf) - 1
            );
        }

        snprintf(
            part,
            sizeof(part),
            "%lld minute%s",
            minutes,
            minutes == 1 ? "" : "s"
        );

        strncat(
            buf,
            part,
            sizeof(buf) - strlen(buf) - 1
        );
    }

    snprintf(
        out,
        size,
        "%s",
        buf
    );
}

void make_progress_bar(
    char *out,
    size_t size,
    double percentage,
    int width
) {
    if (!out || size == 0) {
        return;
    }

    if (percentage < 0.0) {
        percentage = 0.0;
    }

    if (percentage > 100.0) {
        percentage = 100.0;
    }

    if (width < 0) {
        width = 0;
    }

    int filled =
        (int)((percentage / 100.0) * width);

    size_t pos = 0;

    if (pos + 1 < size) {
        out[pos++] = '[';
    }

    for (
        int i = 0;
        i < width && pos + 1 < size;
        i++
    ) {
        const char *block =
            i < filled ? "█" : "░";

        size_t block_len =
            strlen(block);

        if (pos + block_len >= size) {
            break;
        }

        memcpy(
            out + pos,
            block,
            block_len
        );

        pos += block_len;
    }

    if (pos + 1 < size) {
        out[pos++] = ']';
    }

    if (pos < size) {
        snprintf(
            out + pos,
            size - pos,
            " %.0f%%",
            percentage
        );
    }

    out[size - 1] = '\0';
}

void print_color_palette(void) {
    printf("   ");

    for (int i = 0; i < 8; i++) {
        printf(
            "\033[4%dm   \033[0m",
            i
        );
    }

    printf("\n   ");

    for (int i = 0; i < 8; i++) {
        printf(
            "\033[10%dm   \033[0m",
            i
        );
    }

    printf("\n");
}

void collect_system_data(
    SystemData *data
) {
    if (!data) {
        return;
    }

    get_os_name(
        data->os_name,
        sizeof(data->os_name)
    );

    get_kernel(
        data->kernel,
        sizeof(data->kernel)
    );

    get_cpu_name(
        data->cpu_name,
        sizeof(data->cpu_name)
    );

    get_cpu_temp(
        data->cpu_temp,
        sizeof(data->cpu_temp)
    );

    get_ram_info(
        data->ram_total,
        sizeof(data->ram_total)
    );

    get_ram_type(
        data->ram_type,
        sizeof(data->ram_type)
    );

    get_storage_info(
        data->storage_info,
        sizeof(data->storage_info)
    );

    get_screen_info(
        data->screen_info,
        sizeof(data->screen_info)
    );

    get_gpu_info(
        data->gpu_type,
        sizeof(data->gpu_type)
    );

    get_shell_info(
        data->shell_info,
        sizeof(data->shell_info)
    );

    get_flatpak_count(
        data->flatpak_count,
        sizeof(data->flatpak_count)
    );

    get_os_age(
        data->os_age,
        sizeof(data->os_age)
    );

    get_os_uptime(
        data->os_uptime,
        sizeof(data->os_uptime)
    );
}
