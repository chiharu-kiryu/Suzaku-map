#include <stdbool.h>
#include <stdio.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int suzaku_linux_state = 3;
static bool suzaku_linux_active = false;
static char suzaku_linux_transcript[2048] = {0};

static bool suzaku_linux_flag_enabled(const char* key) {
    const char* value = getenv(key);
    return value != NULL && strcmp(value, "1") == 0;
}

static bool suzaku_linux_has_session_bus(void) {
    const char* value = getenv("DBUS_SESSION_BUS_ADDRESS");
    return value != NULL && value[0] != '\0';
}

static bool suzaku_linux_has_path_executable(const char* name) {
    const char* path = getenv("PATH");
    if (path == NULL || path[0] == '\0') {
        return false;
    }

    size_t name_len = strlen(name);
    const char* cursor = path;
    while (*cursor != '\0') {
        const char* next = strchr(cursor, ':');
        size_t dir_len = next == NULL ? strlen(cursor) : (size_t)(next - cursor);
        if (dir_len > 0) {
            char candidate[1024];
            if (dir_len + 1 + name_len + 1 < sizeof(candidate)) {
                memcpy(candidate, cursor, dir_len);
                candidate[dir_len] = '/';
                memcpy(candidate + dir_len + 1, name, name_len);
                candidate[dir_len + 1 + name_len] = '\0';
                if (access(candidate, X_OK) == 0) {
                    return true;
                }
            }
        }

        if (next == NULL) {
            break;
        }
        cursor = next + 1;
    }

    return false;
}

bool suzaku_linux_speech_portal_available(void) {
    const char* override = getenv("SUZAKU_LINUX_PORTAL_AVAILABLE");
    if (override != NULL) {
        return strcmp(override, "1") == 0;
    }

    return suzaku_linux_has_session_bus()
        && suzaku_linux_has_path_executable("xdg-desktop-portal");
}

bool suzaku_linux_speech_pipewire_available(void) {
    const char* override = getenv("SUZAKU_LINUX_PIPEWIRE_AVAILABLE");
    if (override != NULL) {
        return strcmp(override, "1") == 0;
    }

    return suzaku_linux_has_path_executable("pipewire")
        || suzaku_linux_has_path_executable("pipewire-pulse")
        || getenv("PIPEWIRE_RUNTIME_DIR") != NULL;
}

static void suzaku_linux_apply_debug_state(void) {
    if (suzaku_linux_flag_enabled("SUZAKU_LINUX_VOICE_FORCE_READY")) {
        suzaku_linux_state = 1;
        return;
    }
    if (suzaku_linux_flag_enabled("SUZAKU_LINUX_VOICE_FORCE_DENIED")) {
        suzaku_linux_state = 4;
        return;
    }
    if (suzaku_linux_flag_enabled("SUZAKU_LINUX_VOICE_FORCE_ERROR")) {
        suzaku_linux_state = 5;
        return;
    }
    if (suzaku_linux_speech_portal_available()
        && suzaku_linux_speech_pipewire_available()) {
        suzaku_linux_state = 1;
    } else {
        suzaku_linux_state = 3;
    }
}

bool suzaku_linux_speech_supports_live_capture(void) {
    return suzaku_linux_speech_portal_available()
        && suzaku_linux_speech_pipewire_available();
}

int suzaku_linux_speech_state(void) {
    suzaku_linux_apply_debug_state();
    return suzaku_linux_state;
}

void suzaku_linux_speech_request_permissions(void) {
    suzaku_linux_apply_debug_state();
}

bool suzaku_linux_speech_start(void) {
    suzaku_linux_apply_debug_state();
    if (suzaku_linux_state != 1 && suzaku_linux_state != 2) {
        return false;
    }
    suzaku_linux_active = true;

    if (suzaku_linux_transcript[0] == '\0') {
        const char* sample = getenv("SUZAKU_LINUX_VOICE_SAMPLE");
        if (sample == NULL || sample[0] == '\0') {
            sample = getenv("SUZAKU_UBUNTU_VOICE_SAMPLE");
        }
        if (sample != NULL && sample[0] != '\0') {
            strncpy(suzaku_linux_transcript, sample, sizeof(suzaku_linux_transcript) - 1);
            suzaku_linux_transcript[sizeof(suzaku_linux_transcript) - 1] = '\0';
        }
    }

    return true;
}

void suzaku_linux_speech_stop(void) {
    suzaku_linux_active = false;
}

bool suzaku_linux_speech_consume_transcript(char* buffer, size_t capacity) {
    if (buffer == NULL || capacity == 0 || suzaku_linux_transcript[0] == '\0') {
        return false;
    }

    strncpy(buffer, suzaku_linux_transcript, capacity - 1);
    buffer[capacity - 1] = '\0';
    suzaku_linux_transcript[0] = '\0';
    return true;
}

void suzaku_linux_speech_debug_seed_transcript(const char* text) {
    if (text == NULL) {
        suzaku_linux_transcript[0] = '\0';
        return;
    }

    strncpy(suzaku_linux_transcript, text, sizeof(suzaku_linux_transcript) - 1);
    suzaku_linux_transcript[sizeof(suzaku_linux_transcript) - 1] = '\0';
}
