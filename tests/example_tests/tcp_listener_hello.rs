use super::utils;
use rama::{http::BodyExtractExt, service::Context};

const EXPECTED_FILE_CONTENT: &str = include_str!("../../examples/tcp_listener_hello.rs");

#[tokio::test]
#[ignore]
async fn test_tcp_listener_hello() {
    let runner = utils::ExampleRunner::interactive("tcp_listener_hello");

    let file_content = runner
        .get("http://localhost:40500")
        .send(Context::default())
        .await
        .unwrap()
        .try_into_string()
        .await
        .unwrap();

    assert_eq!(EXPECTED_FILE_CONTENT, file_content);
}