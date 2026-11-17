use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use iroh::{
    EndpointId, SecretKey,
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler},
};
use iroh_chain::{EndpointIdStore, PairingOffer, Server};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{preset::WebPreset, stream::WebStream};

type Callbacks = Arc<Mutex<HashMap<u32, js_sys::Function>>>;

#[derive(Clone, Debug)]
struct Echo;

impl ProtocolHandler for Echo {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        while let Ok((mut send, mut recv)) = connection.accept_bi().await {
            let bytes = recv
                .read_to_end(1024 * 1024)
                .await
                .map_err(std::io::Error::other)?;
            send.write_all(&bytes)
                .await
                .map_err(std::io::Error::other)?;
            send.finish().map_err(std::io::Error::other)?;
        }
        Ok(())
    }
}

#[wasm_bindgen]
pub struct WebServer {
    server: Server,
    message_callbacks: Callbacks,
    pairing_callbacks: Callbacks,
    offers: Arc<Mutex<HashMap<u32, PairingOffer>>>,
    next_handle: Arc<AtomicU32>,
}

#[wasm_bindgen]
impl WebServer {
    #[wasm_bindgen]
    pub async fn create(
        secret_key_hex: Option<String>,
        relay_urls: Vec<String>,
    ) -> Result<WebServer, JsError> {
        let secret_key = match secret_key_hex {
            Some(hex) => {
                SecretKey::from_str(&hex).map_err(|error| JsError::new(&error.to_string()))?
            }
            None => SecretKey::generate(),
        };
        let preset = WebPreset::new(&relay_urls).map_err(|error| JsError::new(&error))?;
        let allowed = EndpointIdStore::new();
        let server = Server::bind_with_secret_key(preset, secret_key, allowed)
            .await
            .map_err(|error| JsError::new(&error.to_string()))?;
        server.router(Echo);
        let web = Self {
            server,
            message_callbacks: Arc::default(),
            pairing_callbacks: Arc::default(),
            offers: Arc::default(),
            next_handle: Arc::new(AtomicU32::new(1)),
        };
        web.spawn_message_pump();
        web.spawn_pairing_pump();
        Ok(web)
    }

    pub fn endpoint_id(&self) -> String {
        self.server.endpoint().id().to_string()
    }

    pub async fn broadcast(&self, payload: &[u8]) -> Result<(), JsError> {
        self.server
            .broadcast(payload)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub async fn send(&self, destination_id: &str, payload: &[u8]) -> Result<(), JsError> {
        let id = EndpointId::from_str(destination_id)
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.server
            .send(id, payload)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub fn subscribe_messages(&self, callback: js_sys::Function) -> u32 {
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        self.message_callbacks
            .lock()
            .expect("message callbacks poisoned")
            .insert(handle, callback);
        handle
    }

    pub fn unsubscribe_messages(&self, handle: u32) {
        self.message_callbacks
            .lock()
            .expect("message callbacks poisoned")
            .remove(&handle);
    }

    pub fn peers(&self) -> Vec<String> {
        self.server
            .peers()
            .ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    pub fn add_peer(&self, id: &str) -> Result<(), JsError> {
        let id = EndpointId::from_str(id).map_err(|error| JsError::new(&error.to_string()))?;
        self.server.peers().add(id);
        Ok(())
    }

    pub fn remove_peer(&self, id: &str) -> Result<(), JsError> {
        let id = EndpointId::from_str(id).map_err(|error| JsError::new(&error.to_string()))?;
        self.server.peers().remove(id);
        Ok(())
    }

    pub fn set_pairing_enabled(&self, enabled: bool) {
        self.server.set_pairing_enabled(enabled);
    }

    pub fn subscribe_pairing_offers(&self, callback: js_sys::Function) -> u32 {
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
        self.pairing_callbacks
            .lock()
            .expect("pairing callbacks poisoned")
            .insert(handle, callback);
        handle
    }

    pub fn unsubscribe_pairing_offers(&self, handle: u32) {
        self.pairing_callbacks
            .lock()
            .expect("pairing callbacks poisoned")
            .remove(&handle);
    }

    pub async fn reply_to_pairing_offer(
        &self,
        offer_handle: u32,
        payload: &[u8],
    ) -> Result<(), JsError> {
        let offer = self
            .offers
            .lock()
            .expect("offers poisoned")
            .get(&offer_handle)
            .cloned();
        let Some(offer) = offer else {
            return Err(JsError::new("unknown pairing offer handle"));
        };
        offer
            .reply(payload)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub async fn send_pairing_offer(
        &self,
        destination_id: &str,
        payload: &[u8],
    ) -> Result<Vec<u8>, JsError> {
        let id = EndpointId::from_str(destination_id)
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.server
            .send_pairing_offer(id, payload)
            .await
            .map_err(|error| JsError::new(&error.to_string()))
    }

    pub async fn open_stream(&self, peer_id: &str) -> Result<WebStream, JsError> {
        let id = EndpointId::from_str(peer_id).map_err(|error| JsError::new(&error.to_string()))?;
        let (connection, send, recv) = self
            .server
            .open_stream(id)
            .await
            .map_err(|error| JsError::new(&error.to_string()))?;
        Ok(WebStream::new(connection, send, recv))
    }

    pub async fn close(&self) {
        self.server.close().await;
    }
}

impl WebServer {
    fn spawn_message_pump(&self) {
        let mut receiver = self.server.subscribe_messages();
        let callbacks = Arc::clone(&self.message_callbacks);
        spawn_local(async move {
            loop {
                let Ok(message) = receiver.recv().await else {
                    break;
                };
                let callbacks = callbacks.lock().expect("message callbacks poisoned");
                for callback in callbacks.values() {
                    let source = JsValue::from(message.source.to_string());
                    let destination = message
                        .destination
                        .map(|id| JsValue::from(id.to_string()))
                        .unwrap_or(JsValue::NULL);
                    let payload = js_sys::Uint8Array::from(message.payload.as_slice());
                    let _ = callback.call3(&JsValue::NULL, &source, &destination, &payload);
                }
            }
        });
    }

    fn spawn_pairing_pump(&self) {
        let mut receiver = self.server.subscribe_pairing_offers();
        let callbacks = Arc::clone(&self.pairing_callbacks);
        let offers = Arc::clone(&self.offers);
        let next_handle = Arc::clone(&self.next_handle);
        spawn_local(async move {
            loop {
                let Ok(offer) = receiver.recv().await else {
                    break;
                };
                let handle = next_handle.fetch_add(1, Ordering::Relaxed);
                let remote = JsValue::from(offer.remote_id.to_string());
                let payload = js_sys::Uint8Array::from(offer.payload.as_slice());
                offers
                    .lock()
                    .expect("offers poisoned")
                    .insert(handle, offer);
                let callbacks = callbacks.lock().expect("pairing callbacks poisoned");
                for callback in callbacks.values() {
                    let _ =
                        callback.call3(&JsValue::NULL, &JsValue::from(handle), &remote, &payload);
                }
            }
        });
    }
}
