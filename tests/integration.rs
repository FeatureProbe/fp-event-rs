#[cfg(feature = "collector")]
mod tests {

    use axum::{
        async_trait,
        extract::Extension,
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use feature_probe_event::{
        collector::{cors_headers, post_events, EventHandler, FPEventError},
        event::{AccessEvent, PackedData},
        recorder::EventRecorder,
    };
    use headers::HeaderValue;
    use lazy_static::lazy_static;
    use parking_lot::Mutex;
    use reqwest::StatusCode;
    use serde_json::json;
    use std::{collections::VecDeque, net::SocketAddr, time::Duration};

    lazy_static! {
        static ref IS_SUCCESS: Mutex<bool> = Mutex::new(false);
    }

    #[tokio::test]
    async fn test_recorder() {
        let _ = tracing_subscriber::fmt().init();

        setup_collector().await;
        let recorder = setup_recorder();
        let event = AccessEvent {
            time: 1,
            key: "key".to_owned(),
            value: json!("1"),
            index: None,
            version: Some(1),
            reason: "reason".to_owned(),
        };
        recorder.record_access(event.clone());
        recorder.record_access(event.clone());
        recorder.record_access(event.clone());
        recorder.record_access(event.clone());
        recorder.record_access(event);

        tokio::time::sleep(Duration::from_secs(3)).await;

        let guard = IS_SUCCESS.lock();
        assert!(*guard == true);
    }

    async fn setup_collector() {
        tokio::spawn(async move {
            let handler = MockHandler {};
            let app = Router::new()
                .route("/", post(post_events::<MockHandler>))
                .layer(Extension(handler));
            let addr = SocketAddr::from(([127, 0, 0, 1], 19919));
            axum::Server::bind(&addr)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
    }

    fn setup_recorder() -> EventRecorder {
        let events_url = "http://127.0.0.1:19919/".parse().unwrap();
        let auth = HeaderValue::from_static("auth");
        let flush_interval = Duration::from_secs(1);
        let capacity = 10;
        let user_agent = "Rust".to_owned();
        EventRecorder::new(events_url, auth, user_agent, flush_interval, capacity)
    }

    #[derive(Clone)]
    struct MockHandler {}

    #[async_trait]
    impl EventHandler for MockHandler {
        async fn handle_events(
            &self,
            sdk_key: String,
            _user_agent: String,
            mut data: VecDeque<PackedData>,
        ) -> Result<Response, FPEventError> {
            assert!(sdk_key.len() > 0);
            assert!(data.len() == 1);

            let packed_data = data.pop_front().unwrap();
            let counters = packed_data.access.counters;
            assert!(counters.len() > 0);

            let v = counters.get("key").unwrap().first().unwrap();
            assert!(v.count == 5);

            let mut guard = IS_SUCCESS.lock();
            *guard = true;

            Ok((StatusCode::OK, cors_headers(), "").into_response())
        }
    }
}
