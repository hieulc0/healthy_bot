use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
    CacheAndHttp,
};
use sysinfo::System;
use dotenvy::dotenv;
use std::{fs, env, time::Duration};
use tokio::time;

struct Handler {
    allowed_channel_id: u64,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Only accept from allowed channel
        if msg.channel_id.0 != self.allowed_channel_id {
            return;
        }
        let content = msg.content.trim();

        if content == "/battery" || content == "!battery" {
            let battery = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/capacity")
                .unwrap_or("unknown".to_string());
            let status = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/status")
                .unwrap_or("unknown".to_string());
            let reply = format!("🔋 Battery: {}% ({})", battery.trim(), status.trim());
            let _ = msg.channel_id.say(&ctx.http, reply).await;
        } else if content == "/cpu" || content == "!cpu" {
            let mut sys = System::new();
            sys.refresh_cpu();
            let cpu = sys.global_cpu_info().cpu_usage();
            let reply = format!("🧠 CPU Usage: {:.2}%", cpu);
            let _ = msg.channel_id.say(&ctx.http, reply).await;
        } else if content == "/ram" || content == "!ram" {
            let mut sys = System::new();
            sys.refresh_memory();
            let used = sys.used_memory() / 1024;
            let total = sys.total_memory() / 1024;
            let reply = format!("💾 RAM Usage: {} / {} MB", used, total);
            let _ = msg.channel_id.say(&ctx.http, reply).await;
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("✅ Bot connected as {}", ready.user.name);
    }
}

async fn battery_notify_task(ctx: std::sync::Arc<CacheAndHttp>, channel_id: u64) {
    loop {
        let battery = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/capacity")
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        let status = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/status")
            .unwrap_or("unknown".to_string());
        if let Some(b) = battery {
            if b < 20 {
                let content = format!(
                    "⚠️ Battery low: {}% ({})",
                    b, status.trim()
                );
                let _ = serenity::model::id::ChannelId(channel_id)
                    .say(&ctx.http, content)
                    .await;
            }
        }
        // Check every 5 minutes
        time::sleep(Duration::from_secs(300)).await;
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Missing DISCORD_TOKEN");
    let allowed_channel_id: u64 = env::var("ALLOWED_CHANNEL_ID")
        .expect("Missing ALLOWED_CHANNEL_ID")
        .parse()
        .expect("Invalid channel ID");

    let handler = Handler { allowed_channel_id };
    let mut client = serenity::Client::builder(
        &token,
        serenity::model::gateway::GatewayIntents::GUILD_MESSAGES | serenity::model::gateway::GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(handler)
    .await
    .expect("Client creation failed");

    // Start battery notifier
    let ctx = client.cache_and_http.clone();
    let background_channel = allowed_channel_id;
    tokio::spawn(async move {
        battery_notify_task(
            ctx,
            background_channel,
        ).await;
    });

    if let Err(err) = client.start().await {
        eprintln!("Client error: {:?}", err);
    }
}
