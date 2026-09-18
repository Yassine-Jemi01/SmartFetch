#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <unistd.h>
#include <pwd.h>
#endif

#include <curl/curl.h>
#include "sysinfo.h"

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#ifndef HOST_NAME_MAX
#define HOST_NAME_MAX 256
#endif

struct membuf {
    char *data;
    size_t size;
};

static size_t curl_write_cb(void *contents, size_t size, size_t nmemb, void *userp) {
    size_t realsize = size * nmemb;
    struct membuf *mem = (struct membuf *)userp;
    char *ptr = realloc(mem->data, mem->size + realsize + 1);

    if (!ptr) return 0;

    mem->data = ptr;
    memcpy(mem->data + mem->size, contents, realsize);
    mem->size += realsize;
    mem->data[mem->size] = '\0';

    return realsize;
}

static int parse_owner_repo(const char *repo_url, char *owner, size_t owner_size, char *repo, size_t repo_size) {
    const char *marker = strstr(repo_url, "github.com/");
    if (!marker) return 0;

    marker += strlen("github.com/");

    const char *slash = strchr(marker, '/');
    if (!slash) return 0;

    size_t owner_len = (size_t)(slash - marker);
    if (owner_len == 0 || owner_len >= owner_size) return 0;

    memcpy(owner, marker, owner_len);
    owner[owner_len] = '\0';

    const char *repo_start = slash + 1;
    const char *git_suffix = strstr(repo_start, ".git");

    size_t repo_len = git_suffix
        ? (size_t)(git_suffix - repo_start)
        : strlen(repo_start);

    if (repo_len == 0 || repo_len >= repo_size) return 0;

    memcpy(repo, repo_start, repo_len);
    repo[repo_len] = '\0';

    return 1;
}

static int extract_json_sha(const char *json, char *out, size_t out_size) {
    const char *key = "\"sha\"";
    const char *p = strstr(json, key);

    if (!p) return 0;

    p = strchr(p + strlen(key), ':');
    if (!p) return 0;

    p++;

    while (*p == ' ' || *p == '"') p++;

    size_t i = 0;

    while (p[i] && p[i] != '"' && i < out_size - 1) {
        out[i] = p[i];
        i++;
    }

    out[i] = '\0';

    return i > 0;
}

static int fetch_latest_sha(char *out, size_t out_size) {
    char owner[128], repo[128];

    if (!parse_owner_repo(SF_REPO_URL, owner, sizeof(owner), repo, sizeof(repo))) {
        return 0;
    }

    char api_url[256];

    snprintf(
        api_url,
        sizeof(api_url),
        "https://api.github.com/repos/%s/%s/commits/main",
        owner,
        repo
    );

    CURL *curl = curl_easy_init();
    if (!curl) return 0;

    struct membuf mem = {
        .data = malloc(1),
        .size = 0
    };

    if (!mem.data) {
        curl_easy_cleanup(curl);
        return 0;
    }

    mem.data[0] = '\0';

    curl_easy_setopt(curl, CURLOPT_URL, api_url);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, curl_write_cb);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, (void *)&mem);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, "smartfetch");
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 5L);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_PROTOCOLS, CURLPROTO_HTTPS);

    CURLcode res = curl_easy_perform(curl);

    long http_code = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &http_code);

    curl_easy_cleanup(curl);

    int ok = 0;

    if (res == CURLE_OK && http_code == 200) {
        ok = extract_json_sha(mem.data, out, out_size);
    }

    free(mem.data);

    return ok;
}

static void get_exe_dir(char *out, size_t size) {
    out[0] = '\0';

    if (size == 0) return;

#ifdef _WIN32
    DWORD len = GetModuleFileNameA(NULL, out, (DWORD)size);

    if (len == 0 || len >= size) {
        out[0] = '\0';
        return;
    }

    char *slash = strrchr(out, '\\');

    if (!slash) {
        slash = strrchr(out, '/');
    }

    if (slash) {
        *(slash + 1) = '\0';
    } else {
        out[0] = '\0';
    }

#else
    ssize_t len = readlink("/proc/self/exe", out, size - 1);

    if (len <= 0 || (size_t)len >= size) {
        out[0] = '\0';
        return;
    }

    out[len] = '\0';

    char *slash = strrchr(out, '/');

    if (slash) {
        *(slash + 1) = '\0';
    } else {
        out[0] = '\0';
    }
#endif
}

static int visible_len(const char *s) {
    int len = 0;

    while (*s) {
        if (*s == '\033' && *(s + 1) == '[') {
            s += 2;

            while (*s && *s != 'm') {
                s++;
            }

            if (*s == 'm') {
                s++;
            }
        } else {
            len++;
            s++;
        }
    }

    return len;
}

static FILE *open_ascii_file(const char *exe_dir, const char *ascii_name) {
    char path[PATH_MAX];

    snprintf(
        path,
        sizeof(path),
        "/usr/share/smartfetch/Ascii_art/%s",
        ascii_name
    );

    FILE *fp = fopen(path, "r");

    if (fp) return fp;

    snprintf(
        path,
        sizeof(path),
        "%sAscii_art/%s",
        exe_dir,
        ascii_name
    );

    fp = fopen(path, "r");

    if (fp) return fp;

    snprintf(
        path,
        sizeof(path),
        "Ascii_art/%s",
        ascii_name
    );

    return fopen(path, "r");
}

typedef struct {
    const char *needle;
    const char *file;
} os_match_t;

static const char *pick_ascii_name(const char *os_name) {
    static const os_match_t os_matches[] = {
        {"CachyOS", "cashyos_ascii.txt"},
        {"cachyos", "cashyos_ascii.txt"},
        {"EndeavourOS", "endeavouros_ascii.txt"},
        {"endeavouros", "endeavouros_ascii.txt"},
        {"Arch", "arch_ascii.txt"},
        {"arch", "arch_ascii.txt"},
        {"Fedora", "fedora_ascii.txt"},
        {"fedora", "fedora_ascii.txt"},
        {"Linux Mint", "linuxmint_ascii.txt"},
        {"linuxmint", "linuxmint_ascii.txt"},
        {"Ubuntu", "ubuntu_ascii.txt"},
        {"ubuntu", "ubuntu_ascii.txt"},
        {"Debian GNU/Linux", "debian_ascii.txt"},
        {"debian", "debian_ascii.txt"},
        {"Windows", "windows_ascii.txt"},
        {"windows", "windows_ascii.txt"},
    };

    for (size_t i = 0; i < sizeof(os_matches) / sizeof(os_matches[0]); i++) {
        if (strstr(os_name, os_matches[i].needle) != NULL) {
            return os_matches[i].file;
        }
    }

    return "default_ascii.txt";
}

static void resolve_identity(
    const char **username_out,
    char *hostname,
    size_t hostname_size
) {
#ifdef _WIN32
    static char win_user[256];

    DWORD len = sizeof(win_user);

    if (GetUserNameA(win_user, &len)) {
        *username_out = win_user;
    } else {
        const char *env_user = getenv("USERNAME");
        *username_out = env_user ? env_user : "user";
    }

    DWORD host_len = (DWORD)hostname_size;

    if (!GetComputerNameA(hostname, &host_len)) {
        const char *env_host = getenv("COMPUTERNAME");

        snprintf(
            hostname,
            hostname_size,
            "%s",
            env_host ? env_host : "smartfetch"
        );
    }

#else
    struct passwd *pw = getpwuid(getuid());

    if (pw && pw->pw_name) {
        *username_out = pw->pw_name;
    } else {
        const char *env_user = getenv("USER");
        *username_out = env_user ? env_user : "user";
    }

    if (gethostname(hostname, hostname_size) != 0) {
        snprintf(hostname, hostname_size, "smartfetch");
    } else {
        hostname[hostname_size - 1] = '\0';

        if (hostname[0] == '\0') {
            snprintf(hostname, hostname_size, "smartfetch");
        }
    }
#endif
}

static void build_info_lines(
    char info_lines[SF_LINE_COUNT][SF_LINE_WIDTH],
    const SystemData *data,
    const char *username,
    const char *hostname
) {
    memset(
        info_lines,
        0,
        SF_LINE_COUNT * SF_LINE_WIDTH
    );

    snprintf(
        info_lines[0],
        SF_LINE_WIDTH,
        "\033[1;36m%s\033[0m@\033[1;36m%s\033[0m",
        username,
        hostname
    );

    int user_host_len = strlen(username) + strlen(hostname) + 1;

    char separator[128] = "";

    for (int i = 0; i < user_host_len && i < 127; i++) {
        separator[i] = '-';
    }

    separator[user_host_len < 127 ? user_host_len : 127] = '\0';

    snprintf(
        info_lines[1],
        SF_LINE_WIDTH,
        "%s",
        separator
    );

    snprintf(
        info_lines[2],
        SF_LINE_WIDTH,
        "\033[1;32mOS       :\033[0m %s",
        data->os_name
    );

    snprintf(
        info_lines[3],
        SF_LINE_WIDTH,
        "\033[1;32mKernel   :\033[0m %s",
        data->kernel
    );

    snprintf(
        info_lines[4],
        SF_LINE_WIDTH,
        "\033[1;32mUptime   :\033[0m %s",
        data->os_uptime
    );

    snprintf(
        info_lines[5],
        SF_LINE_WIDTH,
        "\033[1;32mOS Age   :\033[0m %s",
        data->os_age
    );

    snprintf(
        info_lines[6],
        SF_LINE_WIDTH,
        "\033[1;32mCPU      :\033[0m %s",
        data->cpu_name
    );

    snprintf(
        info_lines[7],
        SF_LINE_WIDTH,
        "\033[1;32mCPU Temp :\033[0m %s",
        data->cpu_temp
    );

    snprintf(
        info_lines[8],
        SF_LINE_WIDTH,
        "\033[1;32mRAM      :\033[0m %s (%s)",
        data->ram_total,
        data->ram_type
    );

    snprintf(
        info_lines[9],
        SF_LINE_WIDTH,
        "\033[1;32mStorage  :\033[0m %s",
        data->storage_info
    );

    snprintf(
        info_lines[10],
        SF_LINE_WIDTH,
        "\033[1;32mDisplay  :\033[0m %s",
        data->screen_info
    );

    snprintf(
        info_lines[11],
        SF_LINE_WIDTH,
        "\033[1;32mGPU      :\033[0m %s",
        data->gpu_type
    );

    snprintf(
        info_lines[12],
        SF_LINE_WIDTH,
        "\033[1;32mShell    :\033[0m %s",
        data->shell_info
    );

    snprintf(
        info_lines[13],
        SF_LINE_WIDTH,
        "\033[1;32mFlatpak  :\033[0m %s",
        data->flatpak_count
    );
}

static void print_body(
    FILE *ascii_fp,
    char info_lines[SF_LINE_COUNT][SF_LINE_WIDTH]
) {
    char ascii_line[128];
    int line_index = 0;

    while (1) {
        char *got_ascii = NULL;

        if (ascii_fp) {
            got_ascii = fgets(
                ascii_line,
                sizeof(ascii_line),
                ascii_fp
            );

            if (got_ascii) {
                ascii_line[strcspn(ascii_line, "\n")] = 0;
            }
        }

        int has_info = line_index < SF_LINE_COUNT;

        if (!got_ascii && !has_info) {
            break;
        }

        if (got_ascii) {
            int pad = SF_ASCII_WIDTH - visible_len(ascii_line);

            printf("%s", ascii_line);

            for (int i = 0; i < pad; i++) {
                putchar(' ');
            }

            putchar(' ');
        } else {
            for (int i = 0; i < SF_ASCII_WIDTH + 1; i++) {
                putchar(' ');
            }
        }

        if (has_info) {
            printf("%s", info_lines[line_index]);
        }

        printf("\n");
        line_index++;
    }
}

void render_ui(const SystemData *data) {
    char exe_dir[PATH_MAX];

    get_exe_dir(
        exe_dir,
        sizeof(exe_dir)
    );

    const char *ascii_name = pick_ascii_name(data->os_name);

    FILE *ascii_fp = open_ascii_file(
        exe_dir,
        ascii_name
    );

    printf("\n");

    char hostname[HOST_NAME_MAX];
    const char *username = NULL;

    resolve_identity(
        &username,
        hostname,
        sizeof(hostname)
    );

    char info_lines[SF_LINE_COUNT][SF_LINE_WIDTH];

    build_info_lines(
        info_lines,
        data,
        username,
        hostname
    );

    print_body(
        ascii_fp,
        info_lines
    );

    if (ascii_fp) {
        fclose(ascii_fp);
    }

    printf("\n");

    print_color_palette();

    printf("\n");
}

void print_help(void) {
    printf(
        "SmartFetch (sfetch) - v%s\n",
        SF_VERSION
    );

    printf(
        "A fast tool to display system information with distro ASCII logo.\n\n"
    );

    printf("Usage:\n");
    printf("  sfetch                  Display system information\n");
    printf("  sfetch -h, --help       Display this help message\n");
    printf("  sfetch -u, --update     Check for new updates on GitHub\n");
}

void check_for_update(void) {
    char remote_hash[64] = "N/A";

    if (!fetch_latest_sha(remote_hash, sizeof(remote_hash))) {
        printf("Could not check for updates.\n");
        printf("Make sure you are connected to the internet.\n");
        return;
    }

    if (strcmp(SF_VERSION, "unknown") == 0) {
        printf(
            "Cannot determine current version (installed without git info).\n"
        );

        printf(
            "Latest version on GitHub: %.7s\n",
            remote_hash
        );

        printf(
            "To update run: git pull origin main && make && sudo make install\n"
        );

        return;
    }

    if (strncmp(remote_hash, SF_VERSION, strlen(SF_VERSION)) == 0) {
        printf(
            "You are on the latest version (%s).\n",
            SF_VERSION
        );
    } else {
        printf("A new update is available!\n");

        printf(
            "Current version : %s\n",
            SF_VERSION
        );

        printf(
            "Latest version  : %.7s\n",
            remote_hash
        );

        printf(
            "\nTo update, go to the project folder and run:\n"
        );

        printf("  git pull origin main\n");
        printf("  make && sudo make install\n");
    }
}
