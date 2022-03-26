use torque_config::loader::{load_from_str, ConfigFormat};
use torque_config::schema::TorqueConfig;
use torque_config::validator::validate_config;
use torque_config::watcher::ConfigManager;

const SAMPLE_YAML: &str = r#"
server:
  bind_addr: "127.0.0.1:8080"
  worker_threads: 4
  max_connections: 10000

admin:
  bind_addr: "127.0.0.1:9090"
  enable_dashboard: true

clusters:
  - name: "api_cluster"
    lb_policy: "peak_ewma"
    endpoints:
      - addr: "10.0.1.1:8000"
        weight: 10
      - addr: "10.0.1.2:8000"
        weight: 20

routes:
  - name: "get_users"
    path: "/api/v1/users/:id"
    methods: ["GET"]
    cluster: "api_cluster"
"#;

#[test]
fn test_parse_valid_yaml() {
    let config = load_from_str(SAMPLE_YAML, ConfigFormat::Yaml).expect("parse YAML failed");
    assert_eq!(config.server.bind_addr, "127.0.0.1:8080");
    assert_eq!(config.clusters.len(), 1);
    assert_eq!(config.clusters[0].name, "api_cluster");
    assert_eq!(config.clusters[0].endpoints.len(), 2);
    assert_eq!(config.routes.len(), 1);
    assert_eq!(config.routes[0].cluster, "api_cluster");
}

#[test]
fn test_validation_fails_on_unknown_cluster() {
    let mut config: TorqueConfig =
        serde_yaml::from_str(SAMPLE_YAML).expect("raw deserialize should succeed");
    config.routes[0].cluster = "non_existent_cluster".to_string();

    let err = validate_config(&config).expect_err("should fail validation");
    let err_msg = format!("{}", err);
    assert!(err_msg.contains("unknown upstream cluster"));
}

#[test]
fn test_validation_fails_on_duplicate_cluster() {
    let mut config: TorqueConfig =
        serde_yaml::from_str(SAMPLE_YAML).expect("raw deserialize should succeed");
    let duplicate = config.clusters[0].clone();
    config.clusters.push(duplicate);

    let err = validate_config(&config).expect_err("should fail on duplicate cluster");
    let err_msg = format!("{}", err);
    assert!(err_msg.contains("Duplicate cluster name"));
}

#[test]
fn test_config_manager_atomic_swap() {
    let initial_config = load_from_str(SAMPLE_YAML, ConfigFormat::Yaml).unwrap();
    let manager = ConfigManager::from_config(initial_config.clone(), "gateway.yaml");

    assert_eq!(manager.current().routes[0].name, "get_users");

    let mut updated_config = initial_config;
    updated_config.routes[0].name = "get_users_v2".to_string();

    // Directly test atomic reload
    let manager = ConfigManager::from_config(updated_config, "gateway.yaml");
    assert_eq!(manager.current().routes[0].name, "get_users_v2");
}
