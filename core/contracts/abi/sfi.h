/**
 * sfi.h — C ABI for in-process plugins (apiVersion 0).
 *
 * Cap'n Proto schemas: schema/*.capnp
 * Serialized messages are passed as Cap'n Proto packed or standard encoding in
 * sfi_process_task; exact transport is defined by plugin-host.
 */
#ifndef SFI_H
#define SFI_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SFI_API_VERSION_MAJOR 0
#define SFI_API_VERSION_MINOR 0

/** Matches ResultStatus in result.capnp */
typedef enum sfi_result_status {
    SFI_RESULT_OK = 0,
    SFI_RESULT_ERROR = 1,
    SFI_RESULT_TIMEOUT = 2,
    SFI_RESULT_CANCELLED = 3,
    SFI_RESULT_PARTIAL = 4
} sfi_result_status;

/** Matches StatusCode in common.capnp */
typedef enum sfi_status_code {
    SFI_STATUS_OK = 0,
    SFI_STATUS_TIMEOUT = 1,
    SFI_STATUS_CANCELLED = 2,
    SFI_STATUS_INVALID_ARGUMENT = 3,
    SFI_STATUS_NOT_FOUND = 4,
    SFI_STATUS_RESOURCE_EXHAUSTED = 5,
    SFI_STATUS_INTERNAL = 6,
    SFI_STATUS_UNAVAILABLE = 7
} sfi_status_code;

/**
 * Host services provided to in-process plugins.
 * All callbacks are optional (NULL = not provided).
 */
typedef struct sfi_host {
    uint16_t api_version_major;
    uint16_t api_version_minor;

    /** Log message at info level (UTF-8). */
    void (*log_info)(const char *msg);

    /** Resolve a buffer handle to a mapped pointer; unmap via release_buffer. */
    int (*map_buffer)(const void *handle_msg, size_t handle_len,
                      void **out_ptr, size_t *out_len, void **map_cookie);

    void (*release_buffer)(void *map_cookie);

    void *user_data;
} sfi_host;

typedef struct sfi_plugin_info {
    const char *name;
    const char *version;
    const char **capabilities;
    size_t capability_count;
} sfi_plugin_info;

/**
 * Return 0 on success. host must remain valid until sfi_shutdown.
 */
int sfi_init(const sfi_host *host, sfi_plugin_info *out_info);

/**
 * Process one task. task_msg / result_msg are Cap'n Proto serialized Task / Result.
 * result_cap is the maximum writable size of result_msg.
 * Returns 0 on success; negative errno-style code on failure.
 */
int sfi_process_task(const void *task_msg, size_t task_len,
                     void *result_msg, size_t result_cap, size_t *result_len);

void sfi_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* SFI_H */
