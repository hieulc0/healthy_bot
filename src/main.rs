use serenity::{
    async_trait,
    framework::standard::{macros::command, macros::group, CommandResult, StandardFramework},
    model::{channel::Message, gateway::Ready},
    prelude::*,
    CacheAndHttp,
};
use sysinfo::System;
use dotenvy::dotenv;
use std::{fs, env, time::Duration};
use tokio::time;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("✅ Bot connected as {}", ready.user.name);
    }
}

#[group]
#[commands(battery, cpu, ram, commands)]
struct General;

#[command]
async fn battery(ctx: &Context, msg: &Message) -> CommandResult {
    let battery = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/capacity")
        .unwrap_or("unknown".to_string());
    let status = fs::read_to_string("/host/sys/class/power_supply/qcom-battery/status")
        .unwrap_or("unknown".to_string());
    let reply = format!("🔋 Battery: {}% ({})", battery.trim(), status.trim());
    msg.reply(ctx, reply).await?;
    Ok(())
}

#[command]
async fn cpu(ctx: &Context, msg: &Message) -> CommandResult {
    let mut sys = System::new();
    sys.refresh_cpu();
    let cpu = sys.global_cpu_info().cpu_usage();
    let reply = format!("🧠 CPU Usage: {:.2}%", cpu);
    msg.reply(ctx, reply).await?;
    Ok(())
}

#[command]
async fn ram(ctx: &Context, msg: &Message) -> CommandResult {
    let mut sys = System::new();
    sys.refresh_memory();
    let used = sys.used_memory() / 1024;
    let total = sys.total_memory() / 1024;
    let reply = format!("💾 RAM Usage: {} / {} MB", used, total);
    msg.reply(ctx, reply).await?;
    Ok(())
}

#[command]
#[aliases("help", "commands")]
async fn commands(ctx: &Context, msg: &Message) -> CommandResult {
    let text = "**Available commands:**\n\
/battery or !battery — Show battery\n\
/cpu or !cpu — Show CPU usage\n\
/ram or !ram — Show RAM usage\n\
/commands or !commands or /help or !help — Show this message";
    msg.reply(ctx, text).await?;
    Ok(())
}

use serenity::framework::standard::{macros::help, Args, CommandGroup, HelpOptions};
use serenity::model::prelude::UserId;

#[help]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = serenity::framework::standard::help_commands::with_embeds(
        context,
        msg,
        args,
        help_options,
        groups,
        owners,
    )
    .await;
    Ok(())
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

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!").whitespace(true).prefixes(["/", "!"]))
        .help(&MY_HELP)
        .group(&GENERAL_GROUP);

    let mut client = serenity::Client::builder(
        &token,
        serenity::model::gateway::GatewayIntents::GUILD_MESSAGES | serenity::model::gateway::GatewayIntents::MESSAGE_CONTENT,
    )
    .event_handler(Handler)
    .framework(framework)
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
