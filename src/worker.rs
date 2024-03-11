use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::screenshots::take_screenshots;
use crate::types::State;

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingRequest {
    content: String,
    image: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponse {
    embedding: Vec<f64>,
}

pub fn record_state_loop(state_mutex: Arc<Mutex<State>>) -> Result<()> {
    while true {
        record_state(&state_mutex);
    }
    Ok(())
}

fn record_state(state_mutex: &Arc<Mutex<State>>) -> Result<()> {
    let screenshots = take_screenshots()?;
    (*state_mutex).lock().unwrap().window_count = screenshots.len();

    let client = reqwest::blocking::Client::new();
    for screenshot in screenshots {
        let jpeg = screenshot.jpeg;
        let request = EmbeddingRequest {
            content: "<image>\nUSER:\n\
                Provide a full description of this screenshot. Be as accurate and detailed as \
                possible.\n\
                ASSISTANT:\n"
                .into(),
            image: STANDARD.encode(jpeg),
        };
        let _resp = client
            .post("http://127.0.0.1:8080/embedding")
            .json(&request)
            .send()?
            .json::<EmbeddingResponse>()?;
        println!("resp");
    }

    Ok(())
}
