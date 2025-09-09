use crate::models::{Board, Image, NewBoard, NewReply, NewThread, Reply, Report, Thread};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::list_boards,
        crate::routes::create_board,
        crate::routes::list_threads,
        crate::routes::create_thread,
        crate::routes::list_replies,
        crate::routes::create_reply,
        crate::routes::bitcoin_challenge,
        crate::routes::bitcoin_verify,
        crate::routes::upload_image,
    crate::routes::set_subject_role,
    crate::routes::list_roles,
    crate::routes::delete_role,
    ),
    components(schemas(
        Board, NewBoard, Thread, NewThread, Reply, NewReply,
        Image, Report, crate::routes::ImageUploadResponse,
        crate::routes::BitcoinChallengeRequest, crate::routes::BitcoinChallengeResponse,
        crate::routes::BitcoinVerifyRequest, crate::routes::BitcoinVerifyResponse
    ,crate::routes::SetSubjectRoleRequest, crate::routes::RoleAssignment
     )),
    tags(
        (name = "boards", description = "Board operations"),
        (name = "threads", description = "Thread operations"),
        (name = "replies", description = "Reply operations"),
    )
)]
pub struct ApiDoc;
