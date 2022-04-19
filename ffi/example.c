/**
 * @file example.c
 * @brief Demonstration of embedding Torque Reverse Proxy inside a native C application.
 */

#include <stdio.h>
#include <stdlib.h>
#include "include/torque.h"

int main(int argc, char** argv) {
    const char* config_path = (argc > 1) ? argv[1] : "examples/gateway.yaml";

    printf("Initializing Torque Reverse Proxy embedded runtime...\n");
    printf("Loading configuration from: %s\n", config_path);

    torque_server_t* server = NULL;
    torque_status_t status = torque_server_create(config_path, &server);

    if (status != TORQUE_OK) {
        fprintf(stderr, "Failed to initialize Torque server (code: %d)\n", status);
        return 1;
    }

    printf("Torque runtime initialized successfully at %p\n", (void*)server);

    torque_telemetry_t telemetry;
    if (torque_server_get_telemetry(server, &telemetry) == TORQUE_OK) {
        printf("Current active connections: %ld\n", (long)telemetry.active_connections);
    }

    printf("Triggering hot-reload demonstration...\n");
    if (torque_server_reload(server) == TORQUE_OK) {
        printf("Configuration hot-reloaded successfully via C ABI.\n");
    }

    printf("Shutting down and freeing Torque runtime.\n");
    torque_server_destroy(server);

    return 0;
}
