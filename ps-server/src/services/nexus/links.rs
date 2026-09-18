use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::{json, Value};

use crate::emitter::{Emitter, WeakEmitter};
use crate::messages::MessageType;

use super::now_secs;

pub const MAX_PENDING: usize = 16;

#[derive(Default)]
pub struct NexusLinks {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    pending: VecDeque<Value>,
    subscribers: Vec<WeakEmitter>,
}

pub fn link_payload(raw: &str, now_secs: u64) -> Value {
    match ps_core::nexus::parse_nxm(raw) {
        Ok(link) if link.is_expired(now_secs) => json!({
            "link": link,
            "error": { "code": "link_expired", "message": "this download link has expired" },
        }),
        Ok(link) => json!({ "link": link }),
        Err(error) => json!({ "error": { "code": error.code(), "message": error.to_string() } }),
    }
}

impl NexusLinks {
    pub fn push_raw(&self, raw: &str) {
        self.push(link_payload(raw, now_secs()));
    }

    pub fn push(&self, payload: Value) {
        let mut inner = self.lock();
        inner.subscribers.retain(WeakEmitter::is_connected);
        let live: Vec<Emitter> = inner
            .subscribers
            .iter()
            .filter_map(WeakEmitter::upgrade)
            .collect();
        if live.is_empty() {
            if inner.pending.len() == MAX_PENDING {
                inner.pending.pop_front();
            }
            inner.pending.push_back(payload);
            return;
        }
        for emitter in live {
            emitter.emit(MessageType::NexusLink, &payload);
        }
    }

    /// Queued links go out under the lock, so a link pushed meanwhile cannot
    /// arrive ahead of older ones.
    pub fn subscribe(&self, emitter: &Emitter) {
        let mut inner = self.lock();
        inner.subscribers.retain(WeakEmitter::is_connected);
        if !inner
            .subscribers
            .iter()
            .any(|subscriber| subscriber.is_same_connection(emitter))
        {
            inner.subscribers.push(emitter.downgrade());
        }
        for payload in inner.pending.drain(..) {
            emitter.emit(MessageType::NexusLink, &payload);
        }
    }

    pub fn pending_len(&self) -> usize {
        self.lock().pending.len()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(receiver: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(text) = receiver.try_recv() {
            out.push(serde_json::from_str(&text).unwrap());
        }
        out
    }

    #[test]
    fn payloads_describe_links_expiry_and_rejections() {
        let ok = link_payload(
            "nxm://palworld/mods/4821/files/99001?key=k&expires=200",
            100,
        );
        assert_eq!(ok["link"]["mod_id"], 4821);
        assert!(ok.get("error").is_none());
        let expired = link_payload(
            "nxm://palworld/mods/4821/files/99001?key=k&expires=100",
            100,
        );
        assert_eq!(expired["error"]["code"], "link_expired");
        assert_eq!(expired["link"]["file_id"], 99001);
        let rejected = link_payload("nxm://palworld/collections/x/revisions/1", 100);
        assert_eq!(rejected["error"]["code"], "unsupported_link");
        assert!(rejected.get("link").is_none());
    }

    #[test]
    fn links_queue_until_a_subscriber_and_then_arrive_once_per_connection() {
        let links = NexusLinks::default();
        links.push_raw("nxm://palworld/mods/1/files/2");
        assert_eq!(links.pending_len(), 1);

        let (emitter, mut receiver) = Emitter::test_channel();
        links.subscribe(&emitter);
        links.subscribe(&emitter.clone());
        assert_eq!(links.pending_len(), 0);
        let queued = frames(&mut receiver);
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0]["type"], "nexus_link");
        assert_eq!(queued[0]["data"]["link"]["file_id"], 2);

        links.push_raw("nxm://palworld/mods/1/files/3");
        assert_eq!(
            frames(&mut receiver).len(),
            1,
            "one frame despite two subscribes"
        );
    }

    #[test]
    fn a_closed_connection_stops_receiving_and_links_queue_again() {
        let links = NexusLinks::default();
        let (emitter, receiver) = Emitter::test_channel();
        links.subscribe(&emitter);
        drop(receiver);
        links.push_raw("nxm://palworld/mods/1/files/2");
        assert_eq!(links.pending_len(), 1);
    }

    #[test]
    fn at_most_sixteen_links_wait() {
        let links = NexusLinks::default();
        for file_id in 1..=20 {
            links.push_raw(&format!("nxm://palworld/mods/1/files/{file_id}"));
        }
        assert_eq!(links.pending_len(), MAX_PENDING);
        let (emitter, mut receiver) = Emitter::test_channel();
        links.subscribe(&emitter);
        let delivered = frames(&mut receiver);
        assert_eq!(
            delivered[0]["data"]["link"]["file_id"], 5,
            "the oldest were dropped"
        );
    }
}
