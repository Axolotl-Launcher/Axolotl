use anyhow::Result;
use clap::Parser;

use async_minecraft_ping::ConnectionConfig;

#[derive(Debug, Parser)]
#[command(name = "example")]
struct Args {
    /// Server to connect to
    address: String,

    /// Port to connect to
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,

    /// Enable SRV record lookup (requires 'srv' feature)
    #[cfg(feature = "srv")]
    #[arg(long = "srv")]
    srv_lookup: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut config = ConnectionConfig::build(args.address);
    if let Some(port) = args.port {
        config = config.with_port(port);
    }
    #[cfg(feature = "srv")]
    if args.srv_lookup {
        config = config.with_srv_lookup();
    }

    let connection = config.connect().await?;

    let connection = connection.status().await?;

    println!(
        "{} of {} player(s) online",
        connection.status.players.online, connection.status.players.max
    );

    connection.ping(42).await?;

    Ok(())
}
