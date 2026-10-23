//! WebAudio backend: an `AudioWorkletNode` whose processor runs the DSP from a
//! standalone wasm module (built from `crates/worklet` by `build.rs`).
//!
//! Browsers only let an `AudioContext` start after a user gesture, so the
//! context is created on the first [`WebBackend::user_gesture`]. Until the node
//! is up, parameter changes only update the [`ParamCache`]; the whole cache is
//! flushed to the worklet once it is ready.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use js_sys::{Array, Object, Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioContextOptions, AudioContextState, AudioWorkletNode,
    AudioWorkletNodeOptions, Blob, BlobPropertyBag, Url,
};

use super::{AudioStatus, Command, ParamCache};

const WORKLET_JS: &str = include_str!("worklet.js");
const DSP_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/stabkussion_worklet.wasm"));
const PROCESSOR_NAME: &str = "stabkussion";

#[derive(Default)]
struct Inner {
    ctx: Option<AudioContext>,
    node: Option<AudioWorkletNode>,
    cache: ParamCache,
}

pub struct WebBackend {
    inner: Rc<RefCell<Inner>>,
    status: Signal<AudioStatus>,
}

impl WebBackend {
    pub fn new(mut status: Signal<AudioStatus>, initial: ParamCache) -> Self {
        status.set(AudioStatus::NeedsGesture);
        Self {
            inner: Rc::new(RefCell::new(Inner {
                cache: initial,
                ..Default::default()
            })),
            status,
        }
    }

    pub fn send(&self, cmd: Command) {
        let mut inner = self.inner.borrow_mut();
        inner.cache.apply(cmd);
        if let Some(node) = &inner.node {
            post(node, cmd);
        }
    }

    pub fn user_gesture(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(ctx) = &inner.ctx {
            if ctx.state() == AudioContextState::Suspended {
                let _ = ctx.resume();
            }
            return;
        }
        let opts = AudioContextOptions::new();
        opts.set_latency_hint(&JsValue::from_str("interactive"));
        let ctx = match AudioContext::new_with_context_options(&opts) {
            Ok(ctx) => ctx,
            Err(e) => {
                let mut status = self.status;
                status.set(AudioStatus::Failed(js_error(&e)));
                return;
            }
        };
        inner.ctx = Some(ctx.clone());
        drop(inner);

        let mut status = self.status;
        status.set(AudioStatus::Starting);
        let shared = self.inner.clone();
        spawn(async move {
            match start(&ctx).await {
                Ok(node) => {
                    let mut inner = shared.borrow_mut();
                    for cmd in inner.cache.commands() {
                        post(&node, cmd);
                    }
                    post_raw(&node, &[3.0]);
                    inner.node = Some(node);
                    status.set(AudioStatus::Running {
                        sample_rate: ctx.sample_rate() as u32,
                        detail: "WebAudio · AudioWorklet".into(),
                    });
                }
                Err(e) => status.set(AudioStatus::Failed(js_error(&e))),
            }
        });
    }
}

async fn start(ctx: &AudioContext) -> Result<AudioWorkletNode, JsValue> {
    // Load the processor script from a Blob URL so no extra asset has to be served.
    let parts = Array::of1(&JsValue::from_str(WORKLET_JS));
    let props = BlobPropertyBag::new();
    props.set_type("text/javascript");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &props)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let added = JsFuture::from(ctx.audio_worklet()?.add_module(&url)?).await;
    let _ = Url::revoke_object_url(&url);
    added?;

    let bytes = Uint8Array::from(DSP_WASM);
    let module = JsFuture::from(WebAssembly::compile(&bytes.buffer().into())).await?;

    let processor_options = Object::new();
    Reflect::set(&processor_options, &"module".into(), &module)?;
    let opts = AudioWorkletNodeOptions::new();
    opts.set_number_of_inputs(0);
    opts.set_number_of_outputs(1);
    opts.set_output_channel_count(&Array::of1(&JsValue::from_f64(2.0)));
    opts.set_processor_options(Some(&processor_options));
    let node = AudioWorkletNode::new_with_options(ctx, PROCESSOR_NAME, &opts)?;
    node.connect_with_audio_node(&ctx.destination())?;
    if ctx.state() == AudioContextState::Suspended {
        let _ = ctx.resume();
    }
    Ok(node)
}

fn post(node: &AudioWorkletNode, cmd: Command) {
    match cmd {
        Command::Param {
            voice,
            param,
            value,
        } => post_raw(node, &[0.0, voice as f64, param.id() as f64, value as f64]),
        Command::Master(v) => post_raw(node, &[1.0, v as f64]),
        Command::Trigger {
            voice,
            velocity,
            snap,
        } => post_raw(
            node,
            &[
                2.0,
                voice as f64,
                velocity as f64,
                if snap { 1.0 } else { 0.0 },
            ],
        ),
    }
}

fn post_raw(node: &AudioWorkletNode, values: &[f64]) {
    let msg: Array = values.iter().map(|&v| JsValue::from_f64(v)).collect();
    if let Ok(port) = node.port() {
        let _ = port.post_message(&msg);
    }
}

fn js_error(e: &JsValue) -> String {
    if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        return String::from(err.message());
    }
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}
