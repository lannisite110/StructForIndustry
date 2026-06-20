#ifndef SFI_HAL_IPC_H
#define SFI_HAL_IPC_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SFI_HAL_IPC_MAGIC 0x00494653u /* "SFI\0" little-endian */
#define SFI_HAL_IPC_VERSION 1u

#define SFI_HAL_SOURCE_ID_LEN 32
#define SFI_HAL_POOL_ID_LEN 16
#define SFI_HAL_SHM_NAME_LEN 32

enum sfi_hal_pixel_format {
    SFI_HAL_PIXEL_UNKNOWN = 0,
    SFI_HAL_PIXEL_GRAY8 = 1,
};

#pragma pack(push, 1)
typedef struct sfi_hal_frame_notify {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved0;
    uint64_t frame_id;
    uint64_t timestamp_ns;
    uint64_t sequence;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint8_t format;
    uint8_t reserved1[3];
    char source_id[SFI_HAL_SOURCE_ID_LEN];
    char pool_id[SFI_HAL_POOL_ID_LEN];
    uint32_t slot_index;
    uint32_t generation;
    uint64_t byte_length;
    char shm_name[SFI_HAL_SHM_NAME_LEN];
} sfi_hal_frame_notify;
#pragma pack(pop)

#ifdef __cplusplus
}
#endif

#endif /* SFI_HAL_IPC_H */
