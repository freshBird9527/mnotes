use async_trait::async_trait;
use bytes::Bytes;
use http::{Response, StatusCode};
use log::debug;
use pingora_timeout::timeout;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use pingora_core::apps::http_app::ServeHttp;
use pingora_core::apps::ServerApp;
use pingora_core::protocols::http::ServerSession;
use pingora_core::protocols::Stream;
use pingora_core::server::configuration::Opt;
use pingora_core::server::{Server, ShutdownWatch};
use pingora_core::services::listening::Service;

#[derive(Clone)]
pub struct EchoApp;

#[async_trait]
impl ServerApp for EchoApp {
    async fn process_new(
        self: &Arc<Self>,
        mut io: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        let mut buf = [0; 1024];
        loop {
            let n = io.read(&mut buf).await.unwrap();
            if n == 0 {
                debug!("session closing");
                return None;
            }
            io.write_all(&buf[0..n]).await.unwrap();
            io.flush().await.unwrap();
        }
    }
}

pub struct HttpEchoApp;

#[async_trait]
impl ServeHttp for HttpEchoApp {
    async fn response(&self, http_stream: &mut ServerSession) -> Response<Vec<u8>> {
        // read timeout of 2s
        let read_timeout = 2000;
        let body = match timeout(
            Duration::from_millis(read_timeout),
            http_stream.read_request_body(),
        )
        .await
        {
            Ok(res) => match res.unwrap() {
                Some(bytes) => bytes,
                None => Bytes::from("no body!"),
            },
            Err(_) => {
                panic!("Timed out after {:?}ms", read_timeout);
            }
        };

        Response::builder()
            .status(StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "text/html")
            .header(http::header::CONTENT_LENGTH, body.len())
            .body(body.to_vec())
            .unwrap()
    }
}

fn main() {
    env_logger::init();

    let opt = Some(Opt::parse_args());
    let mut server = Server::new(opt).unwrap();

    server.bootstrap();

    let mut echo_service_l4 = Service::new("echo_service_l4".to_string(), EchoApp);
    let mut echo_service_l7 = Service::new("echo_service_l7".to_string(), HttpEchoApp);

    echo_service_l4.add_tcp("0.0.0.0:6666");
    echo_service_l7.add_tcp("0.0.0.0:8888");

    server.add_service(echo_service_l4);
    server.add_service(echo_service_l7);

    server.run_forever();
}
