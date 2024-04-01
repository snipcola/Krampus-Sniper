// Copyright (c) 2024 Snipcola
// SPDX-License-Identifier: MIT

#![allow(non_snake_case, non_upper_case_globals)]

// Program
use colored::{Colorize, control};
use chrono::Local;

use tokio::spawn;
use regex::Regex;

use image::load_from_memory;
use rusty_tesseract::{image_to_string, Args, Image};

use serenity::prelude::*;
use serenity::model::channel::{Message, Attachment};
use serenity::async_trait;

use std::fs::File;
use std::io::Read;
use std::error::Error;

use serde::{Deserialize, Serialize};
use serde_json::{json, from_str, Value, Map};

use reqwest::header;

// Structs
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    discord_token: String,
    krampus_auth: String,
    server_ids: Vec<i64>,
    key_length: usize
}

impl Config {
    fn from_file(path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: Config = from_str(&contents)?;
        return Ok(config);
    }
}

// Functions
fn timestamp() -> String {
    let now = Local::now();
    return now.format("%H:%M:%S").to_string();
}

async fn get_redeem_data(config: &Config, keys: Vec<String>) -> Result<Value, Box<dyn std::error::Error>> {
    let mut json_map = Map::new();

    for (index, key) in keys.iter().enumerate() {
        json_map.insert(index.to_string(), json!({ "json": key }));
    }

    let client = reqwest::Client::new();
    let response = client
        .post(format!("https://api.acedia.gg/trpc/license.claim?batch={}", keys.len()))
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .header(header::AUTHORIZATION, format!("Bearer {}", config.krampus_auth))
        .header(header::REFERER, "https://acedia.gg/dashboard/licenses")
        .header(header::ORIGIN, "https://acedia.gg")
        .json(&json_map)
        .send()
        .await?;

    let data = response.json().await?;
    return Ok(data);
}

async fn redeem_keys(config: &Config, keys: Vec<String>) {
    // TODO: Wait for acedia.gg API to support batched requests, and then send all keys at once.

    if keys.len() < 1 {
        return;
    }

    for key in keys.iter() {
        println!("{} Redeeming Key: {}", format!("[{}, KEY]", timestamp()).blue(), key.bold());
    }

    let data = get_redeem_data(&config, keys.clone()).await;

    match data {
        Ok(json_obj) => {
            for (index, key) in keys.iter().enumerate() {
                let data: Option<(bool, String)> = json_obj.get(index).and_then(|data| {
                    data.get("error").and_then(|error| {
                        error.get("json").and_then(|json| {
                            json.get("message").map(|message| (false, message.to_string()))
                        })
                    }).or_else(|| {
                        data.get("result").and_then(|result| {
                            result.get("data").and_then(|data| {
                                data.get("json").and_then(|json| {
                                    json.get("status").map(|status| (true, status.to_string()))
                                })
                            })
                        })
                    })
                });

                match data {
                    Some((success, message)) => {
                        if success {
                            println!("{} Redeemed Key: {}, {}", format!("[{}, KEY]", timestamp()).green(), key.bold(), message.bold());
                        } else {
                            println!("{} Failed to Redeem Key: {}, {}", format!("[{}, KEY]", timestamp()).red(), key.bold(), message.bold());
                        }
                    },
                    None => {
                        println!("{} Failed to Redeem Key: {}, {}", format!("[{}, KEY]", timestamp()).red(), key.bold(), "Invalid Response".bold());
                    }
                }
            }
        },
        Err(error) => {
            let time = timestamp();

            for key in keys.iter() {
                println!("{} Failed to redeem key: {}, {}", format!("[{}, KEY]", time).red(), key.bold(), format!("{:?}", error).bold());
            }
        }
    }
}

fn get_potential_keys(text: &str, key_length: usize) -> Vec<String> {
    let regex = Regex::new(r"[^a-zA-Z0-9]").unwrap();

    return text
        .split_whitespace()
        .filter(|word| !word.starts_with("https:") && !word.starts_with("http:"))
        .map(|word| regex.replace_all(word, "").to_string())
        .filter(|word| word.len() == key_length)
        .collect();
}

async fn handle_attachment(config: &Config, attachment: Attachment) {
    let response = match reqwest::get(attachment.url).await {
        Ok(response) => response,
        Err(_) => return
    };

    let data = match response.bytes().await {
        Ok(data) => data,
        Err(_) => return
    };

    let image = match load_from_memory(&data) {
        Ok(image) => match Image::from_dynamic_image(&image) {
            Ok(image) => image,
            Err(_) => return
        },
        Err(_) => return
    };

    let text = match image_to_string(&image, &Args::default()) {
        Ok(text) => text,
        Err(_) => return
    };

    let keys = get_potential_keys(&text, config.key_length);
    
    for key in keys {
        let config_clone = config.clone();
        
        spawn(async move {
            redeem_keys(&config_clone, vec![key]).await;
        });
    }
}

async fn handle_message(config: &Config, message: Message, guild_id: i64) {
    if config.server_ids.contains(&guild_id) {
        let keys = get_potential_keys(&message.content, config.key_length);
        
        for key in keys {
            let config_clone = config.clone();

            spawn(async move {
                redeem_keys(&config_clone, vec![key]).await;
            });
        }

        for attachment in message.attachments {
            let config_clone = config.clone();
            spawn(async move {
                handle_attachment(&config_clone, attachment).await;
            });
        }
    }
}

// Handler
struct Handler {
    config: Config
}

impl Handler {
    fn new(config: Config) -> Self {
        return Handler { config };
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _context: Context, message: Message) {
        let guild_id: i64 = match message.guild_id {
            Some(guild_id) => guild_id.into(),
            None => message.channel_id.into()
        };

        handle_message(&self.config, message, guild_id).await;
    }
}

// Main
#[tokio::main]
async fn main() {
    control::set_virtual_terminal(true).ok();

    let config = match Config::from_file("config.json") {
        Ok(config) => config,
        Err(error) => {
            println!("{} Failed to Read Config: {}", "[ERROR]".red(), format!("{:?}", error).bold());
            return;
        }
    };
        
    let handler = Handler::new(config.clone());
    let mut client = Client::builder(config.discord_token)
        .event_handler(handler)
        .await
        .expect(&format!("{} Failed to Create Client.", "[ERROR]".red()));

    if let Err(error) = client.start().await {
        println!("{} Failed to Start Client: {}", "[ERROR]".red(), format!("{:?}", error).bold());
    }
}