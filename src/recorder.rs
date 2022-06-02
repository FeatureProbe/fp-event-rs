use crate::event::{Access, AccessEvent, CountValue, PackedData, ToggleCounter, Variation};
use headers::HeaderValue;
use parking_lot::Mutex;
#[cfg(feature = "use_tokio")]
use reqwest::{header::AUTHORIZATION, Client, Method};
use std::collections::{HashMap, VecDeque};
use std::{sync::Arc, time::Duration};
use tracing::error;
use url::Url;

#[derive(Debug, Clone)]
pub struct EventRecorder {
    inner: Arc<Inner>,
}

impl EventRecorder {
    pub fn new(
        events_url: Url,
        auth: HeaderValue,
        flush_interval: Duration,
        capacity: usize,
    ) -> Self {
        let slf = Self {
            inner: Arc::new(Inner {
                auth,
                events_url,
                flush_interval,
                capacity,
                incoming_events: Default::default(),
                packed_data: Default::default(),
            }),
        };

        slf.start();
        slf
    }

    fn start(&self) {
        #[cfg(feature = "use_tokio")]
        self.tokio_start();

        #[cfg(feature = "use_std")]
        self.std_start();
    }

    #[cfg(feature = "use_tokio")]
    fn tokio_start(&self) {
        let inner = self.inner.clone();
        let client = reqwest::Client::new();
        // TODO: gracefull shutdown
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(inner.flush_interval);
            loop {
                inner.do_async_flush(&client).await;
                interval.tick().await;
            }
        });
    }

    #[cfg(feature = "use_std")]
    fn std_start(&self) {
        let inner = self.inner.clone();
        std::thread::spawn(move || loop {
            inner.do_flush();
            std::thread::sleep(inner.flush_interval);
        });
    }

    // TODO: performance
    pub fn record_access(&self, event: AccessEvent) {
        let mut guard = self.inner.incoming_events.lock();
        let mut events = guard.take();

        match events {
            None => events = Some(vec![event]),
            Some(ref mut v) => v.push(event),
        };

        *guard = events;
    }
}

#[derive(Debug)]
struct Inner {
    pub auth: HeaderValue,
    pub events_url: Url,
    pub flush_interval: Duration,
    pub capacity: usize,
    pub incoming_events: Mutex<Option<Vec<AccessEvent>>>,
    pub packed_data: Mutex<Option<VecDeque<PackedData>>>,
}

impl Inner {
    #[cfg(feature = "use_tokio")]
    async fn do_async_flush(&self, client: &Client) {
        use tracing::info;

        let events = match self.take_events() {
            Some(v) if !v.is_empty() => v,
            _ => return,
        };

        let packed_data = self.build_packed_data(events);
        let request = client
            .request(Method::POST, self.events_url.clone())
            .header(AUTHORIZATION, self.auth.clone())
            .timeout(self.flush_interval)
            .json(&packed_data);

        //TODO: report failure
        match request.send().await {
            Err(e) => {
                error!("event post error: {}", e);
                self.set_packed_data(packed_data); // put back
            }
            Ok(r) => info!("{:?}", r),
        }
    }

    #[cfg(feature = "use_std")]
    fn do_flush(&self) {
        let events = match self.take_events() {
            Some(v) if !v.is_empty() => v,
            _ => return,
        };

        let packed_data = self.build_packed_data(events);
        let body = match serde_json::to_string(&packed_data) {
            Err(e) => {
                error!("{:?}", e);
                return;
            }
            Ok(s) => s,
        };

        //TODO: report failure
        if let Err(e) = ureq::post(self.events_url.as_str())
            .set(
                "authorization",
                self.auth.to_str().expect("already valid header value"),
            )
            .timeout(self.flush_interval)
            .set("Content-Type", "application/json")
            .send_string(&body)
        {
            error!("event post error: {}", e);
            self.set_packed_data(packed_data); // put back
        }
    }

    fn take_events(&self) -> Option<Vec<AccessEvent>> {
        let mut guard = self.incoming_events.lock();
        guard.take()
    }

    fn take_packed_data(&self) -> Option<VecDeque<PackedData>> {
        let mut guard = self.packed_data.lock();
        guard.take()
    }

    fn set_packed_data(&self, packed_data: Option<VecDeque<PackedData>>) {
        let mut guard = self.packed_data.lock();
        *guard = packed_data
    }

    fn build_access(&self, events: &Vec<AccessEvent>) -> Access {
        let mut start_time = u128::MAX;
        let mut end_time = 0;
        let mut counters: HashMap<Variation, CountValue> = HashMap::new();

        for e in events {
            if e.time < start_time {
                start_time = e.time;
            }
            if e.time > end_time {
                end_time = e.time
            }
            let variation = Variation {
                key: e.key.clone(),
                version: e.version,
                index: e.index,
            };

            let count_value = counters.entry(variation).or_insert(CountValue {
                count: 0,
                value: e.value.clone(),
            });
            count_value.count += 1;
        }

        let mut access = Access {
            start_time,
            end_time,
            counters: Default::default(),
        };

        for (k, v) in counters {
            let counter = ToggleCounter {
                index: k.index,
                version: k.version,
                value: v.value,
                count: v.count,
            };
            let vec = access.counters.entry(k.key).or_insert(Vec::new());
            vec.push(counter);
        }

        access
    }

    fn build_packed_data(&self, events: Vec<AccessEvent>) -> Option<VecDeque<PackedData>> {
        let access = self.build_access(&events);
        let packed_data = PackedData { events, access };
        let mut packed_data_vec = self.take_packed_data();
        match packed_data_vec {
            None => {
                packed_data_vec = {
                    let mut vecdeque = VecDeque::new();
                    vecdeque.push_back(packed_data);
                    Some(vecdeque)
                }
            }
            Some(ref mut v) => {
                if v.len() > self.capacity {
                    let _ = v.pop_front();
                }
                v.push_back(packed_data)
            }
        };
        packed_data_vec
    }
}

pub fn unix_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards!")
        .as_millis()
}