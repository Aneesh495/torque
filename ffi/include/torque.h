/**
 * @file torque.h
 * @brief Foreign Function Interface (FFI) declarations for Torque Reverse Proxy.
 *
 * Provides a stable C ABI for embedding Torque into native C/C++ applications,
 * controlling runtime daemons, inspecting metrics, and triggering atomic reloads.
 */

#ifndef TORQUE_H
#define TORQUE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

/** Status return codes for Torque C ABI operations. */
typedef enum {
    TORQUE_OK = 0,
    TORQUE_ERR_NULL_POINTER = -1,
    TORQUE_ERR_CONFIG_INVALID = -2,
    TORQUE_ERR_BIND_FAILED = -3,
    TORQUE_ERR_RELOAD_FAILED = -4,
    TORQUE_ERR_BUFFER_OVERFLOW = -5,
    TORQUE_ERR_INTERNAL = -99
} torque_status_t;

/** Opaque handle to a running Torque server instance. */
typedef struct torque_server torque_server_t;

/** Telemetry snapshot holding high-level operational counters. */
typedef struct {
    uint64_t total_requests;
    uint64_t status_2xx;
    uint64_t status_3xx;
    uint64_t status_4xx;
    uint64_t status_5xx;
    int64_t  active_connections;
    uint64_t bytes_sent;
    uint64_t bytes_received;
    double   latency_p50_ms;
    double   latency_p90_ms;
    double   latency_p99_ms;
} torque_telemetry_t;

/**
 * Initialize a new Torque proxy server instance from a configuration file.
 *
 * @param config_path Null-terminated string containing path to YAML/JSON/TOML config file.
 * @param out_server Pointer to receive allocated server handle.
 * @return TORQUE_OK on success, or appropriate error code.
 */
torque_status_t torque_server_create(const char* config_path, torque_server_t** out_server);

/**
 * Start the proxy event loops on the current thread or background worker pool.
 *
 * @param server Valid server handle created via torque_server_create.
 * @return TORQUE_OK on clean shutdown, or error code on failure.
 */
torque_status_t torque_server_start(torque_server_t* server);

/**
 * Request graceful termination of server event loops.
 *
 * @param server Valid server handle.
 * @return TORQUE_OK on successful signal delivery.
 */
torque_status_t torque_server_stop(torque_server_t* server);

/**
 * Trigger zero-downtime configuration hot reload.
 *
 * @param server Valid server handle.
 * @return TORQUE_OK on successful reload.
 */
torque_status_t torque_server_reload(torque_server_t* server);

/**
 * Extract live operational telemetry counters.
 *
 * @param server Valid server handle.
 * @param out_telemetry Pointer to allocated telemetry structure.
 * @return TORQUE_OK on success.
 */
torque_status_t torque_server_get_telemetry(torque_server_t* server, torque_telemetry_t* out_telemetry);

/**
 * Export Prometheus metrics string into caller-provided buffer.
 *
 * @param server Valid server handle.
 * @param buffer Output buffer to receive null-terminated text.
 * @param buf_len Length of output buffer.
 * @param out_written Pointer to receive written byte count (excluding null terminator).
 * @return TORQUE_OK on success, or TORQUE_ERR_BUFFER_OVERFLOW if buffer is insufficient.
 */
torque_status_t torque_server_export_prometheus(
    torque_server_t* server,
    char* buffer,
    size_t buf_len,
    size_t* out_written
);

/**
 * Destroy server instance and release all associated resources.
 *
 * @param server Valid server handle.
 */
void torque_server_destroy(torque_server_t* server);

#ifdef __cplusplus
}
#endif

#endif /* TORQUE_H */
