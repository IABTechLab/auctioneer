use super::{HarnessError, Result, io};
use serde::Serialize;

#[cfg(unix)]
mod unix {
    use super::{HarnessError, Result};
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
    use std::fs::{self, File};
    use std::io::{self, Error, ErrorKind, IsTerminal as _, Write as _};
    use std::os::fd::AsFd as _;
    use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _};
    use std::time::Duration;
    use tokio::{io::unix::AsyncFd, time::timeout};

    enum Output {
        File(File),
        Ready(AsyncFd<File>),
    }

    fn stdout() -> Result<Output> {
        let descriptor = io::stdout()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|_error| HarnessError::new("output_io"))?;
        let file = File::from(descriptor);
        let metadata = file
            .metadata()
            .map_err(|_error| HarnessError::new("output_io"))?;
        let kind = metadata.file_type();
        let dev_null = kind.is_char_device()
            && fs::metadata("/dev/null").is_ok_and(|device| device.rdev() == metadata.rdev());
        if metadata.is_file() || dev_null {
            return Ok(Output::File(file));
        }
        if !kind.is_fifo() && !kind.is_socket() && !io::stdout().is_terminal() {
            return Err(HarnessError::new("output_io"));
        }
        let flags = fcntl_getfl(&file).map_err(|_error| HarnessError::new("output_io"))?;
        fcntl_setfl(&file, flags | OFlags::NONBLOCK)
            .map_err(|_error| HarnessError::new("output_io"))?;
        Ok(Output::Ready(
            AsyncFd::new(file).map_err(|_error| HarnessError::new("output_io"))?,
        ))
    }

    async fn ready_write(output: &AsyncFd<File>, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let mut ready = output.writable().await?;
            match ready.try_io(|descriptor| descriptor.get_ref().write(bytes)) {
                Ok(Ok(0)) => return Err(ErrorKind::WriteZero.into()),
                Ok(Ok(count)) => {
                    bytes = bytes
                        .get(count..)
                        .ok_or_else(|| Error::from(ErrorKind::InvalidData))?;
                }
                Ok(Err(error)) => return Err(error),
                Err(_would_block) => {}
            }
        }
        Ok(())
    }

    pub async fn write(bytes: &[u8]) -> Result<()> {
        match stdout()? {
            // Regular files use the same bounded filesystem boundary as the
            // artifact writer. Pipe/terminal deadlines own no blocking thread.
            Output::File(mut output) => output
                .write_all(bytes)
                .map_err(|_error| HarnessError::new("output_io")),
            Output::Ready(output) => timeout(Duration::from_secs(2), ready_write(&output, bytes))
                .await
                .map_err(|_elapsed| HarnessError::new("output_limit"))?
                .map_err(|_error| HarnessError::new("output_io")),
        }
    }
}

pub async fn write_json<T: Serialize>(value: &T) -> Result<()> {
    let mut bytes = io::json_bytes(value, 256 << 10_u32)?;
    bytes.push(b'\n');
    #[cfg(unix)]
    {
        unix::write(&bytes).await
    }
    #[cfg(not(unix))]
    {
        Err(HarnessError::new("identity_unsupported"))
    }
}
