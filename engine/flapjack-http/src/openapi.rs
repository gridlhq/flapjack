use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Flapjack API",
        version = "0.1.0",
        description = "Drop-in replacement for Algolia search. Provides full-text search with filters, facets, geo-search, and more.",
        license(name = "MIT OR Apache-2.0"),
        contact(
            name = "Flapjack",
            url = "https://github.com/stuartcrobinson/flapjack"
        )
    ),
    servers(
        (url = "http://localhost:7700", description = "Local development"),
        (url = "https://fj-us-west-1.flapjack.foo", description = "Production US West")
    ),
    paths(
        crate::handlers::health::health,
        crate::handlers::indices::create_index,
        crate::handlers::indices::delete_index,
        crate::handlers::indices::list_indices,
        crate::handlers::indices::clear_index,
        crate::handlers::indices::operation_index,
        crate::handlers::search::search,
        crate::handlers::search::batch_search,
        crate::handlers::objects::add_documents,
        crate::handlers::objects::get_object,
        crate::handlers::objects::delete_object,
        crate::handlers::objects::put_object,
        crate::handlers::objects::get_objects,
        crate::handlers::objects::delete_by_query,
        crate::handlers::browse::browse_index,
        crate::handlers::facets::search_facet_values,
        crate::handlers::settings::get_settings,
        crate::handlers::settings::set_settings,
        crate::handlers::tasks::get_task,
        crate::handlers::tasks::get_task_for_index,
        crate::handlers::synonyms::get_synonym,
        crate::handlers::synonyms::save_synonym,
        crate::handlers::synonyms::delete_synonym,
        crate::handlers::synonyms::save_synonyms,
        crate::handlers::synonyms::clear_synonyms,
        crate::handlers::synonyms::search_synonyms,
        crate::handlers::rules::get_rule,
        crate::handlers::rules::save_rule,
        crate::handlers::rules::delete_rule,
        crate::handlers::rules::save_rules,
        crate::handlers::rules::clear_rules,
        crate::handlers::rules::search_rules,
        crate::handlers::keys::create_key,
        crate::handlers::keys::list_keys,
        crate::handlers::keys::get_key,
        crate::handlers::keys::update_key,
        crate::handlers::keys::delete_key,
        crate::handlers::keys::restore_key,
        crate::handlers::keys::generate_secured_key,
        crate::handlers::snapshot::export_snapshot,
        crate::handlers::snapshot::import_snapshot,
        crate::handlers::snapshot::snapshot_to_s3,
        crate::handlers::snapshot::restore_from_s3,
        crate::handlers::snapshot::list_s3_snapshots,
    ),
    components(
        schemas(
            crate::dto::CreateIndexRequest,
            crate::dto::IndexSchema,
            crate::handlers::indices::CreateIndexResponse,
            crate::handlers::indices::OperationIndexRequest,
            crate::dto::SearchRequest,
            crate::dto::AddDocumentsRequest,
            crate::dto::BatchOperation,
            crate::dto::AddDocumentsResponse,
            crate::dto::GetObjectsRequest,
            crate::dto::GetObjectRequest,
            crate::dto::GetObjectsResponse,
            crate::dto::DeleteByQueryRequest,
            crate::dto::SearchFacetValuesRequest,
            crate::dto::SearchFacetValuesResponse,
            crate::dto::FacetHit,
            crate::dto::TaskResponse,
            crate::dto::DocFailureDto,
        )
    ),
    tags(
        (name = "health", description = "Health check"),
        (name = "indices", description = "Index management operations"),
        (name = "search", description = "Search and query operations"),
        (name = "documents", description = "Document CRUD operations"),
        (name = "settings", description = "Index settings"),
        (name = "synonyms", description = "Synonym management"),
        (name = "rules", description = "Query rules"),
        (name = "keys", description = "API key management"),
        (name = "snapshots", description = "Backup and restore operations"),
        (name = "tasks", description = "Task status endpoints"),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("x-algolia-api-key"),
                    ),
                ),
            );
        }
    }
}
