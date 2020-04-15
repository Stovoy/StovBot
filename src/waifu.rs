use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Copy, Clone)]
#[serde(untagged)]
enum Seed {
    Value(i32),
    Values([f32; 3]),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateRequest {
    step: i32,
    current_girl: Option<[Seed; 18]>,
    size: Option<i32>,
}

#[derive(Deserialize)]
struct GenerateBigResponse {
    girl: String,
}

pub fn generate_waifu_image() -> String {
    let client = reqwest::blocking::Client::new();
    let mut headers = reqwest::header::HeaderMap::new();

    headers.insert("content-type", "application/json".parse().unwrap());

    let mut rng = rand::thread_rng();

    let mut seeds = [Seed::Value(0); 18];
    for i in 0..17 {
        seeds[i] = Seed::Value(rng.gen_range(0, 1000000));
    }
    let mut values = [0.0; 3];
    for i in 0..3 {
        values[i] = rng.gen_range(0.0, 1000.0);
    }
    seeds[17] = Seed::Values(values);

    match client
        .post("https://api.waifulabs.com/generate_big")
        .headers(headers)
        .json(&GenerateRequest {
            step: 4,
            current_girl: Some(seeds),
            size: Some(512),
        })
        .send()
    {
        Err(e) => e.to_string(),
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<GenerateBigResponse>() {
                    Err(e) => e.to_string(),
                    Ok(response) => response.girl,
                }
            } else {
                format!("server error: {}", response.status())
            }
        }
    }
}
