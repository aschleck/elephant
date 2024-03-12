use anyhow::Result;
use arrow_array::types::Float32Type;
use arrow_array::{FixedSizeListArray, RecordBatch, RecordBatchIterator, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::screenshots::get_windows;
use crate::types::{State, Window};

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingRequest {
    content: String,
    image_data: Vec<Image>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Image {
    id: u32,
    data: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TextOnlyEmbeddingRequest {
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

const LOOP_DURATION: Duration = Duration::from_secs(10);

const VECTOR_TABLE: &str = "screenshots";

#[tokio::main]
pub async fn record_state_loop(state_mutex: Arc<Mutex<State>>) -> Result<()> {
    let db = lancedb::connect("data-ldb").execute().await?;

    let vector_schema = Arc::new(Schema::new(vec![
        Field::new("metrohash", DataType::UInt64, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 4096),
            true,
        ),
    ]));
    let table = db
        .create_empty_table(VECTOR_TABLE, vector_schema.clone())
        .mode(lancedb::connection::CreateTableMode::ExistOk(Box::new(
            |t| t,
        )))
        .execute()
        .await?;

    let client = reqwest::Client::new();
    let response = client
        .post("http://127.0.0.1:8080/embedding")
        .json(&TextOnlyEmbeddingRequest {
            content: "USER:\n\
                Provide a full description of the following. Be as accurate and detailed as \
                possible.\n\
                coffee\nASSISTANT:\n"
                .into(),
        })
        .send()
        .await?
        .json::<EmbeddingResponse>()
        .await?;
    let results = table
        .search(&response.embedding)
        .select(&["metrohash"])
        .limit(4)
        .execute_stream()
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    println!("{:?}", results);

    // TODO(april): Need better termination handling
    loop {
        let start = Instant::now();
        record_state(&table, &vector_schema, &state_mutex).await?;
        let end = Instant::now();
        thread::sleep(start + LOOP_DURATION - end);
    }
}

async fn record_state(
    table: &lancedb::Table,
    schema: &Arc<Schema>,
    state_mutex: &Arc<Mutex<State>>,
) -> Result<()> {
    let (changed, unchanged) = get_and_compare_windows(&state_mutex)?;

    let client = reqwest::Client::new();
    let mut embeddings = Vec::new();
    for window in &changed {
        let request = EmbeddingRequest {
            content: "[img-0]\nUSER:\n\
                Provide a full description of this screenshot. Be as accurate and detailed as \
                possible.\n\
                ASSISTANT:\n"
                .into(),
            image_data: vec![Image {
                id: 0,
                data: STANDARD.encode(&window.jpeg),
            }],
        };
        let response = client
            .post("http://127.0.0.1:8080/embedding")
            .json(&request)
            .send()
            .await?
            .json::<EmbeddingResponse>()
            .await?;
        embeddings.push(response.embedding);
        println!("{}", window.title);
        // TODO
        break;
    }

    let new_batches = RecordBatchIterator::new(
        vec![RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(UInt64Array::from_iter_values(
                    // TODO
                    changed[..1].iter().map(|w| w.jpeg_metrohash),
                )),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        embeddings[..1]
                            .iter()
                            .map(|e| Some(e.iter().map(|i| Some(i.clone())))),
                        4096,
                    ),
                ),
            ],
        )
        .unwrap()]
        .into_iter()
        .map(Ok),
        schema.clone(),
    );
    table.add(Box::new(new_batches)).execute().await?;

    let mut mapped = HashMap::new();
    for window in changed {
        std::fs::write(format!("out/{}.jpg", window.jpeg_metrohash), &window.jpeg)?;
        mapped.insert(window.id, window);
    }
    for window in unchanged {
        mapped.insert(window.id, window);
    }
    let mut state = (*state_mutex).lock().unwrap();
    state.windows = mapped;

    Ok(())
}

fn get_and_compare_windows(state_mutex: &Arc<Mutex<State>>) -> Result<(Vec<Window>, Vec<Window>)> {
    let windows = get_windows()?;

    let state = (*state_mutex).lock().unwrap();
    let mut changed: Vec<Window> = Vec::new();
    let mut unchanged: Vec<Window> = Vec::new();
    for window in windows {
        let last_window = state.windows.get(&window.id);
        if let Some(last) = last_window {
            if window.jpeg_metrohash == last.jpeg_metrohash {
                unchanged.push(window);
                continue;
            }
        }
        changed.push(window);
    }

    Ok((changed, unchanged))
}
