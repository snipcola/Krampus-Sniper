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
use serde_json::{Value, from_str};

use reqwest::header;
use reqwest::multipart::Form;

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

#[derive(Debug, Deserialize)]
struct ResponseData {
    status: u16,
    data: Value
}

// Functions
fn timestamp() -> String {
    let now = Local::now();
    return now.format("%H:%M:%S").to_string();
}

async fn get_redeem_data(config: &Config, key: &str) -> Result<ResponseData, Box<dyn std::error::Error>> {
    let form = Form::new().text("key", key.to_string());
    let cookie = format!("_db_ses=Bearer%20{}", config.krampus_auth);
    let client = reqwest::Client::new();
    let response = client
        .post("https://loader.live/dashboard/licenses?/claim")
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .header(header::REFERER, "https://loader.live/dashboard/licenses")
        .header(header::ORIGIN, "https://loader.live")
        .header(header::COOKIE, cookie)
        .multipart(form)
        .send()
        .await?;

    if response.status().is_success() {
        let data: ResponseData = response.json().await?;
        return Ok(data);
    } else {
        return Err("Request Failed".into());
    }
}

async fn redeem_key(config: &Config, key: &str) {
    println!("{} Redeeming Key: {}", format!("[KEY, {}]", timestamp()).blue(), key.bold());
    let data = get_redeem_data(&config, &key).await;
                
    match data {
        Ok(data) => {
            if data.status == 200 {
                println!("{} Redeemed key: {}, {}", format!("[KEY, {}]", timestamp()).green(), key.bold(), data.data.to_string().bold());
            } else {
                println!("{} Failed to redeem key: {}, {}", format!("[KEY, {}]", timestamp()).red(), key.bold(), data.data.to_string().bold());
            }
        },
        Err(error) => {
            println!("{} Failed to redeem key: {}, {}", format!("[KEY, {}]", timestamp()).red(), key.bold(), format!("{:?}", error).bold());
        }
    }
}

fn get_potential_keys(text: &str, key_length: usize) -> Vec<String> {
    let regex = Regex::new(r"[^a-zA-Z0-9]").unwrap();

    return text
        .split_whitespace()
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

    for key in get_potential_keys(&text, config.key_length) {
        let config_clone = config.clone();
        spawn(async move {
            redeem_key(&config_clone, &key).await;
        });
    }
}

async fn handle_message(config: &Config, message: Message, guild_id: i64) {
    if config.server_ids.contains(&guild_id) {
        for key in get_potential_keys(&message.content, config.key_length) {
            let config_clone = config.clone();
            spawn(async move {
                redeem_key(&config_clone, &key).await;
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
            println!("{} Failed to read config: {}", "[ERROR]".red(), format!("{:?}", error).bold());
            return;
        }
    };
        
    let handler = Handler::new(config.clone());
    let mut client = Client::builder(config.discord_token)
        .event_handler(handler)
        .await
        .expect(&format!("{} Failed to create client.", "[ERROR]".red()));

    if let Err(error) = client.start().await {
        println!("{} Failed to start client: {}", "[ERROR]".red(), format!("{:?}", error).bold());
    }
}