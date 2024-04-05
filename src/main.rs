// Copyright (c) 2024 Snipcola
// SPDX-License-Identifier: MIT

#![allow(non_snake_case, non_upper_case_globals)]

// Program
#[cfg(target_os = "windows")]
use colored::control;

use colored::Colorize;
use chrono::Local;

use tokio::{spawn, time::sleep};
use regex::Regex;

use image::load_from_memory;
use rusty_tesseract::{image_to_string, Args, Image};

use serenity::prelude::*;
use serenity::model::channel::{Message, Attachment};
use serenity::async_trait;

use std::fs::File;
use std::io::{stdin, Read};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::error::Error;
use std::process::exit;

use serde::{Deserialize, Serialize};
use serde_json::{json, from_str, Value, Map};

use reqwest::header;
use lazy_static::lazy_static;

// Globals
lazy_static! {
    static ref JWT_TOKEN: Arc<Mutex<String>> = Arc::new(Mutex::new("".to_string()));
}

// Structs
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Credentials {
    login: String,
    password: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    discord_token: String,
    krampus_credentials: Credentials,
    server_ids: Vec<i64>,
    key_lengths: Vec<usize>,
    snipe_images: bool,
    strict: bool
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
fn pause() {
    let _ = stdin().read(&mut [0; 1]);
}

fn timestamp() -> String {
    let now = Local::now();
    return now.format("%H:%M:%S").to_string();
}

async fn get_login_data(config: &Config) -> Result<(Value, String), Box<dyn std::error::Error>> {
    let mut json_map = Map::new();
    json_map.insert("0".to_string(), json!({
        "json": {
            "emailOrUsername": config.krampus_credentials.login,
            "password": config.krampus_credentials.password
        }
    }));
    
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.acedia.gg/trpc/auth.logIn?batch=1")
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .header(header::REFERER, "https://acedia.gg")
        .header(header::ORIGIN, "https://acedia.gg")
        .json(&json_map)
        .send()
        .await?;

    let headers = response.headers().clone();
    let cookies = match headers.get(header::SET_COOKIE) {
        Some(data) => match data.to_str() {
            Ok(cookie_string) => cookie_string.split(";")
                .map(|cookie| {
                    let parts: Vec<&str> = cookie.split("=").collect();
        
                    if parts.len() == 2 {
                        return (parts[0].trim(), parts[1].trim());
                    } else {
                        return ("", "");
                    }
                })
                .filter(|(key, _)| !key.is_empty())
                .collect(),
            Err(_) => vec![]
        },
        None => vec![]
    };
    
    let data: Value = response.json().await?;
    let cookie = cookies.iter()
        .find(|(key, _)| key == &"_session")
        .map(|(_, value)| value.to_string())
        .unwrap_or_else(|| "".to_string());

    return Ok((data.get(0).unwrap().to_owned(), cookie));
}

async fn login(config: &Config) -> Option<String> {
    let (data, cookie) = match get_login_data(config).await {
        Ok(response) => response,
        Err(error) => return Some(format!("{:?}", error))
    };

    let data: Option<(bool, String)> = data.get("error").and_then(|error| {
        error.get("json").and_then(|json| {
            json.get("message").map(|message| (false, message.to_string()))
        })
    }).or_else(|| {
        data.get("result").and_then(|_| Some((true, "".to_string())))
    });

    match data {
        Some((success, message)) => {
            if success {
                *JWT_TOKEN.lock().unwrap() = cookie;
                return None;
            } else {
                return Some(message);
            }
        },
        None => return Some("Invalid Response".to_string())
    }
}

async fn get_redeem_data(keys: Vec<String>) -> Result<Value, Box<dyn std::error::Error>> {
    let jwt_token = JWT_TOKEN.lock().unwrap().clone();
    let mut json_map = Map::new();

    for (index, key) in keys.iter().enumerate() {
        json_map.insert(index.to_string(), json!({ "json": key }));
    }

    let client = reqwest::Client::new();
    let response = client
        .post(format!("https://api.acedia.gg/trpc/license.claim?batch={}", keys.len()))
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .header(header::AUTHORIZATION, format!("Bearer {}", jwt_token))
        .header(header::REFERER, "https://acedia.gg/dashboard/licenses")
        .header(header::ORIGIN, "https://acedia.gg")
        .json(&json_map)
        .send()
        .await?;

    let data = response.json().await?;
    return Ok(data);
}

async fn redeem_keys(keys: Vec<String>) {
    if keys.len() < 1 {
        return;
    }

    for key in keys.iter() {
        println!("{} Redeeming Key: {}", format!("[{}, KEY]", timestamp()).cyan(), key.bold());
    }

    let data = get_redeem_data(keys.clone()).await;

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

fn get_potential_keys(text: &str, key_lengths: &Vec<usize>, strict: bool) -> Vec<String> {
    let regex = Regex::new(r"[^a-zA-Z0-9]").unwrap();

    return text
        .split_whitespace()
        .map(|word| regex.replace_all(word, "").to_string())
        .filter(|word| {
            let correct_length = key_lengths.contains(&word.len());

            if strict {
                let has_alphabet = word.chars().any(char::is_alphabetic);
                let has_numbers = word.chars().any(char::is_numeric);
                return correct_length && has_alphabet && has_numbers;
            } else {
                return correct_length;
            }
        })
        .collect();
}

async fn handle_attachment(attachment: Attachment, key_lengths: &Vec<usize>, strict: bool) {
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

    let keys = get_potential_keys(&text, key_lengths, strict);
    
    for key in keys {        
        spawn(async move {
            redeem_keys(vec![key]).await;
        });
    }
}

async fn handle_message(config: &Config, message: Message, guild_id: i64) {
    if config.server_ids.contains(&guild_id) {
        let key_lengths = &config.key_lengths;
        let keys = get_potential_keys(&message.content, key_lengths, config.strict);
        
        for key in keys {
            spawn(async move {
                redeem_keys(vec![key]).await;
            });
        }

        if config.snipe_images {
            for attachment in message.attachments {
                let cloned_config = config.clone();

                spawn(async move {
                    handle_attachment(attachment, &cloned_config.key_lengths, cloned_config.strict).await;
                });
            }
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
    #[cfg(target_os = "windows")]
    control::set_virtual_terminal(true).ok();

    let config = match Config::from_file("config.json") {
        Ok(config) => config,
        Err(error) => {
            println!("{} Failed to Read Config: {}", "[ERROR]".red(), format!("{:?}", error).bold());
            return pause();
        }
    };


    let config_clone = config.clone();

    spawn(async move {
        async fn attempt_login(config: &Config) {
            let login_response = login(&config).await;
            
            if login_response.is_some() {
                println!("{} Failed to Login: {}", "[ERROR]".red(), login_response.unwrap().bold());
                exit(1);
            }
        }

        loop {
            attempt_login(&config_clone).await;
            sleep(Duration::from_secs(300)).await;
        }
    });
        
    let handler = Handler::new(config.clone());
    let mut client = Client::builder(config.discord_token)
        .event_handler(handler)
        .await
        .expect(&format!("{} Failed to Create Client.", "[ERROR]".red()));

    println!("{} Starting Client", format!("[{}, SNIPER]", timestamp()).green());

    if let Err(error) = client.start().await {
        println!("{} Failed to Start Client: {}", "[ERROR]".red(), format!("{:?}", error).bold());
    }
}