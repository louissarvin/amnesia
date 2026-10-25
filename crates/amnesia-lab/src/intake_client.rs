//! Fetches a one-time address from a desk's donor intake exactly as a donor's browser would:
//! GET the form, solve the proof of work, POST with `Accept: text/plain`.

use anyhow::{Result, anyhow, bail};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub(crate) async fn http(host: &str, req: String) -> Result<(u16, String)> {
    let mut s = TcpStream::connect(host).await?;
    s.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await?;
    let text = String::from_utf8(buf)?;
    let (head, body) = text.split_once("\r\n\r\n").ok_or(anyhow!("bad response"))?;
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or(anyhow!("bad status"))?;
    Ok((status, body.to_string()))
}

fn attr<'a>(html: &'a str, name: &str) -> Result<&'a str> {
    let key = format!("{name}=\"");
    let i = html.find(&key).ok_or(anyhow!("missing {name}"))? + key.len();
    let j = html[i..].find('"').ok_or(anyhow!("unterminated {name}"))?;
    Ok(&html[i..i + j])
}

fn solve(salt: &[u8; 16], id: &[u8; 16], bits: u32) -> String {
    (0u64..)
        .map(|n| n.to_string())
        .find(|n| amnesia_core::pow::verify(salt, id, n.as_bytes(), bits))
        .expect("unbounded search")
}

/// Returns (address, valid_until_utc_hour).
/// Returns the address, its deadline, and the shred receipt (epoch id) when the desk sends one.
pub async fn fetch_address(host: &str) -> Result<(String, u64, Option<String>)> {
    let (code, form) = http(host, format!("GET /donate HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")).await?;
    if code != 200 {
        bail!("intake form returned {code}");
    }
    let salt: [u8; 16] = hex::decode(attr(&form, "data-salt")?)?.try_into().map_err(|_| anyhow!("salt"))?;
    let id: [u8; 16] = hex::decode(attr(&form, "data-id")?)?.try_into().map_err(|_| anyhow!("id"))?;
    let bits: u32 = attr(&form, "data-bits")?.parse()?;
    let nonce = solve(&salt, &id, bits);
    let body = format!("challenge_id={}&nonce={nonce}", hex::encode(id));
    let req = format!(
        "POST /donate HTTP/1.1\r\nHost: {host}\r\nAccept: text/plain\r\nContent-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let (code, text) = http(host, req).await?;
    if code != 200 {
        bail!("intake returned {code}");
    }
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(str::to_string);
    let address = field("address").ok_or(anyhow!("no address"))?;
    let until = field("valid_until_utc").and_then(|v| v.parse().ok()).ok_or(anyhow!("no deadline"))?;
    let receipt = field("receipt").filter(|r| r.len() == 32 && r.bytes().all(|b| b.is_ascii_hexdigit()));
    Ok((address, until, receipt))
}
