#![cfg(feature = "inmem-store")]

use rib::{
    models::{NewBoard, UpdateBoard, NewThread, NewReply},
    repo::{inmem::InMemRepo, RepoError},
    auth::Role,
};
// Bring trait method namespaces into scope so calls on InMemRepo resolve.
use rib::repo::{BoardRepo, ThreadRepo, ReplyRepo, DiscordRoleRepo};

/// Helper that returns a fresh, empty repository for every test run.
fn repo() -> InMemRepo {
    // isolate state: do **not** persist to the default file path
    std::env::set_var("RIB_DATA_DIR", tempfile::tempdir().unwrap().path());
    InMemRepo::new()
}

#[tokio::test]
async fn board_crud_and_conflict() {
    let r = repo();

    // starts empty
    assert!(r.list_boards().await.unwrap().is_empty());

    // create a board
    let b = r
        .create_board(NewBoard {
            slug: "tech".into(),
            title: "Technology".into(),
        })
        .await
        .unwrap();
    assert_eq!(b.slug, "tech");

    // duplicate slug → conflict
    let err = r
        .create_board(NewBoard {
            slug: "tech".into(),
            title: "Dup".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, RepoError::Conflict));

    // update board
    let updated = r
        .update_board(
            b.id,
            UpdateBoard {
                slug: Some("g".into()),
                title: Some("Games".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.slug, "g");
    assert_eq!(updated.title, "Games");
}

#[tokio::test]
async fn thread_and_reply_flow() {
    let r = repo();

    // prerequisite board
    let board = r
        .create_board(NewBoard {
            slug: "a".into(),
            title: "Anime".into(),
        })
        .await
        .unwrap();

    // create thread
    let thread = r
        .create_thread(NewThread {
            board_id: board.id,
            subject: "First".into(),
            body: "OP body".into(),
            image_hash: None,
            mime: None,
        })
        .await
        .unwrap();
    assert_eq!(thread.board_id, board.id);

    // list threads
    let threads = r.list_threads(board.id).await.unwrap();
    assert_eq!(threads.len(), 1);

    // create reply
    let reply = r
        .create_reply(NewReply {
            thread_id: thread.id,
            content: "Hi".into(),
            image_hash: None,
            mime: None,
        })
        .await
        .unwrap();
    assert_eq!(reply.thread_id, thread.id);

    // list replies
    let replies = r.list_replies(thread.id).await.unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].id, reply.id);
}

#[tokio::test]
async fn discord_role_helpers() {
    let r = repo();

    // initially none
    assert!(r.get_discord_user_role("123").await.is_none());

    // set role
    r.set_discord_user_role("123", Role::Moderator)
        .await
        .unwrap();
    assert_eq!(
        r.get_discord_user_role("123").await.unwrap(),
        Role::Moderator
    );
}
