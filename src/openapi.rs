use crate::models::{
    Board, Image, NewBoard, NewReply, NewSubjectBan, NewThread, Reply, Report, SubjectBan, Thread,
};
use utoipa::{Modify, OpenApi};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{Http, HttpAuthScheme, SecurityScheme};

        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::list_boards,
        crate::routes::create_board,
        crate::routes::list_threads,
        crate::routes::create_thread,
        crate::routes::get_thread,
        crate::routes::list_replies,
        crate::routes::create_reply,
        crate::routes::update_board,
        crate::routes::auth_me,
        crate::routes::bitcoin_challenge,
        crate::routes::bitcoin_verify,
        crate::routes::upload_image,
        crate::routes::set_subject_role,
        crate::routes::list_roles,
        crate::routes::delete_role,
        crate::routes::get_thread_author,
        crate::routes::get_reply_author,
        crate::routes::create_subject_ban,
        crate::routes::list_subject_bans,
        crate::routes::delete_subject_ban,
    ),
    components(schemas(
        Board, NewBoard, Thread, NewThread, Reply, NewReply,
        Image, Report, SubjectBan, NewSubjectBan, crate::routes::FileUploadResponse,
        crate::routes::BitcoinChallengeRequest, crate::routes::BitcoinChallengeResponse,
        crate::routes::BitcoinVerifyRequest, crate::routes::BitcoinVerifyResponse,
        crate::routes::SetSubjectRoleRequest, crate::routes::RoleAssignment,
        crate::routes::AuthorAttribution
     )),
    tags(
        (name = "boards", description = "Board operations"),
        (name = "threads", description = "Thread operations"),
        (name = "replies", description = "Reply operations"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::ApiDoc;
    use utoipa::OpenApi;

    #[test]
    fn documents_moderation_endpoints_and_bearer_scheme() {
        let document = serde_json::to_value(ApiDoc::openapi()).expect("serialize OpenAPI");
        assert!(document["paths"]
            .get("/api/v1/admin/threads/{id}/author")
            .is_some());
        assert!(document["paths"].get("/api/v1/admin/bans").is_some());
        assert!(document["components"]["securitySchemes"]
            .get("bearer_auth")
            .is_some());
    }
}
