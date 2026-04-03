#include <atomic>
#include <cstdlib>
#include <cstring>
#include <string>

namespace {
enum SuzakuWindowsSpeechState {
    kUnavailable = 0,
    kReady = 1,
    kListening = 2,
    kPermissionPending = 3,
    kDenied = 4,
    kError = 5,
};

std::atomic<int> g_state{kPermissionPending};
std::string g_transcript;

bool env_flag_enabled(const char *name) {
    const char *value = std::getenv(name);
    return value != nullptr && std::string(value) == "1";
}
}

extern "C" bool suzaku_windows_speech_is_supported(void) {
    return true;
}

extern "C" bool suzaku_windows_speech_supports_live_capture(void) {
    return env_flag_enabled("SUZAKU_WINDOWS_VOICE_NATIVE_LIVE");
}

extern "C" int suzaku_windows_speech_state(void) {
    return g_state.load();
}

extern "C" void suzaku_windows_speech_request_permissions(void) {
    if (env_flag_enabled("SUZAKU_WINDOWS_VOICE_FORCE_DENIED")) {
        g_state.store(kDenied);
        return;
    }
    if (env_flag_enabled("SUZAKU_WINDOWS_VOICE_FORCE_ERROR")) {
        g_state.store(kError);
        return;
    }
    if (g_state.load() == kPermissionPending) {
        g_state.store(kReady);
    }
}

extern "C" bool suzaku_windows_speech_start(void) {
    if (g_state.load() != kReady) {
        return false;
    }
    g_state.store(kListening);
    if (g_transcript.empty()) {
        const char *seed = std::getenv("SUZAKU_WINDOWS_VOICE_SAMPLE");
        if (seed != nullptr) {
            g_transcript = seed;
        }
    }
    return true;
}

extern "C" void suzaku_windows_speech_stop(void) {
    if (g_state.load() == kListening) {
        g_state.store(kReady);
    }
}

extern "C" bool suzaku_windows_speech_consume_transcript(char *buffer, size_t capacity) {
    if (buffer == nullptr || capacity == 0 || g_transcript.empty()) {
        return false;
    }
    if (g_transcript.size() + 1 > capacity) {
        return false;
    }
    std::memcpy(buffer, g_transcript.c_str(), g_transcript.size());
    buffer[g_transcript.size()] = '\0';
    g_transcript.clear();
    return true;
}

extern "C" void suzaku_windows_speech_debug_seed_transcript(const char *text) {
    if (text == nullptr) {
        g_transcript.clear();
        return;
    }
    g_transcript = text;
}

extern "C" void suzaku_windows_speech_debug_set_state(int state) {
    g_state.store(state);
}
