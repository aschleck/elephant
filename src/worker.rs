use anyhow::Result;
use arrow_array::types::Float32Type;
use arrow_array::{FixedSizeListArray, RecordBatch, RecordBatchIterator, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::future;
use futures::TryStreamExt;
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::screenshots::get_windows;
use crate::types::{State, Window};

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingRequest {
    instances: Vec<EmbeddingRequestInstance>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingRequestInstance {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<EmbeddingRequestInstanceImage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingRequestInstanceImage {
    bytesBase64Encoded: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponse {
    predictions: Vec<EmbeddingResponsePrediction>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EmbeddingResponsePrediction {
    #[serde(skip_serializing_if = "Option::is_none")]
    imageEmbedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    textEmbedding: Option<Vec<f32>>,
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
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), 1408),
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

    let google_key = String::from_utf8(
        Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .expect("Unable to get Google token")
            .stdout,
    )?;
    let client = reqwest::Client::new();
    let response = client
        .post(
            "https://us-west1-aiplatform.googleapis.com/v1/projects/1012868746574/\
                locations/us-west1/publishers/google/models/\
                multimodalembedding@001:predict",
        )
        .header(AUTHORIZATION, format!("Bearer {}", google_key.trim()))
        .json(&EmbeddingRequest {
            instances: vec![EmbeddingRequestInstance {
                text: Some("A screenshot about mass production of coffee".into()),
                image: None,
            }],
        })
        .send()
        .await?
        .json::<EmbeddingResponse>()
        .await?;
    let results = table
        .search(response.predictions[0].textEmbedding.as_ref().unwrap())
        .select(&["metrohash"])
        .limit(10)
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

    let google_key = String::from_utf8(
        Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .expect("Unable to get Google token")
            .stdout,
    )?;

    let client = reqwest::Client::new();
    let mut responses = Vec::new();
    for window in &changed {
        let request = EmbeddingRequest {
            instances: vec![EmbeddingRequestInstance {
                text: Some(
                    "Provide a full description of this screenshot. Be as accurate and detailed \
                    as possible."
                        .into(),
                ),
                image: Some(EmbeddingRequestInstanceImage {
                    bytesBase64Encoded: STANDARD.encode(&window.jpeg),
                }),
            }],
        };
        responses.push(
            client
                .post(
                    "https://us-west1-aiplatform.googleapis.com/v1/projects/1012868746574/\
                    locations/us-west1/publishers/google/models/\
                    multimodalembedding@001:predict",
                )
                .header(AUTHORIZATION, format!("Bearer {}", google_key.trim()))
                .json(&request)
                .send(),
        );
    }

    let mut embeddings = Vec::new();
    for response in future::join_all(responses).await {
        embeddings.push(
            response?.json::<EmbeddingResponse>().await?.predictions[0]
                .imageEmbedding
                .clone()
                .unwrap(),
        );
    }

    let new_batches = RecordBatchIterator::new(
        vec![RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(UInt64Array::from_iter_values(
                    // TODO
                    changed.iter().map(|w| w.jpeg_metrohash),
                )),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        embeddings
                            .iter()
                            .map(|e| Some(e.iter().map(|i| Some(i.clone())))),
                        1408,
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
