use rib::models::{NewBoard, NewThread, PublicIdentity};
use rib::repo::pg::PgRepo;
use rib::repo::{BoardRepo, ThreadRepo};

#[actix_web::test]
async fn duplicate_blob_can_be_attached_to_multiple_threads() {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("connect test database");
    let repo = PgRepo::new(pool);
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let board = repo
        .create_board(NewBoard {
            slug: format!("dup{}", &suffix[..8]),
            title: "Duplicate attachment test".to_string(),
        })
        .await
        .expect("create board");
    let hash = "a".repeat(64);

    let first = repo
        .create_thread(
            NewThread {
                board_id: board.id,
                subject: "first".to_string(),
                body: "first".to_string(),
                image_hash: Some(hash.clone()),
                mime: Some("image/png".to_string()),
                author_name: None,
                tripcode_password: None,
            },
            serde_json::json!({"provider":"test"}),
            PublicIdentity::default(),
        )
        .await
        .expect("create first thread");
    let second = repo
        .create_thread(
            NewThread {
                board_id: board.id,
                subject: "second".to_string(),
                body: "second".to_string(),
                image_hash: Some(hash.clone()),
                mime: Some("image/png".to_string()),
                author_name: None,
                tripcode_password: None,
            },
            serde_json::json!({"provider":"test"}),
            PublicIdentity::default(),
        )
        .await
        .expect("create second thread");

    assert_eq!(first.image_hash.as_deref(), Some(hash.as_str()));
    assert_eq!(second.image_hash.as_deref(), Some(hash.as_str()));
}
