use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

#[allow(dead_code)]
pub async fn spawn_server() -> (String, TempDir) {
    spawn_server_with_key(None).await
}

pub async fn spawn_server_with_key(admin_key: Option<&str>) -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let manager = flapjack::IndexManager::new(temp_dir.path());

    let key_store = admin_key.map(|k| {
        Arc::new(flapjack_http::auth::KeyStore::load_or_create(
            temp_dir.path(),
            k,
        ))
    });

    let state = Arc::new(flapjack_http::handlers::AppState {
        manager,
        key_store: key_store.clone(),
        replication_manager: None,
        ssl_manager: None,
    });

    let key_routes = if let Some(ref ks) = key_store {
        Router::new()
            .route(
                "/1/keys",
                post(flapjack_http::handlers::create_key).get(flapjack_http::handlers::list_keys),
            )
            .route(
                "/1/keys/:key",
                get(flapjack_http::handlers::get_key)
                    .put(flapjack_http::handlers::update_key)
                    .delete(flapjack_http::handlers::delete_key),
            )
            .route(
                "/1/keys/:key/restore",
                post(flapjack_http::handlers::restore_key),
            )
            .with_state(ks.clone())
    } else {
        Router::new()
    };

    let health_route = Router::new()
        .route("/health", get(flapjack_http::handlers::health))
        .with_state(state.clone());

    let protected = Router::new()
        .route("/1/indexes", post(flapjack_http::handlers::create_index))
        .route("/1/indexes", get(flapjack_http::handlers::list_indices))
        .route(
            "/1/indexes/:indexName/batch",
            post(flapjack_http::handlers::add_documents),
        )
        .route(
            "/1/indexes/:indexName/query",
            post(flapjack_http::handlers::search),
        )
        .route(
            "/1/indexes/:indexName/queries",
            post(flapjack_http::handlers::batch_search),
        )
        .route(
            "/1/indexes/:indexName/settings",
            get(flapjack_http::handlers::get_settings)
                .post(flapjack_http::handlers::set_settings)
                .put(flapjack_http::handlers::set_settings),
        )
        .route(
            "/1/indexes/:indexName/objects",
            post(flapjack_http::handlers::get_objects),
        )
        .route(
            "/1/indexes/:indexName/deleteByQuery",
            post(flapjack_http::handlers::delete_by_query),
        )
        .route(
            "/1/indexes/:indexName/:objectID/partial",
            post(flapjack_http::handlers::partial_update_object),
        )
        .route(
            "/1/indexes/:indexName/:objectID",
            get(flapjack_http::handlers::get_object)
                .delete(flapjack_http::handlers::delete_object)
                .put(flapjack_http::handlers::put_object),
        )
        .route(
            "/1/indexes/:indexName",
            post(flapjack_http::handlers::add_record_auto_id)
                .delete(flapjack_http::handlers::delete_index),
        )
        .route(
            "/1/indexes/:indexName/browse",
            post(flapjack_http::handlers::browse_index),
        )
        .route(
            "/1/indexes/:indexName/clear",
            post(flapjack_http::handlers::clear_index),
        )
        .route(
            "/1/indexes/:indexName/facets/:facetName/query",
            post(flapjack_http::handlers::search_facet_values),
        )
        .route(
            "/1/indexes/:indexName/synonyms/:objectID",
            get(flapjack_http::handlers::get_synonym)
                .put(flapjack_http::handlers::save_synonym)
                .delete(flapjack_http::handlers::delete_synonym),
        )
        .route(
            "/1/indexes/:indexName/synonyms/batch",
            post(flapjack_http::handlers::save_synonyms),
        )
        .route(
            "/1/indexes/:indexName/synonyms/clear",
            post(flapjack_http::handlers::clear_synonyms),
        )
        .route(
            "/1/indexes/:indexName/synonyms/search",
            post(flapjack_http::handlers::search_synonyms),
        )
        .route(
            "/1/indexes/:indexName/rules/:objectID",
            get(flapjack_http::handlers::get_rule)
                .put(flapjack_http::handlers::save_rule)
                .delete(flapjack_http::handlers::delete_rule),
        )
        .route(
            "/1/indexes/:indexName/rules/batch",
            post(flapjack_http::handlers::save_rules),
        )
        .route(
            "/1/indexes/:indexName/rules/clear",
            post(flapjack_http::handlers::clear_rules),
        )
        .route(
            "/1/indexes/:indexName/rules/search",
            post(flapjack_http::handlers::search_rules),
        )
        .route(
            "/1/indexes/:indexName/operation",
            post(flapjack_http::handlers::operation_index),
        )
        .route("/1/tasks/:task_id", get(flapjack_http::handlers::get_task))
        .route(
            "/1/indexes/:indexName/task/:task_id",
            get(flapjack_http::handlers::get_task_for_index),
        )
        .with_state(state.clone());

    let quickstart = Router::new()
        .route(
            "/indexes",
            get(flapjack_http::handlers::quickstart::qs_list_indexes),
        )
        .route(
            "/indexes/:indexName/search",
            get(flapjack_http::handlers::quickstart::qs_search_get)
                .post(flapjack_http::handlers::quickstart::qs_search_post),
        )
        .route(
            "/indexes/:indexName/documents",
            post(flapjack_http::handlers::quickstart::qs_add_documents),
        )
        .route(
            "/indexes/:indexName/documents/delete",
            post(flapjack_http::handlers::quickstart::qs_delete_documents),
        )
        .route(
            "/indexes/:indexName/documents/:docId",
            get(flapjack_http::handlers::quickstart::qs_get_document)
                .delete(flapjack_http::handlers::quickstart::qs_delete_document),
        )
        .route(
            "/indexes/:indexName/settings",
            get(flapjack_http::handlers::quickstart::qs_get_settings)
                .put(flapjack_http::handlers::quickstart::qs_set_settings),
        )
        .route(
            "/indexes/:indexName",
            delete(flapjack_http::handlers::quickstart::qs_delete_index),
        )
        .route(
            "/tasks/:taskId",
            get(flapjack_http::handlers::quickstart::qs_get_task),
        )
        .route(
            "/migrate",
            post(flapjack_http::handlers::quickstart::qs_migrate),
        )
        .with_state(state);

    let ks_for_middleware = key_store.clone();
    let auth_middleware = middleware::from_fn(
        move |mut request: axum::extract::Request, next: middleware::Next| {
            let ks = ks_for_middleware.clone();
            async move {
                if let Some(ref store) = ks {
                    request.extensions_mut().insert(store.clone());
                }
                flapjack_http::auth::authenticate_and_authorize(request, next).await
            }
        },
    );

    let app = Router::new()
        .merge(health_route)
        .merge(key_routes)
        .merge(protected)
        .layer(auth_middleware)
        .merge(quickstart);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (addr, temp_dir)
}
