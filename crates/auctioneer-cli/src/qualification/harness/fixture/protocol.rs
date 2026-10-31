use super::super::{HarnessError, Result, io, process::Supervisor};
use super::{Control, Options, server::Server};
use auctioneer_runtime::count_json_nodes;
use futures::future::{Either, select};
#[cfg(unix)]
use rustix::io::dup;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
#[cfg(unix)]
use std::io::{stdin, stdout};
use std::time::Duration;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader,
};
use tokio::time::timeout;

#[cfg(unix)]
pub async fn serve_stdio(supervisor: &Supervisor) -> Result<()> {
    use tokio::net::unix::pipe::{Receiver, Sender};
    let input = dup(stdin()).map_err(|_error| HarnessError::new("input_io"))?;
    let output = dup(stdout()).map_err(|_error| HarnessError::new("output_io"))?;
    let receiver =
        Receiver::from_owned_fd(input).map_err(|_error| HarnessError::new("invalid_input"))?;
    let sender =
        Sender::from_owned_fd(output).map_err(|_error| HarnessError::new("invalid_input"))?;
    serve(receiver, sender, supervisor).await
}

#[cfg(not(unix))]
pub async fn serve_stdio(_supervisor: &Supervisor) -> Result<()> {
    Err(HarnessError::new("identity_unsupported"))
}

async fn record_tail<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut record = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .await
            .map_err(|_error| HarnessError::new("input_io"))?;
        if buffer.is_empty() {
            return Err(HarnessError::new("invalid_input"));
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let length = newline.unwrap_or(buffer.len());
        if record
            .len()
            .checked_add(length)
            .is_none_or(|total| total > 4096)
        {
            return Err(HarnessError::new("invalid_input"));
        }
        record.extend_from_slice(
            buffer
                .get(..length)
                .ok_or(HarnessError::new("invalid_input"))?,
        );
        reader.consume(length.saturating_add(usize::from(newline.is_some())));
        if newline.is_some() {
            return Ok(record);
        }
    }
}

async fn read_record<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    // Idle control channels may span whole measurement windows. Once a frame
    // starts, incomplete data cannot hold the child indefinitely.
    if reader
        .fill_buf()
        .await
        .map_err(|_error| HarnessError::new("input_io"))?
        .is_empty()
    {
        return Ok(None);
    }
    timeout(Duration::from_secs(2), record_tail(reader))
        .await
        .map_err(|_error| HarnessError::new("fixture_timeout"))?
        .map(Some)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    count_json_nodes(bytes, 4096).map_err(|_error| HarnessError::new("invalid_input"))?;
    serde_json::from_slice(bytes).map_err(|_error| HarnessError::new("invalid_input"))
}

async fn write_record<W: AsyncWrite + Unpin, T: Serialize>(
    output: &mut W,
    value: &T,
) -> Result<()> {
    let bytes = io::json_bytes(value, 4096)?;
    timeout(Duration::from_secs(2), async {
        output.write_all(&bytes).await?;
        output.write_all(b"\n").await?;
        output.flush().await
    })
    .await
    .map_err(|_error| HarnessError::new("fixture_timeout"))?
    .map_err(|_error| HarnessError::new("output_io"))
}

async fn controls<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin>(
    input: &mut R,
    output: &mut W,
    server: &mut Server,
    supervisor: &Supervisor,
) -> Result<()> {
    loop {
        let record = {
            let stopped = select(
                Box::pin(supervisor.cancelled()),
                Box::pin(server.completed()),
            );
            match select(Box::pin(stopped), Box::pin(read_record(input))).await {
                Either::Left((Either::Left((error, _server)), _input)) => Err(error),
                Either::Left((Either::Right((result, _signal)), _input)) => {
                    result.and(Err(HarnessError::new("fixture_timeout")))
                }
                Either::Right((record, _stopped)) => record,
            }
        };
        let Some(bytes) = record? else {
            return Ok(());
        };
        let reply = server.command(decode::<Control>(&bytes)?).await?;
        write_record(output, &reply).await?;
    }
}

pub async fn serve<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    input: R,
    mut output: W,
    supervisor: &Supervisor,
) -> Result<()> {
    let mut reader = BufReader::with_capacity(1024, input);
    let record = timeout(Duration::from_secs(2), read_record(&mut reader))
        .await
        .map_err(|_error| HarnessError::new("fixture_timeout"))??
        .ok_or(HarnessError::new("invalid_input"))?;
    supervisor.check_active().await?;
    let mut server = Server::start(decode::<Options>(&record)?).await?;
    let result = async {
        write_record(
            &mut output,
            &json!({"ready":true,"protocol":1_u8,"port":server.port}),
        )
        .await?;
        controls(&mut reader, &mut output, &mut server, supervisor).await
    }
    .await;
    server.stop().await?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::time::Duration;
    use tokio::io::duplex;
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    #[tokio::test]
    async fn private_records_preserve_framing_at_the_cap_and_refuse_partial_or_overflow_data() {
        let (mut sender, receiver) = duplex(0x4000_usize);
        sender.write_all(&vec![b'0'; 4096]).await.unwrap();
        sender.write_all(b"\nnext\n").await.unwrap();
        drop(sender);
        let mut reader = BufReader::with_capacity(128, receiver);
        assert_eq!(read_record(&mut reader).await.unwrap().unwrap().len(), 4096);
        assert_eq!(
            read_record(&mut reader).await.unwrap(),
            Some(b"next".to_vec())
        );
        assert_eq!(read_record(&mut reader).await.unwrap(), None);
        for payload in [vec![b'x'; 4097], b"partial".to_vec()] {
            let (mut invalid_sender, invalid_receiver) = duplex(8192);
            invalid_sender.write_all(&payload).await.unwrap();
            drop(invalid_sender);
            assert_eq!(
                read_record(&mut BufReader::new(invalid_receiver))
                    .await
                    .unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
    }

    #[tokio::test]
    async fn private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof() {
        let (mut control, input) = duplex(8192);
        let (output, responses) = duplex(8192);
        let supervisor = Supervisor::new();
        let child = tokio::spawn(async move { serve(input, output, &supervisor).await });
        control.write_all(b"{\"kind\":\"native\",\"port\":0,\"body_cap\":4096,\"max_requests\":100,\"max_sockets\":4}\n").await.unwrap();
        let mut reader = BufReader::new(responses);
        let mut line = String::new();
        timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(ready["protocol"], 1_u8);
        assert_eq!(ready["ready"], true);
        let port = u16::try_from(ready["port"].as_u64().unwrap()).unwrap();
        let connected = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        drop(connected);
        control
            .write_all(b"{\"id\":1,\"command\":\"stats\",\"settings\":null}\n")
            .await
            .unwrap();
        line.clear();
        timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], 1_u32);
        assert_eq!(reply["stats"]["calls"], 0_u32);
        drop(control);
        timeout(Duration::from_secs(5), child)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        TcpStream::connect(("127.0.0.1", port)).await.unwrap_err();
        assert_eq!(
            reply.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["id", "stats"]
        );
        assert_eq!(json!({"protocol":1_u8,"ready":true,"port":port}), ready);
    }

    #[tokio::test]
    async fn malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener() {
        for message in [
            b"{\"kind\":\"SECRET_CANARY\"}\n".as_slice(),
            b"{\"kind\":\"native\",\"kind\":\"provider\"}\n",
            b"{\"kind\":\"native\",\"port\":0,\"body_cap\":4096,\"max_requests\":100,\"max_sockets\":4,\"SECRET_CANARY\":true}\n",
        ] {
            let (mut control, input) = duplex(8192);
            let (output, responses) = duplex(8192);
            control.write_all(message).await.unwrap();
            drop(control);
            let error = serve(input, output, &Supervisor::new()).await.unwrap_err();
            assert_eq!(error, HarnessError::new("invalid_input"));
            let mut line = String::new();
            assert_eq!(BufReader::new(responses).read_line(&mut line).await.unwrap(), 0);
        }
        let (mut control, input) = duplex(8192);
        let (output, responses) = duplex(8192);
        let supervisor = Supervisor::new();
        let cancellation = supervisor.clone();
        let child = tokio::spawn(async move { serve(input, output, &cancellation).await });
        control.write_all(b"{\"kind\":\"native\",\"port\":0,\"body_cap\":4096,\"max_requests\":100,\"max_sockets\":4}\n").await.unwrap();
        let mut reader = BufReader::new(responses);
        let mut line = String::new();
        timeout(Duration::from_secs(5), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        let port = u16::try_from(ready["port"].as_u64().unwrap()).unwrap();
        supervisor.interrupt(143);
        assert_eq!(
            timeout(Duration::from_secs(5), child)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err(),
            HarnessError::new("interrupted")
        );
        TcpStream::connect(("127.0.0.1", port)).await.unwrap_err();
        line.clear();
        assert_eq!(reader.read_line(&mut line).await.unwrap(), 0);
    }
}
