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
#[commands(battery, cpu, ram, ip, commands, debug)]
struct General;

#[command]
async fn battery(ctx: &Context, msg: &Message) -> CommandResult {
    use std::fs;

    let prefix = "/host/sys/class/power_supply/qcom-battery";
    let battery = fs::read_to_string(format!("{}/capacity", prefix)).unwrap_or_else(|_| "unknown".into());
    let status = fs::read_to_string(format!("{}/status", prefix)).unwrap_or_else(|_| "unknown".into());

    let voltage = match fs::read_to_string(format!("{}/voltage_now", prefix)) {
        Ok(s) => match s.trim().parse::<f64>() {
            Ok(val) => format!("{:.2} V", val / 1_000_000.0),
            _ => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    };

    let current = match fs::read_to_string(format!("{}/current_now", prefix)) {
        Ok(s) => match s.trim().parse::<f64>() {
            Ok(val) => format!("{:.2} A", val / 1_000_000.0),
            _ => "unknown".to_string(),
        },
        _ => "unknown".to_string(),
    };

    let reply = format!(
        "Battery: {}%\nStatus:  {}\nVoltage: {}\nCurrent: {}",
        battery.trim(),
        status.trim(),
        voltage,
        current
    );
    msg.reply(ctx, format!("```{}```", reply)).await?;
    Ok(())
}

#[command]
#[aliases("ip")]
async fn ip(ctx: &Context, msg: &Message) -> CommandResult {
    use std::process::Command;
    use regex::Regex;

    let mut lines = vec!["==== Network Interfaces (via ip addr) ====".to_string()];

    // Try to run `ip -o addr` (each line is one address for one interface)
    let possible_ip_paths = ["/sbin/ip", "/usr/sbin/ip", "/bin/ip", "/usr/bin/ip", "ip"];
    let mut output = None;
    let mut last_error = String::new();
    
    for ip_path in possible_ip_paths {
        match Command::new(ip_path).arg("-o").arg("addr").output() {
            Ok(out) if out.status.success() => {
                output = Some(out);
                break;
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                last_error = format!("'{}' failed with exit code: {}\nStderr: {}", ip_path, out.status, stderr);
            }
            Err(e) => {
                last_error = format!("Failed to execute '{}': {}", ip_path, e);
            }
        }
    }
    
    let output = match output {
        Some(out) => out,
        None => {
            // Fallback: show error but still try to read from /host/sys/class/net
            lines.push(format!("⚠️ Could not execute ip command. Last error: {}", last_error));
            lines.push("📁 Falling back to reading from /host/sys/class/net...".to_string());
            
            // Just show the interfaces we can read from sysfs
            let mut found_any = false;
            if let Ok(fs_interfaces) = std::fs::read_dir("/host/sys/class/net") {
                for entry in fs_interfaces.flatten() {
                    let iface = entry.file_name().into_string().unwrap_or_default();
                    if iface == "lo" { continue; }
                    let mac = std::fs::read_to_string(format!("/host/sys/class/net/{}/address", iface))
                        .unwrap_or("unknown".into()).trim().to_string();
                    let state = std::fs::read_to_string(format!("/host/sys/class/net/{}/operstate", iface))
                        .unwrap_or("unknown".into()).trim().to_string();
                    
                    lines.push(format!("\n🔹 Interface: {}", iface));
                    lines.push(format!("   ├─ State     : {}", state));
                    lines.push(format!("   └─ MAC       : {}", mac));
                    found_any = true;
                }
            }
            
            if !found_any {
                lines.push("(no interfaces found in /host/sys/class/net)".to_string());
            }
            
            let reply = format!("```\n{}\n```", lines.join("\n"));
            msg.reply(ctx, &reply).await?;
            return Ok(());
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Map from interface name to (state, MAC, Vec<ip4>, Vec<ip6>)
    use std::collections::BTreeMap;
    let mut ifaces: BTreeMap<String, (String, String, Vec<String>, Vec<String>)> = BTreeMap::new();
    let re = Regex::new(r"\d+: (?P<iface>\S+)\s+\S+ ([^ ]+ )*inet(6)? (?P<ip>[^ ]+)").unwrap();

    // Get interface state and MAC from sysfs if available
    let sys_prefix = "/host/sys/class/net";
    if let Ok(fs_interfaces) = std::fs::read_dir(sys_prefix) {
        for entry in fs_interfaces.flatten() {
            let iface = entry.file_name().into_string().unwrap_or_default();
            if iface == "lo" { continue; }
            let mac = std::fs::read_to_string(format!("{}/{}/address", sys_prefix, iface)).unwrap_or("unknown".into()).trim().to_string();
            let state = std::fs::read_to_string(format!("{}/{}/operstate", sys_prefix, iface)).unwrap_or("unknown".into()).trim().to_string();
            ifaces.entry(iface).or_insert((state, mac, vec![], vec![]));
        }
    }

    // Parse `ip -o addr` output
    for line in stdout.lines() {
        if let Some(cap) = re.captures(line) {
            let iface = cap.name("iface").unwrap().as_str().to_string();
            let ip = cap.name("ip").unwrap().as_str().to_string();
            let is_v6 = line.contains("inet6");
            let entry = ifaces.entry(iface.clone()).or_insert(("?".into(), "?".into(), vec![], vec![]));
            if is_v6 {
                entry.3.push(ip);
            } else {
                entry.2.push(ip);
            }
        }
    }

    if ifaces.is_empty() {
        lines.push("(no interfaces found)".to_string());
    }
    for (iface, (state, mac, ip4s, ip6s)) in ifaces {
        lines.push(format!("\n🔹 Interface: {iface}"));
        lines.push(format!("   ├─ State     : {state}"));
        lines.push(format!("   ├─ MAC       : {mac}"));
        for ip in ip4s {
            lines.push(format!("   ├─ IPv4      : {ip}"));
        }
        for ip in ip6s {
            lines.push(format!("   └─ IPv6      : {ip}"));
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
async fn debug(ctx: &Context, msg: &Message) -> CommandResult {
    use std::process::Command;
    
    let mut lines = vec!["🔍 Debug Information".to_string()];
    
    // Check PATH
    if let Ok(path) = std::env::var("PATH") {
        lines.push(format!("\n📁 PATH: {}", path));
    }
    
    // Check if ip command exists at various locations
    let possible_paths = ["/sbin/ip", "/usr/sbin/ip", "/bin/ip", "/usr/bin/ip"];
    lines.push("\n🔍 IP command locations:".to_string());
    
    for path in possible_paths {
        if std::path::Path::new(path).exists() {
            lines.push(format!("   ✅ {} exists", path));
            
            // Try to get version
            match Command::new(path).arg("--version").output() {
                Ok(out) if out.status.success() => {
                    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    lines.push(format!("      Version: {}", version));
                }
                Ok(out) => {
                    lines.push(format!("      Version check failed: exit code {}", out.status));
                }
                Err(e) => {
                    lines.push(format!("      Version check error: {}", e));
                }
            }
        } else {
            lines.push(format!("   ❌ {} not found", path));
        }
    }
    
    // Check /host/sys/class/net
    lines.push("\n🌐 Network interfaces in /host/sys/class/net:".to_string());
    match std::fs::read_dir("/host/sys/class/net") {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().into_string().unwrap_or_default();
                lines.push(format!("   📡 {}", name));
            }
        }
        Err(e) => {
            lines.push(format!("   ❌ Error reading /host/sys/class/net: {}", e));
        }
    }
    
    // Check current user
    lines.push("\n👤 Process info:".to_string());
    if let Ok(output) = Command::new("id").output() {
        if output.status.success() {
            let id_output = String::from_utf8_lossy(&output.stdout).trim().to_string();
            lines.push(format!("   {}", id_output));
        }
    }
    
    // Check capabilities (if available)
    if let Ok(output) = Command::new("cat").arg("/proc/self/status").output() {
        if output.status.success() {
            let status = String::from_utf8_lossy(&output.stdout);
            for line in status.lines() {
                if line.starts_with("Cap") {
                    lines.push(format!("   {}", line));
                }
            }
        }
    }
    
    let reply = format!("```\n{}\n```", lines.join("\n"));
    msg.reply(ctx, &reply).await?;
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
- **/debug** or **!debug**
  Shows debugging information about the bot environment and available commands. Example: `!debug`
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
