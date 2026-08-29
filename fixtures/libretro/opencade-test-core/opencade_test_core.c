/*
 * OpenCade deterministic libretro conformance fixture.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Original test-only implementation against the public libretro API surface. It is not an
 * emulator and contains no third-party game code or media.
 */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#if defined(_WIN32)
#define OCADE_EXPORT __declspec(dllexport)
#else
#define OCADE_EXPORT __attribute__((visibility("default")))
#endif

#define RETRO_API_VERSION 1
#define RETRO_ENVIRONMENT_SET_PIXEL_FORMAT 10
#define RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME 18
#define RETRO_PIXEL_FORMAT_XRGB8888 1
#define RETRO_DEVICE_JOYPAD 1
#define RETRO_DEVICE_ID_JOYPAD_B 0
#define RETRO_DEVICE_ID_JOYPAD_START 3
#define RETRO_DEVICE_ID_JOYPAD_UP 4
#define RETRO_DEVICE_ID_JOYPAD_DOWN 5
#define RETRO_DEVICE_ID_JOYPAD_LEFT 6
#define RETRO_DEVICE_ID_JOYPAD_RIGHT 7
#define OCADE_WIDTH 320
#define OCADE_HEIGHT 240

struct retro_game_info {
    const char *path;
    const void *data;
    size_t size;
    const char *meta;
};

struct retro_system_info {
    const char *library_name;
    const char *library_version;
    const char *valid_extensions;
    bool need_fullpath;
    bool block_extract;
};

struct retro_game_geometry {
    unsigned base_width;
    unsigned base_height;
    unsigned max_width;
    unsigned max_height;
    float aspect_ratio;
};

struct retro_system_timing {
    double fps;
    double sample_rate;
};

struct retro_system_av_info {
    struct retro_game_geometry geometry;
    struct retro_system_timing timing;
};

typedef bool (*retro_environment_t)(unsigned command, void *data);
typedef void (*retro_video_refresh_t)(const void *data, unsigned width, unsigned height, size_t pitch);
typedef void (*retro_audio_sample_t)(int16_t left, int16_t right);
typedef size_t (*retro_audio_sample_batch_t)(const int16_t *data, size_t frames);
typedef void (*retro_input_poll_t)(void);
typedef int16_t (*retro_input_state_t)(unsigned port, unsigned device, unsigned index, unsigned id);

struct ocade_state {
    uint64_t frame;
    uint16_t input[2];
};

static retro_environment_t environment_cb;
static retro_video_refresh_t video_cb;
static retro_audio_sample_t audio_cb;
static retro_audio_sample_batch_t audio_batch_cb;
static retro_input_poll_t input_poll_cb;
static retro_input_state_t input_state_cb;
static struct ocade_state state;
static uint32_t pixels[OCADE_WIDTH * OCADE_HEIGHT];

static uint16_t read_pad(unsigned port) {
    const unsigned ids[] = {
        RETRO_DEVICE_ID_JOYPAD_B, RETRO_DEVICE_ID_JOYPAD_START, RETRO_DEVICE_ID_JOYPAD_UP,
        RETRO_DEVICE_ID_JOYPAD_DOWN, RETRO_DEVICE_ID_JOYPAD_LEFT, RETRO_DEVICE_ID_JOYPAD_RIGHT
    };
    uint16_t result = 0;
    unsigned index;
    if (input_state_cb == NULL) {
        return 0;
    }
    for (index = 0; index < sizeof(ids) / sizeof(ids[0]); ++index) {
        if (input_state_cb(port, RETRO_DEVICE_JOYPAD, 0, ids[index]) != 0) {
            result |= (uint16_t)(1u << index);
        }
    }
    return result;
}

static void render_half(unsigned start_x, unsigned end_x, uint16_t input, uint32_t base) {
    unsigned x;
    unsigned y;
    uint32_t active = input == 0 ? base : (base ^ 0x003f3f3fu);
    for (y = 0; y < OCADE_HEIGHT; ++y) {
        for (x = start_x; x < end_x; ++x) {
            uint32_t grid = ((x / 16u) + (y / 16u) + (unsigned)(state.frame / 30u)) & 1u;
            pixels[y * OCADE_WIDTH + x] = active + (grid != 0u ? 0x00101010u : 0u);
        }
    }
}

OCADE_EXPORT unsigned retro_api_version(void) { return RETRO_API_VERSION; }

OCADE_EXPORT void retro_set_environment(retro_environment_t callback) {
    bool support_no_game = true;
    environment_cb = callback;
    if (environment_cb != NULL) {
        environment_cb(RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME, &support_no_game);
    }
}

OCADE_EXPORT void retro_set_video_refresh(retro_video_refresh_t callback) { video_cb = callback; }
OCADE_EXPORT void retro_set_audio_sample(retro_audio_sample_t callback) { audio_cb = callback; }
OCADE_EXPORT void retro_set_audio_sample_batch(retro_audio_sample_batch_t callback) {
    audio_batch_cb = callback;
}
OCADE_EXPORT void retro_set_input_poll(retro_input_poll_t callback) { input_poll_cb = callback; }
OCADE_EXPORT void retro_set_input_state(retro_input_state_t callback) { input_state_cb = callback; }

OCADE_EXPORT void retro_init(void) {
    unsigned pixel_format = RETRO_PIXEL_FORMAT_XRGB8888;
    memset(&state, 0, sizeof(state));
    memset(pixels, 0, sizeof(pixels));
    if (environment_cb != NULL) {
        environment_cb(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT, &pixel_format);
    }
}

OCADE_EXPORT void retro_deinit(void) {}

OCADE_EXPORT void retro_get_system_info(struct retro_system_info *info) {
    if (info == NULL) {
        return;
    }
    info->library_name = "OpenCade Test Core";
    info->library_version = "1.0.0";
    info->valid_extensions = "ocade";
    info->need_fullpath = false;
    info->block_extract = false;
}

OCADE_EXPORT void retro_get_system_av_info(struct retro_system_av_info *info) {
    if (info == NULL) {
        return;
    }
    info->geometry.base_width = OCADE_WIDTH;
    info->geometry.base_height = OCADE_HEIGHT;
    info->geometry.max_width = OCADE_WIDTH;
    info->geometry.max_height = OCADE_HEIGHT;
    info->geometry.aspect_ratio = 4.0f / 3.0f;
    info->timing.fps = 60.0;
    info->timing.sample_rate = 48000.0;
}

OCADE_EXPORT void retro_set_controller_port_device(unsigned port, unsigned device) {
    (void)port;
    (void)device;
}

OCADE_EXPORT void retro_reset(void) { memset(&state, 0, sizeof(state)); }

OCADE_EXPORT void retro_run(void) {
    if (input_poll_cb != NULL) {
        input_poll_cb();
    }
    state.input[0] = read_pad(0);
    state.input[1] = read_pad(1);
    state.frame += 1;
    render_half(0, OCADE_WIDTH / 2, state.input[0], 0x002050a0u);
    render_half(OCADE_WIDTH / 2, OCADE_WIDTH, state.input[1], 0x00a05020u);
    if (video_cb != NULL) {
        video_cb(pixels, OCADE_WIDTH, OCADE_HEIGHT, OCADE_WIDTH * sizeof(uint32_t));
    }
}

OCADE_EXPORT size_t retro_serialize_size(void) { return sizeof(state); }

OCADE_EXPORT bool retro_serialize(void *data, size_t size) {
    if (data == NULL || size < sizeof(state)) {
        return false;
    }
    memcpy(data, &state, sizeof(state));
    return true;
}

OCADE_EXPORT bool retro_unserialize(const void *data, size_t size) {
    if (data == NULL || size != sizeof(state)) {
        return false;
    }
    memcpy(&state, data, sizeof(state));
    return true;
}

OCADE_EXPORT bool retro_load_game(const struct retro_game_info *game) {
    static const char expected[] = "OPENCADE-TEST-1\n";
    if (game != NULL && game->data != NULL) {
        if (game->size != sizeof(expected) - 1u || memcmp(game->data, expected, sizeof(expected) - 1u) != 0) {
            return false;
        }
    }
    retro_reset();
    return true;
}

OCADE_EXPORT void retro_unload_game(void) {}
OCADE_EXPORT unsigned retro_get_region(void) { return 0; }
OCADE_EXPORT void *retro_get_memory_data(unsigned id) { (void)id; return NULL; }
OCADE_EXPORT size_t retro_get_memory_size(unsigned id) { (void)id; return 0; }
OCADE_EXPORT bool retro_load_game_special(unsigned type, const struct retro_game_info *info, size_t count) {
    (void)type;
    (void)info;
    (void)count;
    return false;
}
OCADE_EXPORT void retro_cheat_reset(void) {}
OCADE_EXPORT void retro_cheat_set(unsigned index, bool enabled, const char *code) {
    (void)index;
    (void)enabled;
    (void)code;
}
