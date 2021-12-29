use torque_proto::http1::{Method, Request};
use torque_router::{AtomicRouteTable, Predicate, Route, RouteTable};

#[test]
fn test_exact_route_matching() {
    let routes = vec![
        Route::new("health", "/healthz", "health_cluster"),
        Route::new("metrics", "/metrics", "metrics_cluster"),
    ];

    let table = RouteTable::compile(routes).expect("compile failed");

    let req = Request::new(Method::Get, "/healthz");
    let matched = table.match_request(&req).expect("should match health");
    assert_eq!(matched.route.name, "health");
    assert_eq!(matched.route.cluster, "health_cluster");

    let req_miss = Request::new(Method::Get, "/nonexistent");
    assert!(table.match_request(&req_miss).is_none());
}

#[test]
fn test_parameterized_route_matching() {
    let routes = vec![
        Route::new("user_detail", "/api/v1/users/:user_id", "user_service"),
        Route::new("user_orders", "/api/v1/users/:user_id/orders/:order_id", "order_service"),
    ];

    let table = RouteTable::compile(routes).expect("compile failed");

    let req1 = Request::new(Method::Get, "/api/v1/users/42");
    let m1 = table.match_request(&req1).expect("match user detail");
    assert_eq!(m1.route.name, "user_detail");
    assert_eq!(m1.params.get("user_id"), Some(&"42".to_string()));

    let req2 = Request::new(Method::Get, "/api/v1/users/42/orders/ord-9988");
    let m2 = table.match_request(&req2).expect("match user order");
    assert_eq!(m2.route.name, "user_orders");
    assert_eq!(m2.params.get("user_id"), Some(&"42".to_string()));
    assert_eq!(m2.params.get("order_id"), Some(&"ord-9988".to_string()));
}

#[test]
fn test_regex_constrained_route() {
    let routes = vec![
        Route::new("numeric_id", "/items/:id<\\d+>", "item_service"),
        Route::new("string_code", "/items/:code", "code_service"),
    ];

    let table = RouteTable::compile(routes).expect("compile failed");

    let req_num = Request::new(Method::Get, "/items/12345");
    let m_num = table.match_request(&req_num).expect("should match numeric route");
    assert_eq!(m_num.route.name, "numeric_id");
    assert_eq!(m_num.params.get("id"), Some(&"12345".to_string()));

    let req_str = Request::new(Method::Get, "/items/sku-xyz");
    let m_str = table.match_request(&req_str).expect("should match string route");
    assert_eq!(m_str.route.name, "string_code");
    assert_eq!(m_str.params.get("code"), Some(&"sku-xyz".to_string()));
}

#[test]
fn test_wildcard_route() {
    let routes = vec![
        Route::new("static_assets", "/assets/*filepath", "static_cluster"),
    ];

    let table = RouteTable::compile(routes).expect("compile failed");

    let req = Request::new(Method::Get, "/assets/css/theme/dark.css");
    let m = table.match_request(&req).expect("should match wildcard");
    assert_eq!(m.route.name, "static_assets");
    assert_eq!(
        m.params.get("filepath"),
        Some(&"css/theme/dark.css".to_string())
    );
}

#[test]
fn test_method_and_predicate_filtering() {
    let routes = vec![
        Route::new("post_only", "/api/v1/payments", "payment_cluster")
            .with_methods(vec![Method::Post])
            .with_predicate(Predicate::header_exact("x-api-version", "2")),
    ];

    let table = RouteTable::compile(routes).expect("compile failed");

    // GET should fail method filter
    let mut req_get = Request::new(Method::Get, "/api/v1/payments");
    req_get.headers.insert("x-api-version", "2");
    assert!(table.match_request(&req_get).is_none());

    // POST without matching header should fail predicate filter
    let req_post_no_hdr = Request::new(Method::Post, "/api/v1/payments");
    assert!(table.match_request(&req_post_no_hdr).is_none());

    // POST with matching header should succeed
    let mut req_post_valid = Request::new(Method::Post, "/api/v1/payments");
    req_post_valid.headers.insert("x-api-version", "2");
    let m = table.match_request(&req_post_valid).expect("should match valid post");
    assert_eq!(m.route.name, "post_only");
}

#[test]
fn test_atomic_route_table_swap() {
    let r1 = vec![Route::new("v1", "/api/v1", "cluster_v1")];
    let initial_table = RouteTable::compile(r1).unwrap();
    let atomic_table = AtomicRouteTable::new(initial_table);

    let req1 = Request::new(Method::Get, "/api/v1");
    assert!(atomic_table.load().match_request(&req1).is_some());

    // Swap to new configuration
    let r2 = vec![Route::new("v2", "/api/v2", "cluster_v2")];
    let new_table = RouteTable::compile(r2).unwrap();
    atomic_table.swap(new_table);

    // Old route is gone, new route is active
    assert!(atomic_table.load().match_request(&req1).is_none());
    let req2 = Request::new(Method::Get, "/api/v2");
    assert!(atomic_table.load().match_request(&req2).is_some());
}
