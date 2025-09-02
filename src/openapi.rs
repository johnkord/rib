use utoipa::OpenApi;
use crate::models::{Board, NewBoard, Thread, NewThread, Reply, NewReply, Image, Report};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::list_boards,
        crate::routes::create_board,
        crate::routes::list_threads,
        crate::routes::create_thread,
        crate::routes::list_replies,
        crate::routes::create_reply,
        crate::routes::upload_image,
    ),
    components(schemas(
        Board, NewBoard, Thread, NewThread, Reply, NewReply,
        Image, Report, crate::routes::ImageUploadResponse
     )),
    tags(
        (name = "boards", description = "Board operations"),
        (name = "threads", description = "Thread operations"),
        (name = "replies", description = "Reply operations"),
    )
)]
pub struct ApiDoc;