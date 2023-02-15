use crate::event::{Access, CountValue, Event, PackedData, ToggleCounter, Variation};
use headers::HeaderValue;
use parking_lot::{Mutex, RwLock};
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
        user_agent: String,
        flush_interval: Duration,
        capacity: usize,
        should_stop: Arc<RwLock<bool>>,
    ) -> Self {
        let slf = Self {
            inner: Arc::new(Inner {
                auth,
                user_agent,
                events_url,
                flush_interval,
                capacity,
                incoming_events: Default::default(),
                packed_data: Default::default(),
                should_stop,
            }),
        };

        slf.start();
        slf
    }

    pub fn flush(&self) {
        #[cfg(feature = "use_std")]
        self.inner.do_flush();
        #[cfg(fature = "use_tokio")]
        {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                self.inner.do_async_flush(&client).await;
                let _ = tx.send(());
            });
            let _ = rx.recv();
        }
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
                if *inner.should_stop.read() {
                    break;
                }
            }
        });
    }

    #[cfg(feature = "use_std")]
    fn std_start(&self) {
        let inner = self.inner.clone();
        std::thread::spawn(move || loop {
            inner.do_flush();
            std::thread::sleep(inner.flush_interval);
            if *inner.should_stop.read() {
                break;
            }
        });
    }

    // TODO: performance
    pub fn record_event(&self, event: Event) {
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
    pub user_agent: String,
    pub events_url: Url,
    pub flush_interval: Duration,
    pub capacity: usize,
    pub incoming_events: Mutex<Option<Vec<Event>>>,
    pub packed_data: Mutex<Option<VecDeque<PackedData>>>,
    pub should_stop: Arc<RwLock<bool>>,
}

impl Inner {
    #[cfg(feature = "use_tokio")]
    async fn do_async_flush(&self, client: &Client) {
        use reqwest::header::USER_AGENT;
        use tracing::debug;

        let events = match self.take_events() {
            Some(v) if !v.is_empty() => v,
            _ => return,
        };

        let packed_data = self.build_packed_data(events);
        let request = client
            .request(Method::POST, self.events_url.clone())
            .header(AUTHORIZATION, &self.auth)
            .header(USER_AGENT, &self.user_agent)
            .timeout(self.flush_interval)
            .json(&packed_data);

        //TODO: report failure
        debug!("flush req: {:?}", request);
        match request.send().await {
            Err(e) => {
                error!("event post error: {}", e);
                self.set_packed_data(packed_data); // put back
            }
            Ok(r) => debug!("flush resp: {:?}", r),
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
            .set("user-agent", &self.user_agent)
            .timeout(self.flush_interval)
            .set("Content-Type", "application/json")
            .send_string(&body)
        {
            error!("event post error: {}", e);
            self.set_packed_data(packed_data); // put back
        }
    }

    fn take_events(&self) -> Option<Vec<Event>> {
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

    fn build_events(&self, events: &Vec<Event>) -> Vec<Event> {
        let mut res: Vec<Event> = Vec::new();
        for e in events {
            match e {
                Event::AccessEvent(access_event) => {
                    if access_event.track_access_events {
                        res.push(Event::AccessEvent(access_event.clone()));
                    }
                }
                _ => res.push(e.clone()),
            }
        }
        res
    }

    fn build_access(&self, events: &Vec<Event>) -> Access {
        let mut start_time = u128::MAX;
        let mut end_time = 0;
        let mut counters: HashMap<Variation, CountValue> = HashMap::new();

        for e in events {
            if let Event::AccessEvent(access_event) = e {
                if access_event.time < start_time {
                    start_time = access_event.time;
                }
                if access_event.time > end_time {
                    end_time = access_event.time
                }
                let variation = Variation {
                    key: access_event.key.clone(),
                    version: access_event.version,
                    index: access_event.variation_index,
                };

                let count_value = counters.entry(variation).or_insert(CountValue {
                    count: 0,
                    value: access_event.value.clone(),
                });
                count_value.count += 1;
            }
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

    fn build_packed_data(&self, events: Vec<Event>) -> Option<VecDeque<PackedData>> {
        let access = self.build_access(&events);
        let events = self.build_events(&events);
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
