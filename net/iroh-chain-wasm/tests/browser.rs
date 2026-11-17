#![cfg(target_arch = "wasm32")]

use std::{cell::RefCell, rc::Rc};

use iroh_chain_wasm::WebServer;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

async fn make_server() -> WebServer {
    WebServer::create(None, vec![])
        .await
        .expect("create web server")
}

async fn settle(rounds: u32) {
    for _ in 0..rounds {
        gloo_timers::future::TimeoutFuture::new(100).await;
    }
}

async fn wait_for_relays() {
    gloo_timers::future::TimeoutFuture::new(15_000).await;
}

fn message_recorder() -> (
    Rc<RefCell<Option<Vec<u8>>>>,
    Closure<dyn FnMut(String, JsValue, js_sys::Uint8Array)>,
) {
    let recorded = Rc::new(RefCell::new(None::<Vec<u8>>));
    let sink = Rc::clone(&recorded);
    let closure = Closure::wrap(Box::new(
        move |_source: String, _destination: JsValue, payload: js_sys::Uint8Array| {
            *sink.borrow_mut() = Some(payload.to_vec());
        },
    ) as Box<dyn FnMut(String, JsValue, js_sys::Uint8Array)>);
    (recorded, closure)
}

#[wasm_bindgen_test]
async fn creates_endpoint_with_hex_id() {
    let server = make_server().await;
    assert_eq!(server.endpoint_id().len(), 64);
    server.close().await;
}

#[wasm_bindgen_test]
async fn peer_allowlist_round_trip() {
    let a = make_server().await;
    let b = make_server().await;
    let b_id = b.endpoint_id();
    a.add_peer(&b_id).expect("add peer");
    assert_eq!(a.peers(), vec![b_id.clone()]);
    a.remove_peer(&b_id).expect("remove peer");
    assert!(a.peers().is_empty());
    a.close().await;
    b.close().await;
}

#[wasm_bindgen_test]
async fn broadcast_reaches_peer() {
    let a = make_server().await;
    let b = make_server().await;
    let b_id = b.endpoint_id();
    a.add_peer(&b_id).expect("add peer");
    b.add_peer(&a.endpoint_id()).expect("add peer");
    wait_for_relays().await;

    let (recorded, closure) = message_recorder();
    let handle = b.subscribe_messages(closure.as_ref().unchecked_ref::<js_sys::Function>().clone());
    closure.forget();

    a.broadcast(b"hello-mesh").await.expect("broadcast");
    settle(50).await;
    b.unsubscribe_messages(handle);

    assert_eq!(recorded.borrow().as_deref(), Some(b"hello-mesh".as_slice()));
    a.close().await;
    b.close().await;
}

#[wasm_bindgen_test]
async fn direct_message_reaches_only_destination() {
    let a = make_server().await;
    let b = make_server().await;
    let c = make_server().await;
    for (server, other) in [(&a, &b), (&a, &c), (&b, &a), (&b, &c), (&c, &a), (&c, &b)] {
        server.add_peer(&other.endpoint_id()).expect("add peer");
    }
    wait_for_relays().await;

    let (recorded_b, closure_b) = message_recorder();
    let handle_b = b.subscribe_messages(
        closure_b
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone(),
    );
    closure_b.forget();
    let (recorded_c, closure_c) = message_recorder();
    let handle_c = c.subscribe_messages(
        closure_c
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .clone(),
    );
    closure_c.forget();

    a.send(&b.endpoint_id(), b"direct-hello")
        .await
        .expect("send");
    settle(50).await;
    b.unsubscribe_messages(handle_b);
    c.unsubscribe_messages(handle_c);

    assert_eq!(
        recorded_b.borrow().as_deref(),
        Some(b"direct-hello".as_slice())
    );
    assert!(recorded_c.borrow().is_none());
    a.close().await;
    b.close().await;
    c.close().await;
}

#[wasm_bindgen_test]
async fn pairing_offer_and_reply() {
    let a = make_server().await;
    let b = make_server().await;
    let b_id = b.endpoint_id();
    a.add_peer(&b_id).expect("add peer");
    b.add_peer(&a.endpoint_id()).expect("add peer");
    wait_for_relays().await;

    let offers = Rc::new(RefCell::new(None::<(u32, String, Vec<u8>)>));
    let sink = Rc::clone(&offers);
    let closure = Closure::wrap(Box::new(
        move |handle: u32, remote: String, payload: js_sys::Uint8Array| {
            *sink.borrow_mut() = Some((handle, remote, payload.to_vec()));
        },
    ) as Box<dyn FnMut(u32, String, js_sys::Uint8Array)>);
    b.subscribe_pairing_offers(closure.as_ref().unchecked_ref::<js_sys::Function>().clone());
    closure.forget();

    b.set_pairing_enabled(true);
    let reply = a.send_pairing_offer(&b_id, b"pair-request").await;
    settle(50).await;

    let (handle, remote, payload) = offers.borrow().clone().expect("pairing offer received");
    assert_eq!(remote, a.endpoint_id());
    assert_eq!(payload, b"pair-request");
    b.reply_to_pairing_offer(handle, b"pair-accepted")
        .await
        .expect("reply");
    assert_eq!(reply.expect("pairing reply"), b"pair-accepted".to_vec());

    a.close().await;
    b.close().await;
}
