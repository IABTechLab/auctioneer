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
    use tokio::{io::unix::AsyncFd, net::unix::pipe, time::timeout};

    const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

    enum Output {
        File(File),
        Pipe(pipe::Sender),
        Ready(AsyncFd<File>),
    }

    fn stdout() -> Result<Output> {
        let descriptor = io::stdout()
            .as_fd()
            .try_clone_to_owned()
            .map_err(|_error| HarnessError::new("output_io"))?;
        output(File::from(descriptor))
    }

    fn output(file: File) -> Result<Output> {
        let metadata = file
            .metadata()
            .map_err(|_error| HarnessError::new("output_io"))?;
        let kind = metadata.file_type();
        let dev_null = kind.is_char_device()
            && fs::metadata("/dev/null").is_ok_and(|device| device.rdev() == metadata.rdev());
        if metadata.is_file() || dev_null {
            return Ok(Output::File(file));
        }
        if kind.is_fifo() {
            return pipe::Sender::from_owned_fd(file.into())
                .map(Output::Pipe)
                .map_err(|_error| HarnessError::new("output_io"));
        }
        if !kind.is_socket() && !file.is_terminal() {
            return Err(HarnessError::new("output_io"));
        }
        let flags = fcntl_getfl(&file).map_err(|_error| HarnessError::new("output_io"))?;
        fcntl_setfl(&file, flags | OFlags::NONBLOCK)
            .map_err(|_error| HarnessError::new("output_io"))?;
        #[expect(
            unsafe_code,
            reason = "the private writer retains exclusive ownership of its duplicated descriptor"
        )]
        // SAFETY: Only socket/terminal output reaches this fallback; pipes use
        // the safe constructor above. File owns a valid duplicated descriptor.
        // Output never exposes or replaces it, and retains it across readiness
        // waits and timeout.
        // AsyncFd unregisters before dropping the owned File; forgetting it
        // would retain both the registration and descriptor instead of closing it.
        let output =
            unsafe { AsyncFd::register(file) }.map_err(|_error| HarnessError::new("output_io"))?;
        Ok(Output::Ready(output))
    }

    async fn pipe_write(output: &pipe::Sender, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            output.writable().await?;
            match output.try_write(bytes) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(count) => {
                    bytes = bytes
                        .get(count..)
                        .ok_or_else(|| Error::from(ErrorKind::InvalidData))?;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
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
            Output::Pipe(output) => timeout(WRITE_TIMEOUT, pipe_write(&output, bytes))
                .await
                .map_err(|_elapsed| HarnessError::new("output_limit"))?
                .map_err(|_error| HarnessError::new("output_io")),
            Output::Ready(output) => timeout(WRITE_TIMEOUT, ready_write(&output, bytes))
                .await
                .map_err(|_elapsed| HarnessError::new("output_limit"))?
                .map_err(|_error| HarnessError::new("output_io")),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::fd::OwnedFd;
        use tokio::io::AsyncReadExt as _;
        use tokio::net::{UnixStream, unix::pipe};

        #[tokio::test]
        async fn fifo_has_a_dedicated_nonblocking_output() {
            let (sender, _receiver) = pipe::pipe().unwrap();
            let selected = output(File::from(sender.into_blocking_fd().unwrap())).unwrap();
            assert!(
                !matches!(selected, Output::Ready(_)),
                "FIFO output must not use generic unsafe registration"
            );
            assert!(
                !matches!(selected, Output::File(_)),
                "FIFO output must not use the blocking file path"
            );
        }

        #[tokio::test]
        async fn read_only_pipe_is_refused_before_readiness_registration() {
            let (_sender, receiver) = pipe::pipe().unwrap();
            let selected = output(File::from(receiver.into_blocking_fd().unwrap()));
            assert_eq!(selected.err(), Some(HarnessError::new("output_io")));
        }

        #[tokio::test]
        async fn socket_output_retains_the_audited_readiness_path() {
            let (sender, mut receiver) = UnixStream::pair().unwrap();
            let file = File::from(OwnedFd::from(sender.into_std().unwrap()));
            let Output::Ready(selected) = output(file).unwrap() else {
                panic!("socket output must retain readiness registration");
            };
            ready_write(&selected, b"socket\n").await.unwrap();
            let mut payload = [0_u8; 7];
            timeout(Duration::from_secs(1), receiver.read_exact(&mut payload))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(&payload, b"socket\n");
        }

        #[test]
        fn regular_files_and_dev_null_retain_the_file_path() {
            for file in [
                tempfile::tempfile().unwrap(),
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/null")
                    .unwrap(),
            ] {
                assert!(matches!(output(file).unwrap(), Output::File(_)));
            }
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
