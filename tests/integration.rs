#[cfg(feature = "collector")]
mod tests {

    use axum::{
        async_trait,
        extract::Extension,
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use feature_probe_event::event::Event;
    use feature_probe_event::{
        collector::{cors_headers, post_events, EventHandler, FPEventError},
        event::{AccessEvent, CustomEvent},
        recorder::EventRecorder,
    };
    use headers::{HeaderMap, HeaderValue};
    use lazy_static::lazy_static;
    use parking_lot::{Mutex, RwLock};
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use std::{collections::VecDeque, net::SocketAddr, sync::Arc, time::Duration};

    lazy_static! {
        static ref IS_SUCCESS: Mutex<bool> = Mutex::new(false);
    }

    #[tokio::test]
    async fn test_recorder() {
        let _ = tracing_subscriber::fmt().init();

        setup_collector().await;
        let recorder = setup_recorder();
        let access_event = AccessEvent {
            kind: "access".to_string(),
            time: 1,
            key: "key".to_owned(),
            value: json!("1"),
            user: "user_key".to_string(),
            user_detail: serde_json::to_value("{}").unwrap(),
            variation_index: 0,
            version: Some(1),
            rule_index: Some(1),
            reason: Some("reason".to_string()),
            track_access_events: false,
            track_debug_until_date: 0,
        };

        let custom_event = CustomEvent {
            kind: "custom".to_string(),
            time: 1,
            user: "user_key".to_string(),
            name: "event_name".to_string(),
            value: None,
        };

        recorder.record_event(Event::AccessEvent(access_event.clone()));
        recorder.record_event(Event::AccessEvent(access_event.clone()));
        recorder.record_event(Event::AccessEvent(access_event.clone()));
        recorder.record_event(Event::AccessEvent(access_event.clone()));
        recorder.record_event(Event::AccessEvent(access_event.clone()));

        recorder.record_event(Event::CustomEvent(custom_event.clone()));
        recorder.record_event(Event::CustomEvent(custom_event.clone()));
        recorder.record_event(Event::CustomEvent(custom_event.clone()));
        recorder.record_event(Event::CustomEvent(custom_event.clone()));
        recorder.record_event(Event::CustomEvent(custom_event.clone()));

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
        let should_stop = Arc::new(RwLock::new(false));
        EventRecorder::new(
            events_url,
            auth,
            user_agent,
            flush_interval,
            capacity,
            should_stop,
        )
    }

    #[derive(Clone)]
    struct MockHandler {}

    #[async_trait]
    impl EventHandler for MockHandler {
        async fn handle_events(
            &self,
            sdk_key: String,
            _user_agent: String,
            _headers: HeaderMap,
            data: VecDeque<Value>,
        ) -> Result<Response, FPEventError> {
            assert!(sdk_key.len() > 0);
            assert!(data.len() == 1);

            let mut guard = IS_SUCCESS.lock();
            *guard = true;

            Ok((StatusCode::OK, cors_headers(), "").into_response())
        }
    }
}
