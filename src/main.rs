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
use std::collections::HashSet;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("✅ Bot connected as {}", ready.user.name);
        // Send online notification to allowed channel
        if let Some(channel_id) = std::env::var("ALLOWED_CHANNEL_ID")
            .ok()
            .and_then(|id| id.parse::<u64>().ok())
        {
            let _ = serenity::model::id::ChannelId(channel_id)
                .say(&ctx.http, "🤖 Healthy Bot started and is online!")
                .await;
        }
    }
}

#[group]
#[commands(battery, cpu, ram, ip, commands)]
struct General;

#[command]
async fn battery(ctx: &Context, msg: &Message) -> CommandResult {
    fn read_file(path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    let path_prefix = "/host/sys/class/power_supply/qcom-battery";

    let battery = read_file(&format!("{}/capacity", path_prefix)).unwrap_or("unknown".to_string());
    let status = read_file(&format!("{}/status", path_prefix)).unwrap_or("unknown".to_string());

    let voltage = match read_file(&format!("{}/voltage_now", path_prefix)) {
        Some(ref s) if !s.is_empty() => match s.parse::<f64>() {
            Ok(val) => format!("{:.2} V", val / 1_000_000.0),
            Err(_) => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    };

    let current = match read_file(&format!("{}/current_now", path_prefix)) {
        Some(ref s) if !s.is_empty() => match s.parse::<f64>() {
            Ok(val) => format!("{:.2} A", val / 1_000_000.0),
            Err(_) => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    };

    let reply = format!(
        "🔋 **Battery Info**\nPercent: {}%\nStatus:  {}\nVoltage: {}\nCurrent: {}",
        battery, status, voltage, current
    );
    msg.reply(ctx, reply).await?;
    Ok(())
}

#[command]
#[aliases("ip")]
async fn ip(ctx: &Context, msg: &Message) -> CommandResult {
    use std::fs;
    use regex::Regex;

    let sys_prefix = "/host/sys/class/net";
    let mut lines = vec!["==== Network Interfaces ====".to_string()];

    let interfaces = match fs::read_dir(sys_prefix) {
        Ok(it) => it,
        Err(_) => return {
            msg.reply(ctx, "Could not read network interfaces!").await?;
            Ok(())
        },
    };

    for entry in interfaces {
        if let Ok(entry) = entry {
            let iface = entry.file_name().into_string().unwrap_or_default();
            if iface == "lo" { continue; }

            let mac = fs::read_to_string(format!("{}/{}/address", sys_prefix, iface)).unwrap_or("unknown".into()).trim().to_string();
            let state = fs::read_to_string(format!("{}/{}/operstate", sys_prefix, iface)).unwrap_or("unknown".into()).trim().to_string();

            // IPv4
            let mut ipv4_str = String::new();
            if let Ok(ipv4_out) = fs::read_to_string("/host/proc/net/fib_trie") {
                let ipv4_re = Regex::new(&format!(r"32 host (\d+\.\d+\.\d+\.\d+).*[\n\r]+.*{}$", iface)).unwrap();
                for cap in ipv4_re.captures_iter(&ipv4_out) {
                    ipv4_str = cap[1].to_string();
                }
            }

            // IPv6
            let mut ipv6_str = String::new();
            if let Ok(ipv6_out) = fs::read_to_string("/host/proc/net/if_inet6") {
                for line in ipv6_out.lines() {
                    let cols: Vec<_> = line.split_whitespace().collect();
                    if cols.len() == 6 && cols[5] == iface {
                        let v6raw = &cols[0];
                        let ipv6 = (0..8).map(|i| &v6raw[i*4..i*4+4]).collect::<Vec<_>>().join(":");
                        ipv6_str = ipv6;
                    }
                }
            }

            lines.push(format!("\n🔹 Interface: {iface}"));
            lines.push(format!("   ├─ State     : {state}"));
            lines.push(format!("   ├─ MAC       : {mac}"));
            if !ipv4_str.is_empty() {
                lines.push(format!("   ├─ IPv4      : {ipv4_str}"));
            }
            if !ipv6_str.is_empty() {
                lines.push(format!("   └─ IPv6      : {ipv6_str}"));
            }
        }
    }

    let reply = format!("```\n{}\n```", lines.join("\n"));
    msg.reply(ctx, &reply).await?;
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
    let help_message: &'static str = "🤖 **Healthy Bot Help**
Monitor your system resources via Discord! Use the following commands with either `/` or `!` prefix.

**Available Commands**
- **/battery** or **!battery**
  Shows current battery percentage and status (charging/discharging). Example: `!battery`
- **/cpu** or **!cpu**
  Displays the current CPU usage as a percentage. Example: `/cpu`
- **/ram** or **!ram**
  Shows RAM usage in MB (used/total). Example: `/ram`
- **/ip** or **!ip**
  Lists all host network interfaces, MACs, states, and IPs. Example: `!ip`
- **/commands**, **!commands**, **/help**, or **!help**
  Shows this help message.

**Usage Details**
- All commands work in the allowed Discord channel only (channel ID is configured in the bot).
- You can use both `/` and `!` prefixes interchangeably.
- Responses include relevant emojis for better visibility.
- Resource stats are updated in real-time as requested.

**Automatic Notifications**
- The bot monitors battery level in the background and will send an alert if it drops below 20%.

**Need More Help?**
- For bug reports, feature requests, or assistance, contact the bot maintainer.
- Source code: (add your repo link if public)

Thank you for using Healthy Bot! 🚀";
    msg.reply(ctx, help_message).await?;
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
.configure(|c| c.prefix("!").with_whitespace(true).prefixes(["/", "!"]))
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
        // Try to notify allowed channel if shutdown/crash
        let ctx = client.cache_and_http.clone();
        let _ = serenity::model::id::ChannelId(allowed_channel_id)
            .say(&ctx.http, "⚠️ Healthy Bot is shutting down or crashed!")
            .await;
    }
}
